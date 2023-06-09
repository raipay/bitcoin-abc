use abc_rust_error::Result;
use bitcoinsuite_core::{
    error::DataError,
    ser::BitcoinSer,
    tx::{Tx, TxId, TxMut},
};
use bitcoinsuite_slp::{slp, slpv2};
use bytes::Bytes;
use chronik_bridge::ffi;
use chronik_db::{db::Db, mem::Mempool, slp::data::Protocol};
use thiserror::Error;

use crate::{indexer::Node, query::SlpDbData};

pub struct QueryBroadcast<'a> {
    /// Database
    pub db: &'a Db,
    /// Mempool
    pub mempool: &'a Mempool,
    pub node: &'a Node,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum QueryBroadcastError {
    /// Transaction not in mempool nor DB.
    #[error("400: Parsing tx failed {0}")]
    ParsingFailed(DataError),

    #[error("400: {0}")]
    SlpError(String),

    #[error("400: Broadcast failed: {0}")]
    BroadcastFailed(String),
}

use self::QueryBroadcastError::*;

impl QueryBroadcast<'_> {
    pub fn broadcast_txs(&self, raw_txs: Vec<Bytes>) -> Result<Vec<TxId>> {
        let mut tx_errors = Vec::new();
        for mut raw_tx in raw_txs.iter().cloned() {
            let tx = TxMut::deser(&mut raw_tx).map_err(ParsingFailed)?;
            let tx = Tx::with_txid(TxId::from_tx(&tx), tx);
            let slp =
                SlpDbData::from_tx(self.db, self.mempool.slp(), &tx, |txid| {
                    self.mempool.tx(txid).is_some()
                })?;
            let slp = match slp {
                Some(slp) => slp,
                None => continue,
            };
            let (slpv1_burns, slpv2_burns) = slp.burns();
            let mut burn_msgs =
                Vec::with_capacity(slpv1_burns.len() + slpv2_burns.len());
            for burn in slpv1_burns.as_ref() {
                burn_msgs.push(burn.to_string());
            }
            for burn in slpv2_burns.as_ref() {
                if !burn.is_intentional() {
                    burn_msgs.push(burn.to_string());
                }
            }
            if let Protocol::Slpv2(tx_data) = slp.tx_data.as_ref() {
                if tx_data.sections.is_empty()
                    && tx_data.color_errors.is_empty()
                    && tx_data.burns.is_empty()
                {
                    if let Some(slpv1_error) = &slp.slpv1_error {
                        burn_msgs
                            .push(format!("SLP parse error: {}", slpv1_error));
                    }
                }
                for error in &tx_data.color_errors {
                    if !error.should_ignore() {
                        burn_msgs.push(format!("SLPv2 error: {}", error));
                    }
                }
            }
            if !burn_msgs.is_empty() {
                tx_errors.push((tx.txid(), burn_msgs))
            }
        }
        let mut slp_error = String::new();
        for (tx_idx, (txid, errors)) in tx_errors.iter().enumerate() {
            slp_error.push_str(&format!("Tx {} failed SLP checks: ", txid));
            for (error_idx, error) in errors.iter().enumerate() {
                slp_error.push_str(error);
                slp_error.push('.');
                if tx_idx != tx_errors.len() - 1
                    || error_idx != errors.len() - 1
                {
                    slp_error.push(' ');
                }
            }
        }
        if !tx_errors.is_empty() {
            return Err(SlpError(slp_error).into());
        }
        let mut txids = Vec::with_capacity(raw_txs.len());
        for raw_tx in raw_txs.iter() {
            txids.push(TxId::from(
                self.node
                    .bridge
                    .broadcast_tx(raw_tx, 1000000)
                    .map_err(|err| BroadcastFailed(err.to_string()))?,
            ));
        }
        Ok(txids)
    }
}
