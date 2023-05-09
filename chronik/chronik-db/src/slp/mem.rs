use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::TxId;
use bitcoinsuite_slp::slp;
use chronik_util::log;
use thiserror::Error;

use crate::{db::Db, io::TxReader, mem::MempoolTx, slp::io::SlpReader};

#[derive(Debug, Default)]
pub struct MempoolSlp {
    valid_txs: HashMap<TxId, slp::TxData>,
    genesis_data: HashMap<TxId, slp::GenesisInfo>,
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
        let parsed = match slp::parse_tx(&tx.tx) {
            Ok(parsed) => parsed,
            Err(err) => {
                log!("Tx {} parse error {}\n", tx.tx.txid(), err);
                return Ok(());
            }
        };
        let mut tx_data_inputs =
            HashMap::<TxId, Option<Cow<'_, slp::TxData>>>::new();
        let mut actual_inputs = Vec::with_capacity(tx.tx.inputs.len());
        for input in &tx.tx.inputs {
            if let Entry::Vacant(entry) =
                tx_data_inputs.entry(input.prev_out.txid)
            {
                let tx_data =
                    self.tx_data_or_read(db, entry.key(), &is_mempool_tx)?;
                entry.insert(tx_data);
            }
        }
        for input in &tx.tx.inputs {
            let tx_data_input = &tx_data_inputs[&input.prev_out.txid];
            actual_inputs.push(tx_data_input.as_ref().and_then(|tx_data| {
                tx_data
                    .output_tokens
                    .get(input.prev_out.out_idx as usize)
                    .map(|token| slp::SlpSpentOutput {
                        token_id: tx_data.token_id,
                        token_type: tx_data.token_type,
                        token: *token,
                        group_token_id: tx_data.group_token_id.clone(),
                    })
            }));
        }
        let tx_data = match slp::validate(&parsed, &actual_inputs) {
            Ok(tx_data) => tx_data,
            Err(_) => return Ok(()),
        };
        self.valid_txs.insert(tx.tx.txid(), tx_data);
        if let slp::TxType::Genesis(genesis) = &parsed.tx_type {
            self.genesis_data
                .insert(tx.tx.txid(), genesis.as_ref().clone());
        }
        Ok(())
    }

    pub fn remove(&mut self, txid: &TxId) {
        self.valid_txs.remove(txid);
        self.genesis_data.remove(txid);
    }

    pub fn tx_data_or_read(
        &self,
        db: &Db,
        txid: &TxId,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<Option<Cow<'_, slp::TxData>>> {
        let tx_reader = TxReader::new(db)?;
        let slp_reader = SlpReader::new(db)?;
        match self.valid_txs.get(txid) {
            Some(tx_data) => Ok(Some(Cow::Borrowed(tx_data))),
            None => {
                if is_mempool_tx(txid) {
                    return Ok(None);
                }
                let tx_num = tx_reader
                    .tx_num_by_txid(txid)?
                    .ok_or(InputNotFound(*txid))?;
                match slp_reader.tx_data_by_tx_num(tx_num)? {
                    Some(tx_data) => Ok(Some(Cow::Owned(tx_data))),
                    None => Ok(None),
                }
            }
        }
    }

    pub fn tx_data(&self, txid: &TxId) -> Option<&slp::TxData> {
        self.valid_txs.get(txid)
    }

    pub fn genesis_data(&self, txid: &TxId) -> Option<&slp::GenesisInfo> {
        self.genesis_data.get(txid)
    }
}
