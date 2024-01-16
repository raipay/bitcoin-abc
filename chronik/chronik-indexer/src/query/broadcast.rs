use abc_rust_error::Result;
use bitcoinsuite_core::{
    error::DataError,
    ser::BitcoinSer,
    tx::{Tx, TxId, TxMut},
};
use bytes::Bytes;
use chronik_bridge::ffi;
use chronik_db::{db::Db, mem::Mempool};
use chronik_proto::proto;
use thiserror::Error;

use crate::{
    avalanche::Avalanche,
    indexer::Node,
    query::{make_tx_proto, OutputsSpent, QueryBroadcastError::*, TxTokenData},
};

/// Struct for broadcasting txs on the network
#[derive(Debug)]
pub struct QueryBroadcast<'a> {
    /// Database
    pub db: &'a Db,
    /// Avalanche
    pub avalanche: &'a Avalanche,
    /// Mempool
    pub mempool: &'a Mempool,
    /// Access to bitcoind to actually broadcast txs
    pub node: &'a Node,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum QueryBroadcastError {
    /// Transaction not in mempool nor DB.
    #[error("400: Parsing tx failed {0}")]
    ParsingFailed(DataError),

    /// Token validation error that prevented us from broadcasting the tx
    #[error("400: {0}")]
    SlpError(String),

    /// Node rejected the tx
    #[error("400: Broadcast failed: {0}")]
    BroadcastFailed(String),
}

impl QueryBroadcast<'_> {
    /// Broadcast all the txs; if one fails token validation we don't broadcast
    /// any of them.
    pub fn broadcast_txs(&self, raw_txs: Vec<Bytes>) -> Result<Vec<TxId>> {
        let mut token_errors = Vec::new();
        let mut coins_to_uncache = Vec::new();
        for mut raw_tx in raw_txs.iter().cloned() {
            let tx = TxMut::deser(&mut raw_tx).map_err(ParsingFailed)?;
            let mut ffi_tx = ffi::Tx::from(tx);

            let mut tx_not_found = Vec::new();
            let mut tx_coins_to_uncache = Vec::new();
            self.node.bridge.lookup_spent_coins(&mut ffi_tx, &mut tx_not_found, &mut tx_coins_to_uncache)?;
            coins_to_uncache.extend(tx_coins_to_uncache);

            let tx = Tx::from(ffi_tx);
            let slp =
                TxTokenData::from_unbroadcast_tx(self.db, self.mempool, &tx)?;
            let slp = match slp {
                Some(slp) => slp,
                None => continue,
            };
            let mut burn_msgs = Vec::new();
            for failed_parsing in &slp.tx.failed_parsings {
                burn_msgs.push(failed_parsing.to_string());
            }
            for entry in &slp.tx.entries {
                if !entry.is_normal() {
                    burn_msgs.push(entry.burn_summary());
                }
            }
            if !burn_msgs.is_empty() {
                token_errors.push((tx.txid(), burn_msgs));
            }
        }
        let mut error_msg = String::new();
        for (tx_idx, (txid, errors)) in token_errors.iter().enumerate() {
            error_msg.push_str(&format!("Tx {} failed SLP checks: ", txid));
            for (error_idx, error) in errors.iter().enumerate() {
                error_msg.push_str(error);
                error_msg.push('.');
                if tx_idx != token_errors.len() - 1
                    || error_idx != errors.len() - 1
                {
                    error_msg.push(' ');
                }
            }
        }
        if !token_errors.is_empty() {
            return Err(SlpError(error_msg).into());
        }
        let mut txids = Vec::with_capacity(raw_txs.len());
        for raw_tx in raw_txs.iter() {
            txids.push(TxId::from(
                self.node.bridge.broadcast_tx(raw_tx, 1000000).or_else(
                    |err| -> Result<_> {
                        self.node.bridge.uncache_coins(&coins_to_uncache)?;
                        Err(BroadcastFailed(err.to_string()).into())
                    },
                )?,
            ));
        }
        Ok(txids)
    }

    /// Parse the tx and validate it as a token tx without broadcasting,
    /// returning verified tx data.
    pub fn validate_tx(&self, raw_tx: Vec<u8>) -> Result<proto::Tx> {
        let tx = TxMut::deser(&mut raw_tx.into())?;
        let mut ffi_tx = ffi::Tx::from(tx);
        let mut coins_to_uncache = Vec::new();
        self.node.bridge.lookup_spent_coins(&mut ffi_tx, &mut vec![], &mut coins_to_uncache)?;
        self.node.bridge.uncache_coins(&coins_to_uncache)?;
        let tx = Tx::from(ffi_tx);
        let slp = TxTokenData::from_unbroadcast_tx(self.db, self.mempool, &tx)?;
        Ok(make_tx_proto(
            &tx,
            &OutputsSpent::default(),
            0,
            false,
            None,
            self.avalanche,
            slp.as_ref(),
        ))
    }
}
