//! Module for [`MempoolPlugins`].

use std::collections::BTreeMap;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, Tx, TxId};
use bitcoinsuite_slp::{token_tx::TokenTx, verify::SpentToken};
use chronik_plugin::{context::PluginContext, pyo3::Python};
use thiserror::Error;

use crate::{
    db::Db,
    io::TxReader,
    mem::{MempoolGroupHistory, MempoolGroupUtxos, MempoolTx},
    plugins::{
        data::{PluginOutput, PluginOutputEntry},
        io::{DbOutpoint, PluginGroup, PluginsReader},
        mem::MempoolPluginsError::*,
    },
};

/// Token data of the mempool
#[derive(Debug, Default)]
pub struct MempoolPlugins {
    plugin_outputs: BTreeMap<OutPoint, PluginOutput>,
    group_history: MempoolGroupHistory,
    group_utxo: MempoolGroupUtxos,
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
    /// Parse, color and verify a potential token tx.
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

        Python::with_gil(|py| -> Result<()> {
            let result =
                plugin_ctx.run_plugin_outputs(py, &tx.tx, token_data)?;

            for (output_idx, plugin_output) in result.outputs {
                let mut tx_data = PluginOutput::default();
                for (plugin_name, plugin_output) in plugin_output {
                    tx_data.plugins.insert(
                        plugin_name.to_string(),
                        PluginOutputEntry {
                            groups: plugin_output
                                .groups
                                .iter()
                                .map(|group| group.to_vec())
                                .collect(),
                            data: plugin_output
                                .data
                                .iter()
                                .map(|data| data.to_vec())
                                .collect(),
                        },
                    );
                }
                plugin_outputs.insert(
                    OutPoint {
                        txid: tx.tx.txid(),
                        out_idx: output_idx as u32,
                    },
                    tx_data.clone(),
                );
                self.plugin_outputs.insert(
                    OutPoint {
                        txid: tx.tx.txid(),
                        out_idx: output_idx as u32,
                    },
                    tx_data,
                );
            }

            Ok(())
        })?;

        let group = PluginGroup {
            plugin_outputs: &plugin_outputs,
        };
        self.group_history.insert(tx, &group);
        self.group_utxo.insert(tx, &is_mempool_tx, &group)?;

        Ok(())
    }

    /// Remove a tx from the plugin mempool index
    pub fn remove(
        &mut self,
        tx: &MempoolTx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<()> {
        let group = PluginGroup {
            plugin_outputs: &self.plugin_outputs,
        };
        self.group_history.remove(tx, &group);
        self.group_utxo.remove(tx, &is_mempool_tx, &group)?;
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
        let group = PluginGroup {
            plugin_outputs: &self.plugin_outputs,
        };
        self.group_history.remove(tx, &group);
        self.group_utxo.remove_mined(tx, &group);
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
    pub fn utxos(&self) -> &MempoolGroupUtxos {
        &self.group_utxo
    }

    /// Get a handle of the txs in the mempool grouped by plugins
    pub fn history(&self) -> &MempoolGroupHistory {
        &self.group_history
    }
}
