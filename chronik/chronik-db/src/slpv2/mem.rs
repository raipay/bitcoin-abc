use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
};

use abc_rust_error::Result;
use bitcoinsuite_core::{tx::{OutPoint, Tx, TxId}, error};
use bitcoinsuite_slp::slpv2;
use chronik_util::log;
use thiserror::Error;

use crate::{db::Db, io::TxReader, slpv2::io::Slpv2Reader, mem::MempoolTx};

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
        log!("Tx {} parsed {:#?}\n", tx.tx.txid(), parsed);
        if parsed.parsed.sections.is_empty() {
            return Ok(());
        }
        let (mut tx_data, error) =
            slpv2::TxSpec::process_parsed(&parsed.parsed, &tx.tx);
        log!("Tx {} processed = {:#?}, error = {:?}\n", tx.tx.txid(), tx_data, error);
        let mut tx_data_inputs =
            HashMap::<TxId, Option<Cow<'_, slpv2::TxData>>>::new();
        let mut actual_inputs = Vec::with_capacity(tx.tx.inputs.len());
        for input in &tx.tx.inputs {
            if let Entry::Vacant(entry) =
                tx_data_inputs.entry(input.prev_out.txid)
            {
                let tx_data =
                    self.get_tx_data(db, entry.key(), &is_mempool_tx)?;
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
        let burns = slpv2::verify(&mut tx_data, &actual_inputs);
        log!("Tx {} verified = {:#?}, burns = {:?}\n", tx.tx.txid(), tx_data, burns);
        if !tx_data.sections.is_empty() {
            let tx_data = slpv2::TxData::from_spec_and_inputs(tx_data, &actual_inputs);
            self.valid_txs.insert(tx.tx.txid(), tx_data);
            if let slpv2::SectionVariant::Genesis(genesis) = &parsed.parsed.sections[0].variant {
                self.genesis_data.insert(tx.tx.txid(), genesis.data.clone());
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, txid: &TxId) {
        self.valid_txs.remove(txid);
        self.genesis_data.remove(txid);
    }

    fn get_tx_data(
        &self,
        db: &Db,
        txid: &TxId,
        is_mempool_tx: &impl Fn(&TxId) -> bool,
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
