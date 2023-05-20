use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::TxId;
use bitcoinsuite_slp::slpv2;
use thiserror::Error;

use crate::{db::Db, io::TxReader, mem::MempoolTx, slpv2::io::Slpv2Reader};

#[derive(Debug, Default)]
pub struct MempoolSlpv2 {
    valid_txs: HashMap<TxId, slpv2::TxData>,
    genesis_data: HashMap<TxId, slpv2::GenesisData>,
}

/// Error indicating something went wrong with [`MempoolGroupUtxos`].
#[derive(Debug, Eq, Error, PartialEq)]
pub enum MempoolSlpv2Error {
    /// Tried adding a UTXO that already exists
    #[error("Inconsistent DB: Mempool tx spending {0} found neither in the mempool nor DB")]
    InputNotFound(TxId),
}

use self::MempoolSlpv2Error::*;

impl MempoolSlpv2 {
    pub fn insert(
        &mut self,
        db: &Db,
        tx: &MempoolTx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<()> {
        let parsed = slpv2::parse_tx(&tx.tx);
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(_) => return Ok(()),
        };
        let tx_spec = slpv2::TxSpec::process_parsed_pushdata(parsed, &tx.tx);
        let mut tx_data_inputs =
            HashMap::<TxId, Option<Cow<'_, slpv2::TxData>>>::new();
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
            actual_inputs.push(
                tx_data_input
                    .as_ref()
                    .and_then(|tx_data| {
                        tx_data.outputs().nth(input.prev_out.out_idx as usize)
                    })
                    .flatten(),
            );
        }
        let (tx_data, burns) =
            slpv2::verify(tx_spec.sections, tx_spec.outputs, &actual_inputs);
        if !tx_data.sections.is_empty() {
            self.valid_txs.insert(tx.tx.txid(), tx_data);
            if let Some((meta, genesis_data)) =
                tx_spec.genesis_data
            {
                self.genesis_data.insert(tx.tx.txid(), genesis_data);
            }
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
    ) -> Result<Option<Cow<'_, slpv2::TxData>>> {
        let tx_reader = TxReader::new(db)?;
        let slp_reader = Slpv2Reader::new(db)?;
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

    pub fn tx_data(&self, txid: &TxId) -> Option<&slpv2::TxData> {
        self.valid_txs.get(txid)
    }

    pub fn genesis_data(&self, txid: &TxId) -> Option<&slpv2::GenesisData> {
        self.genesis_data.get(txid)
    }
}
