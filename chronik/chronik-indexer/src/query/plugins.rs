//! Module for querying plugin generated data

use std::collections::BTreeMap;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, Tx};
use chronik_db::{
    db::Db,
    io::TxReader,
    mem::Mempool,
    plugins::{
        io::{DbOutpoint, PluginGroup, PluginMember, PluginsReader},
        mem::MempoolPlugins,
    },
};
use chronik_plugin::data::PluginOutput;
use chronik_proto::proto;

use crate::{
    avalanche::Avalanche,
    query::{QueryGroupHistory, QueryGroupUtxos, UtxoProtobufOutput},
};

/// Struct for querying indices created by plugins.
#[derive(Debug)]
pub struct QueryPlugins<'a> {
    /// Database
    pub db: &'a Db,
    /// Avalanche
    pub avalanche: &'a Avalanche,
    /// Mempool
    pub mempool: &'a Mempool,
    /// Whether the SLP/ALP token index is enabled
    pub is_token_index_enabled: bool,
}

impl<'a> QueryPlugins<'a> {
    /// Query the UTXOs a plugin has grouped for one member of the group
    pub fn utxos(
        &self,
        plugin_name: &str,
        group: &[u8],
    ) -> Result<Vec<proto::Utxo>> {
        let utxos: QueryGroupUtxos<'_, PluginGroup, UtxoProtobufOutput> =
            QueryGroupUtxos {
                db: self.db,
                avalanche: self.avalanche,
                mempool: self.mempool,
                mempool_utxos: self.mempool.plugins().utxos(),
                group: PluginGroup,
                utxo_mapper: UtxoProtobufOutput,
                is_token_index_enabled: self.is_token_index_enabled,
            };
        utxos.utxos(PluginMember { plugin_name, group }.ser())
    }

    /// Query the confirmed txs of a member of a group a plugin has grouped.
    pub fn confirmed_txs(
        &self,
        plugin_name: &str,
        group: &[u8],
        request_page_num: usize,
        request_page_size: usize,
    ) -> Result<proto::TxHistoryPage> {
        let utxos = QueryGroupHistory {
            db: self.db,
            avalanche: self.avalanche,
            mempool: self.mempool,
            mempool_history: self.mempool.plugins().history(),
            group: PluginGroup,
            is_token_index_enabled: self.is_token_index_enabled,
        };
        utxos.confirmed_txs(
            PluginMember { plugin_name, group }.ser(),
            request_page_num,
            request_page_size,
        )
    }

    /// Query the reverse history of a member of a group a plugin has grouped.
    pub fn rev_history(
        &self,
        plugin_name: &str,
        group: &[u8],
        request_page_num: usize,
        request_page_size: usize,
    ) -> Result<proto::TxHistoryPage> {
        let utxos = QueryGroupHistory {
            db: self.db,
            avalanche: self.avalanche,
            mempool: self.mempool,
            mempool_history: self.mempool.plugins().history(),
            group: PluginGroup,
            is_token_index_enabled: self.is_token_index_enabled,
        };
        utxos.rev_history(
            PluginMember { plugin_name, group }.ser(),
            request_page_num,
            request_page_size,
        )
    }

    /// Query the unconfirmed txs of a member of a group a plugin has grouped.
    pub fn unconfirmed_txs(
        &self,
        plugin_name: &str,
        group: &[u8],
    ) -> Result<proto::TxHistoryPage> {
        let utxos = QueryGroupHistory {
            db: self.db,
            avalanche: self.avalanche,
            mempool: self.mempool,
            mempool_history: self.mempool.plugins().history(),
            group: PluginGroup,
            is_token_index_enabled: self.is_token_index_enabled,
        };
        utxos.unconfirmed_txs(PluginMember { plugin_name, group }.ser())
    }
}

pub(crate) fn read_plugin_outputs(
    db: &Db,
    mempool: &MempoolPlugins,
    tx: &Tx,
) -> Result<BTreeMap<OutPoint, PluginOutput>> {
    let tx_reader = TxReader::new(db)?;
    let mut plugin_outputs = BTreeMap::new();
    let mut input_outpoints = Vec::new();
    for input in &tx.inputs {
        if let Some(plugin_output) = mempool.plugin_output(&input.prev_out) {
            plugin_outputs.insert(input.prev_out, plugin_output.clone());
            continue;
        }
        let Some(input_tx_num) =
            tx_reader.tx_num_by_txid(&input.prev_out.txid)?
        else {
            continue;
        };
        input_outpoints.push((
            input.prev_out,
            DbOutpoint {
                tx_num: input_tx_num,
                out_idx: input.prev_out.out_idx,
            },
        ));
    }
    let plugin_reader = PluginsReader::new(db)?;
    let tx_num = tx_reader.tx_num_by_txid(tx.txid_ref())?;
    if tx_num.is_none() {
        for out_idx in 0..tx.outputs.len() {
            let outpoint = OutPoint {
                txid: tx.txid(),
                out_idx: out_idx as u32,
            };
            if let Some(output) = mempool.plugin_output(&outpoint) {
                plugin_outputs.insert(outpoint, output.clone());
            }
        }
    }
    let mut db_plugin_outputs =
        plugin_reader.plugin_outputs(input_outpoints.into_iter().chain(
            tx_num.into_iter().flat_map(|tx_num| {
                (0..tx.outputs.len()).map(move |out_idx| {
                    (
                        OutPoint {
                            txid: tx.txid(),
                            out_idx: out_idx as u32,
                        },
                        DbOutpoint {
                            tx_num,
                            out_idx: out_idx as u32,
                        },
                    )
                })
            }),
        ))?;
    plugin_outputs.append(&mut db_plugin_outputs);
    Ok(plugin_outputs)
}
