// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`Mempool`], to index mempool txs.

use std::collections::HashMap;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{Tx, TxId};
#[cfg(feature = "plugins")]
use chronik_plugin::context::PluginContext;
use thiserror::Error;

#[cfg(feature = "plugins")]
use crate::plugins::mem::MempoolPlugins;
use crate::{
    db::Db,
    groups::{MempoolScriptHistory, MempoolScriptUtxos, ScriptGroup},
    mem::{MempoolSpentBy, MempoolTokens},
};

/// Mempool of the indexer. This stores txs from the node again, but having a
/// copy here simplifies the implementation significantly. If this redundancy
/// becomes an issue (e.g. excessive RAM usage), we can optimize this later.
#[derive(Debug)]
pub struct Mempool {
    txs: HashMap<TxId, MempoolTx>,
    script_history: MempoolScriptHistory,
    script_utxos: MempoolScriptUtxos,
    spent_by: MempoolSpentBy,
    tokens: MempoolTokens,
    #[cfg(feature = "plugins")]
    plugins: MempoolPlugins,
}

/// Transaction in the mempool.
#[derive(Debug, PartialEq, Eq)]
pub struct MempoolTx {
    /// Transaction, including spent coins.
    pub tx: Tx,
    /// Time this tx has been added to the node's mempool.
    pub time_first_seen: i64,
}

/// Something went wrong with the mempool.
#[derive(Debug, Error)]
pub enum MempoolError {
    /// Tried removing a tx from the mempool that doesn't exist.
    #[error("No such mempool tx: {0}")]
    NoSuchMempoolTx(TxId),

    /// Tried adding a tx to the mempool that already exists.
    #[error("Tx {0} already exists in mempool")]
    DuplicateTx(TxId),
}

use self::MempoolError::*;

impl Mempool {
    /// Create a new [`Mempool`].
    pub fn new() -> Self {
        Mempool {
            txs: HashMap::new(),
            script_history: MempoolScriptHistory::new(),
            script_utxos: MempoolScriptUtxos::new(),
            spent_by: MempoolSpentBy::default(),
            tokens: MempoolTokens::default(),
            #[cfg(feature = "plugins")]
            plugins: MempoolPlugins::default(),
        }
    }

    /// Insert tx into the mempool.
    pub fn insert(
        &mut self,
        db: &Db,
        mempool_tx: MempoolTx,
        #[cfg(feature = "plugins")] plugin_ctx: &PluginContext,
    ) -> Result<()> {
        let txid = mempool_tx.tx.txid();
        self.script_history.insert(&mempool_tx, &ScriptGroup);
        self.script_utxos.insert(
            &mempool_tx,
            |txid| self.txs.contains_key(txid),
            &ScriptGroup,
        )?;
        self.spent_by.insert(&mempool_tx)?;
        self.tokens
            .insert(db, &mempool_tx, |txid| self.txs.contains_key(txid))?;
        #[cfg(feature = "plugins")]
        self.plugins.insert(
            db,
            &mempool_tx,
            |txid| self.txs.contains_key(txid),
            self.tokens
                .token_tx(mempool_tx.tx.txid_ref())
                .zip(self.tokens.tx_token_inputs(mempool_tx.tx.txid_ref())),
            plugin_ctx,
        )?;
        if self.txs.insert(txid, mempool_tx).is_some() {
            return Err(DuplicateTx(txid).into());
        }
        Ok(())
    }

    /// Remove tx from the mempool.
    pub fn remove(&mut self, txid: TxId) -> Result<MempoolTx> {
        let mempool_tx = match self.txs.remove(&txid) {
            Some(mempool_tx) => mempool_tx,
            None => return Err(NoSuchMempoolTx(txid).into()),
        };
        self.script_history.remove(&mempool_tx, &ScriptGroup);
        self.script_utxos.remove(
            &mempool_tx,
            |txid| self.txs.contains_key(txid),
            &ScriptGroup,
        )?;
        self.spent_by.remove(&mempool_tx)?;
        self.tokens.remove(&txid);
        #[cfg(feature = "plugins")]
        self.plugins
            .remove(&mempool_tx, |txid| self.txs.contains_key(txid))?;
        Ok(mempool_tx)
    }

    /// Remove mined tx from the mempool.
    pub fn remove_mined(&mut self, txid: &TxId) -> Result<Option<MempoolTx>> {
        if let Some(mempool_tx) = self.txs.remove(txid) {
            self.script_history.remove(&mempool_tx, &ScriptGroup);
            self.script_utxos.remove_mined(&mempool_tx, &ScriptGroup);
            self.spent_by.remove(&mempool_tx)?;
            self.tokens.remove(txid);
            #[cfg(feature = "plugins")]
            self.plugins.remove_mined(&mempool_tx);
            return Ok(Some(mempool_tx));
        }
        Ok(None)
    }

    /// Get a tx by [`TxId`], or [`None`], if not found.
    pub fn tx(&self, txid: &TxId) -> Option<&MempoolTx> {
        self.txs.get(txid)
    }

    /// Tx history of scripts in the mempool.
    pub fn script_history(&self) -> &MempoolScriptHistory {
        &self.script_history
    }

    /// Tx history of UTXOs in the mempool.
    pub fn script_utxos(&self) -> &MempoolScriptUtxos {
        &self.script_utxos
    }

    /// Which tx outputs have been spent by tx in the mempool.
    pub fn spent_by(&self) -> &MempoolSpentBy {
        &self.spent_by
    }

    /// Token data of txs in the mempool.
    pub fn tokens(&self) -> &MempoolTokens {
        &self.tokens
    }

    /// Token data of txs in the mempool.
    #[cfg(feature = "plugins")]
    pub fn plugins(&self) -> &MempoolPlugins {
        &self.plugins
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Mempool::new()
    }
}
