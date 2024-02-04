//! Module for [`MempoolPlugins`].

use std::collections::BTreeMap;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, Tx, TxId};
use bitcoinsuite_slp::{token_tx::TokenTx, verify::SpentToken};
use chronik_plugin::{context::PluginContext, data::PluginOutput};
use chronik_util::log;
use thiserror::Error;

use crate::{
    db::Db,
    io::TxReader,
    mem::{MempoolGroupHistory, MempoolGroupUtxos, MempoolTx},
    plugins::{
        io::{DbOutpoint, PluginGroup, PluginsReader},
        mem::MempoolPluginsError::*,
    },
};

/// Token data of the mempool
#[derive(Debug)]
pub struct MempoolPlugins {
    plugin_outputs: BTreeMap<OutPoint, PluginOutput>,
    group_history: MempoolGroupHistory<PluginGroup>,
    group_utxo: MempoolGroupUtxos<PluginGroup>,
}

/// Error indicating something went wrong with [`MempoolTokens`].
#[derive(Debug, Eq, Error, PartialEq)]
pub enum MempoolPluginsError {
    /// Mempool tx spends outputs of non-existent tx
    #[error(
        "Failed indexing mempool token tx: Tx is spending {0} which is found \
         neither in the mempool nor DB"
    )]
    InputTxNotFound(TxId),
}

impl MempoolPlugins {
    /// Create a new [`MempoolPlugins`].
    pub fn new() -> Self {
        MempoolPlugins {
            plugin_outputs: BTreeMap::new(),
            group_history: MempoolGroupHistory::new(PluginGroup),
            group_utxo: MempoolGroupUtxos::new(PluginGroup),
        }
    }

    /// Run the tx against the plugins.
    pub fn insert(
        &mut self,
        db: &Db,
        tx: &MempoolTx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
        token_data: Option<(&TokenTx, &[Option<SpentToken>])>,
        plugin_ctx: &PluginContext,
    ) -> Result<()> {
        let mut plugin_outputs =
            self.fetch_plugin_outputs(&tx.tx, db, &is_mempool_tx)??;

        plugin_ctx.with_py(|py| -> Result<()> {
            log!("tx {} has outputs {plugin_outputs:?}\n", tx.tx.txid());
            let result = plugin_ctx.run_plugin_outputs(
                py,
                &tx.tx,
                &plugin_outputs,
                token_data,
            )?;

            for (output_idx, plugin_output) in result.outputs {
                plugin_outputs.insert(
                    OutPoint {
                        txid: tx.tx.txid(),
                        out_idx: output_idx as u32,
                    },
                    plugin_output.clone(),
                );
                self.plugin_outputs.insert(
                    OutPoint {
                        txid: tx.tx.txid(),
                        out_idx: output_idx as u32,
                    },
                    plugin_output,
                );
            }

            Ok(())
        })?;

        self.group_history.insert(tx, &self.plugin_outputs);
        self.group_utxo
            .insert(tx, &is_mempool_tx, &self.plugin_outputs)?;

        Ok(())
    }

    /// Remove a tx from the plugin mempool index
    pub fn remove(
        &mut self,
        tx: &MempoolTx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<()> {
        self.group_history.remove(tx, &self.plugin_outputs);
        self.group_utxo
            .remove(tx, &is_mempool_tx, &self.plugin_outputs)?;
        for output_idx in 0..tx.tx.outputs.len() {
            self.plugin_outputs.remove(&OutPoint {
                txid: tx.tx.txid(),
                out_idx: output_idx as u32,
            });
        }
        Ok(())
    }

    /// Remove a mined tx from the plugin mempool index
    pub fn remove_mined(&mut self, tx: &MempoolTx) {
        self.group_history.remove(tx, &self.plugin_outputs);
        self.group_utxo.remove_mined(tx, &self.plugin_outputs);
        for output_idx in 0..tx.tx.outputs.len() {
            self.plugin_outputs.remove(&OutPoint {
                txid: tx.tx.txid(),
                out_idx: output_idx as u32,
            });
        }
    }

    /// Fetch the plugin outputs either from the mempool or DB
    pub fn fetch_plugin_outputs(
        &self,
        tx: &Tx,
        db: &Db,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<Result<BTreeMap<OutPoint, PluginOutput>, MempoolPluginsError>>
    {
        let tx_reader = TxReader::new(db)?;
        let plugins_reader = PluginsReader::new(db)?;

        // The spent tokens we've found, all default to None
        let mut plugin_outputs = BTreeMap::<OutPoint, PluginOutput>::new();
        // TxNums for which we'll look up token data in the DB
        let mut input_db_outpoints = Vec::new();
        for input in tx.inputs.iter() {
            let input_txid = &input.prev_out.txid;
            // If we find the prevout in the mempool, set the PluginOutput
            if let Some(plugin_output) =
                self.plugin_outputs.get(&input.prev_out)
            {
                plugin_outputs.insert(input.prev_out, plugin_output.clone());
                continue;
            }

            // prevout is in the mempool but not a token tx
            if is_mempool_tx(input_txid) {
                continue;
            }

            // Otherwise, tx should be in the DB, query just its TxNum and store
            // it in input_db_outpoints, so we know which ones to fill in later.
            match tx_reader.tx_num_by_txid(input_txid)? {
                Some(tx_num) => {
                    input_db_outpoints.push((
                        input.prev_out,
                        DbOutpoint {
                            tx_num,
                            out_idx: input.prev_out.out_idx,
                        },
                    ));
                }
                None => return Ok(Err(InputTxNotFound(*input_txid))),
            }
        }

        let mut db_plugin_outputs =
            plugins_reader.plugin_outputs(input_db_outpoints)?;
        plugin_outputs.append(&mut db_plugin_outputs);

        Ok(Ok(plugin_outputs))
    }

    /// Get an individual outpoint from the mempool
    pub fn plugin_output(&self, outpoint: &OutPoint) -> Option<&PluginOutput> {
        self.plugin_outputs.get(outpoint)
    }

    /// Get a handle of the UTXOs in the mempool grouped by plugins
    pub fn utxos(&self) -> &MempoolGroupUtxos<PluginGroup> {
        &self.group_utxo
    }

    /// Get a handle of the txs in the mempool grouped by plugins
    pub fn history(&self) -> &MempoolGroupHistory<PluginGroup> {
        &self.group_history
    }
}

impl Default for MempoolPlugins {
    fn default() -> Self {
        MempoolPlugins::new()
    }
}
