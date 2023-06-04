use std::collections::{hash_map::Entry, HashMap};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, TxId};
use bitcoinsuite_slp::{slp, slpv2};
use chronik_util::log;
use thiserror::Error;

use crate::{
    db::Db,
    io::TxReader,
    mem::MempoolTx,
    slp::{
        data::{
            only_slpv1_inputs, only_slpv2_inputs, EitherToken, EitherTxData,
            Protocol,
        },
        io::SlpReader,
    },
};

#[derive(Debug, Default)]
pub struct MempoolSlp {
    slp_txs: HashMap<TxId, EitherTxData>,
    tx_token_inputs: HashMap<TxId, Vec<Option<EitherToken>>>,
    slpv1_errors: HashMap<TxId, slp::ParseError>,
}

/// Error indicating something went wrong with [`MempoolGroupUtxos`].
#[derive(Debug, Eq, Error, PartialEq)]
pub enum MempoolSlpError {
    /// Tried adding a UTXO that already exists
    #[error("Inconsistent DB: Mempool tx spending {0} found neither in the mempool nor DB")]
    InputNotFound(TxId),
}

use self::MempoolSlpError::*;

impl MempoolSlp {
    pub fn insert(
        &mut self,
        db: &Db,
        tx: &MempoolTx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<()> {
        let mut has_any_tokens: bool = false;
        let mut tx_data_inputs = HashMap::<TxId, _>::new();
        for input in &tx.tx.inputs {
            if let Entry::Vacant(entry) =
                tx_data_inputs.entry(input.prev_out.txid)
            {
                let token_outputs =
                    self.get_token_outputs(db, entry.key(), &is_mempool_tx)?;
                let out_idx = input.prev_out.out_idx as usize;
                if let Some(token_outputs) = &token_outputs {
                    if matches!(token_outputs.get(out_idx), Some(Some(_))) {
                        has_any_tokens = true;
                    }
                }
                entry.insert(token_outputs);
            }
        }
        let mut actual_inputs = Vec::with_capacity(tx.tx.inputs.len());
        for input in &tx.tx.inputs {
            actual_inputs.push(
                tx_data_inputs[&input.prev_out.txid].as_ref().and_then(
                    |tx_data| tx_data[input.prev_out.out_idx as usize].clone(),
                ),
            );
        }
        match slp::parse_tx(&tx.tx) {
            Ok(parse_data) => {
                let actual_inputs = only_slpv1_inputs(&actual_inputs);
                let tx_data = slp::verify(&parse_data, &actual_inputs);
                self.slp_txs.insert(tx.tx.txid(), Protocol::Slp(tx_data));
            }
            Err(parse_error) => {
                let colored_tx = slpv2::ColoredTx::parse_tx(&tx.tx);
                if !colored_tx.should_ignore() || has_any_tokens {
                    let actual_inputs = only_slpv2_inputs(&actual_inputs);
                    let tx_data = slpv2::verify(colored_tx, &actual_inputs);
                    log!(
                        "Added SLPv2 tx data for {} = {:?}\n",
                        tx.tx.txid(),
                        tx_data
                    );
                    self.slp_txs.insert(tx.tx.txid(), Protocol::Slpv2(tx_data));
                }
                log!(
                    "Added SLPv1 error for {} = {:?}\n",
                    tx.tx.txid(),
                    parse_error
                );
                self.slpv1_errors.insert(tx.tx.txid(), parse_error);
            }
        }
        if has_any_tokens {
            log!(
                "Added actual inputs for {} = {:?}\n",
                tx.tx.txid(),
                actual_inputs
            );
            self.tx_token_inputs.insert(tx.tx.txid(), actual_inputs);
        }
        Ok(())
    }

    pub fn remove(&mut self, txid: &TxId) {
        self.slp_txs.remove(txid);
        self.tx_token_inputs.remove(txid);
        self.slpv1_errors.remove(txid);
    }

    fn get_token_outputs(
        &self,
        db: &Db,
        txid: &TxId,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<Option<Vec<Option<EitherToken>>>> {
        match self.slp_txs.get(txid) {
            Some(Protocol::Slp(tx_data)) => Ok(Some(
                tx_data
                    .output_tokens
                    .iter()
                    .map(|&token| {
                        token.map(|token| {
                            Protocol::Slp(slp::SlpSpentOutput {
                                meta: tx_data.meta,
                                token,
                                group_token_id: tx_data.group_token_id.clone(),
                            })
                        })
                    })
                    .collect(),
            )),
            Some(Protocol::Slpv2(tx_data)) => Ok(Some(
                tx_data
                    .outputs()
                    .map(|token| token.map(Protocol::Slpv2))
                    .collect(),
            )),
            None => {
                if is_mempool_tx(txid) {
                    return Ok(None);
                }
                let tx_reader = TxReader::new(db)?;
                let slp_reader = SlpReader::new(db)?;
                let tx_num = tx_reader
                    .tx_num_by_txid(txid)?
                    .ok_or(InputNotFound(*txid))?;
                let (metas, db_tx_data) =
                    match slp_reader.token_metas_and_db_by_tx_num(tx_num)? {
                        Some(token) => token,
                        None => return Ok(None),
                    };
                Ok(Some(
                    db_tx_data.assemble_tokens(&db_tx_data.outputs, &metas),
                ))
            }
        }
    }

    pub fn tx_data(&self, txid: &TxId) -> Option<&EitherTxData> {
        self.slp_txs.get(txid)
    }

    pub fn tx_token_inputs(
        &self,
        txid: &TxId,
    ) -> Option<&Vec<Option<EitherToken>>> {
        self.tx_token_inputs.get(txid)
    }

    pub fn slpv1_error(&self, txid: &TxId) -> Option<&slp::ParseError> {
        self.slpv1_errors.get(txid)
    }

    pub fn token_output(&self, outpoint: &OutPoint) -> Option<EitherToken> {
        let out_idx = outpoint.out_idx as usize;
        let tx_data = self.slp_txs.get(&outpoint.txid)?;
        match tx_data {
            Protocol::Slp(tx_data) => {
                tx_data.output_tokens.get(out_idx).and_then(|&token| {
                    token.map(|token| Protocol::Slp(slp::SlpSpentOutput {
                        meta: tx_data.meta,
                        token,
                        group_token_id: tx_data.group_token_id.clone(),
                    }))
                })
            }
            Protocol::Slpv2(tx_data) => tx_data
                .outputs()
                .nth(out_idx)
                .flatten()
                .map(Protocol::Slpv2),
        }
    }

    pub fn genesis_data(
        &self,
        txid: &TxId,
    ) -> Option<Protocol<&slp::GenesisInfo, &slpv2::GenesisInfo>> {
        let tx_data = self.slp_txs.get(txid)?;
        match tx_data {
            Protocol::Slp(tx_data) => {
                Some(Protocol::Slp(tx_data.genesis_info.as_ref()?))
            }
            Protocol::Slpv2(tx_data) => {
                let section = tx_data.sections.get(0)?;
                Some(Protocol::Slpv2(section.genesis_info.as_ref()?))
            }
        }
    }
}
