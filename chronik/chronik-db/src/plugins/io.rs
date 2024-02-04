//! Module for [`PluginsWriter`] and [`PluginsReader`].

use std::{collections::BTreeMap, fmt::Debug};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::OutPoint;
use chronik_plugin::{context::PluginContext, data::PluginOutput};
use chronik_util::log;
use rocksdb::WriteBatch;
use serde::{Deserialize, Serialize};
use topo_sort::TopoSort;

use crate::{
    db::{
        Db, CF, CF_PLUGIN_GROUP_HISTORY, CF_PLUGIN_GROUP_HISTORY_NUM_TXS,
        CF_PLUGIN_GROUP_UTXOS, CF_PLUGIN_OUTPUTS,
    },
    group::{Group, GroupQuery, MemberItem, UtxoDataOutput},
    index_tx::IndexTx,
    io::{
        token::ProcessedTokenTxBatch, GroupHistoryConf, GroupHistoryMemData,
        GroupHistoryWriter, GroupUtxoConf, GroupUtxoMemData, GroupUtxoWriter,
        TxNum,
    },
    ser::{db_deserialize, db_serialize},
};

struct PluginsCol<'a> {
    db: &'a Db,
    cf_plugin_outputs: &'a CF,
}

/// Runs plugins and writes the results to the DB
#[derive(Debug)]
pub struct PluginsWriter<'a> {
    col: PluginsCol<'a>,
    ctx: &'a PluginContext,
}

/// Read data written by plugins from the DB.
#[derive(Debug)]
pub struct PluginsReader<'a> {
    col: PluginsCol<'a>,
}

/// Outpoint in the DB, but with [`TxNum`] instead of `TxId` for the txid.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub struct DbOutpoint {
    /// [`TxNum`] of tx of the outpoint.
    pub tx_num: TxNum,
    /// Output of the tx referenced by the outpoint.
    pub out_idx: u32,
}

/// Members we group outputs by; i.e. by plugin name first and then by the group
/// emitted by the plugin.
#[derive(Debug, Serialize)]
pub struct PluginMember<'a> {
    /// Name of the plugin that grouped the output
    pub plugin_name: &'a str,
    /// Group the plugin assigned to the output
    pub group: &'a [u8],
}

impl PluginMember<'_> {
    /// Serialize the member so we can use it as a key in the DB.
    pub fn ser(&self) -> Vec<u8> {
        db_serialize::<PluginMember<'_>>(self).unwrap()
    }
}

/// [`Group`] impl for plugins; groups outputs by the groups assigned by plugins
#[derive(Clone, Debug)]
pub struct PluginGroup;

impl Group for PluginGroup {
    type Aux = BTreeMap<OutPoint, PluginOutput>;
    type Iter<'a> = Vec<MemberItem<Vec<u8>>>;
    type Member<'a> = Vec<u8>;
    type MemberSer<'a> = Vec<u8>;
    type UtxoData = UtxoDataOutput;

    fn input_members<'a>(
        &self,
        query: GroupQuery<'a>,
        aux: &BTreeMap<OutPoint, PluginOutput>,
    ) -> Self::Iter<'a> {
        let mut members = Vec::new();
        for (input_idx, input) in query.tx.inputs.iter().enumerate() {
            let Some(plugin_output) = aux.get(&input.prev_out) else {
                continue;
            };
            for (plugin_name, entry) in &plugin_output.plugins {
                for group in &entry.groups {
                    members.push(MemberItem {
                        idx: input_idx,
                        member: db_serialize(&PluginMember {
                            plugin_name,
                            group,
                        })
                        .unwrap(),
                    });
                }
            }
        }
        members
    }

    fn output_members<'a>(
        &self,
        query: GroupQuery<'a>,
        aux: &BTreeMap<OutPoint, PluginOutput>,
    ) -> Self::Iter<'a> {
        let mut members = Vec::new();
        for output_idx in 0..query.tx.outputs.len() {
            let outpoint = OutPoint {
                txid: query.tx.txid(),
                out_idx: output_idx as u32,
            };
            let Some(plugin_output) = aux.get(&outpoint) else {
                continue;
            };
            for (plugin_name, entry) in &plugin_output.plugins {
                for group in &entry.groups {
                    members.push(MemberItem {
                        idx: output_idx,
                        member: db_serialize(&PluginMember {
                            plugin_name,
                            group,
                        })
                        .unwrap(),
                    });
                }
            }
        }
        members
    }

    fn ser_member<'a>(&self, member: &Vec<u8>) -> Self::MemberSer<'a> {
        member.to_vec()
    }

    fn tx_history_conf() -> GroupHistoryConf {
        GroupHistoryConf {
            cf_page_name: CF_PLUGIN_GROUP_HISTORY,
            cf_num_txs_name: CF_PLUGIN_GROUP_HISTORY_NUM_TXS,
            page_size: 1000,
        }
    }

    fn utxo_conf() -> GroupUtxoConf {
        GroupUtxoConf {
            cf_name: CF_PLUGIN_GROUP_UTXOS,
        }
    }
}

impl<'a> PluginsCol<'a> {
    fn new(db: &'a Db) -> Result<Self> {
        let cf_plugin_outputs = db.cf(CF_PLUGIN_OUTPUTS)?;
        Ok(PluginsCol {
            db,
            cf_plugin_outputs,
        })
    }

    fn fetch_plugin_outputs(
        &self,
        outpoints: impl IntoIterator<Item = (OutPoint, DbOutpoint)> + Clone,
    ) -> Result<BTreeMap<OutPoint, PluginOutput>> {
        let ser_outputs = self.db.multi_get(
            self.cf_plugin_outputs,
            outpoints.clone().into_iter().map(|(_, db_outpoint)| {
                db_serialize::<DbOutpoint>(&db_outpoint).unwrap()
            }),
            false,
        )?;
        let mut outputs = BTreeMap::new();
        for ((outpoint, _), ser_output) in
            outpoints.into_iter().zip(ser_outputs)
        {
            let Some(ser_output) = ser_output else {
                continue;
            };
            outputs
                .insert(outpoint, db_deserialize::<PluginOutput>(&ser_output)?);
        }
        Ok(outputs)
    }

    fn has_any_outputs(&self) -> Result<bool> {
        Ok(self
            .db
            .iterator(self.cf_plugin_outputs, b"", rocksdb::Direction::Forward)
            .next()
            .transpose()?
            .is_some())
    }
}

impl<'a> PluginsWriter<'a> {
    /// Create a new [`PluginsWriter`].
    pub fn new(db: &'a Db, ctx: &'a PluginContext) -> Result<Self> {
        Ok(PluginsWriter {
            col: PluginsCol::new(db)?,
            ctx,
        })
    }

    /// Run the plugin scripts write outputs and group them.
    pub fn insert(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
        processed_token_data: &ProcessedTokenTxBatch,
    ) -> Result<()> {
        let plugin_txs = txs
            .iter()
            .filter(|tx| self.ctx.tx_matches_any_plugin(tx.tx))
            .map(|tx| (tx.tx_num, tx))
            .collect::<BTreeMap<_, _>>();

        // Skip outputs
        if !self.col.has_any_outputs()? && plugin_txs.is_empty() {
            log!("Plugins: Skip block\n");
            return Ok(());
        }

        let mut plugin_outputs =
            self.col.fetch_plugin_outputs(txs.iter().flat_map(|tx| {
                tx.tx.inputs.iter().zip(&tx.input_nums).map(
                    |(input, &input_tx_num)| {
                        (
                            input.prev_out,
                            DbOutpoint {
                                tx_num: input_tx_num,
                                out_idx: input.prev_out.out_idx,
                            },
                        )
                    },
                )
            }))?;

        // Build a DAG of tx nums so we can sort topologically
        let mut topo_sort = TopoSort::with_capacity(plugin_txs.len());
        for (&tx_num, tx) in &plugin_txs {
            topo_sort.insert_from_slice(tx_num, &tx.input_nums);
        }

        self.ctx.with_py(|py| -> Result<_> {
            for tx_num in topo_sort.into_nodes() {
                let tx_num = tx_num?;
                let tx = plugin_txs[&tx_num];
                let token_tx = processed_token_data.valid_txs.get(&tx_num);
                let spent_tokens =
                    processed_token_data.spent_tokens.get(&tx_num);
                let result = self.ctx.run_plugin_outputs(
                    py,
                    tx.tx,
                    &plugin_outputs,
                    token_tx.zip(spent_tokens.map(|tokens| tokens.as_slice())),
                )?;
                log!("Plugin result: {result:?}\n");
                for (output_idx, plugin_output) in result.outputs {
                    let mut tx_data = PluginOutput::default();
                    for (plugin_name, entry) in plugin_output.plugins {
                        tx_data.plugins.insert(plugin_name.to_string(), entry);
                    }
                    batch.put_cf(
                        self.col.cf_plugin_outputs,
                        db_serialize::<DbOutpoint>(&DbOutpoint {
                            tx_num,
                            out_idx: output_idx as u32,
                        })?,
                        db_serialize::<PluginOutput>(&tx_data)?,
                    );
                    plugin_outputs.insert(
                        OutPoint {
                            txid: tx.tx.txid(),
                            out_idx: output_idx as u32,
                        },
                        tx_data,
                    );
                }
            }
            Ok(())
        })?;

        let plugin_history_writer =
            GroupHistoryWriter::new(self.col.db, PluginGroup)?;
        plugin_history_writer.insert(
            batch,
            txs,
            &plugin_outputs,
            &mut GroupHistoryMemData::default(),
        )?;

        let plugin_utxo_writer =
            GroupUtxoWriter::new(self.col.db, PluginGroup)?;
        plugin_utxo_writer.insert(
            batch,
            txs,
            &plugin_outputs,
            &mut GroupUtxoMemData::default(),
        )?;

        Ok(())
    }

    /// Delete the plugin data of a batch from the DB.
    pub fn delete(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
    ) -> Result<()> {
        let plugin_outputs =
            self.col.fetch_plugin_outputs(txs.iter().flat_map(|tx| {
                tx.tx
                    .inputs
                    .iter()
                    .zip(&tx.input_nums)
                    .map(|(input, &input_tx_num)| {
                        (
                            input.prev_out,
                            DbOutpoint {
                                tx_num: input_tx_num,
                                out_idx: input.prev_out.out_idx,
                            },
                        )
                    })
                    .chain((0..tx.tx.outputs.len()).map(|output_idx| {
                        (
                            OutPoint {
                                txid: tx.tx.txid(),
                                out_idx: output_idx as u32,
                            },
                            DbOutpoint {
                                tx_num: tx.tx_num,
                                out_idx: output_idx as u32,
                            },
                        )
                    }))
            }))?;

        let plugin_history_writer =
            GroupHistoryWriter::new(self.col.db, PluginGroup)?;
        plugin_history_writer.delete(
            batch,
            txs,
            &plugin_outputs,
            &mut GroupHistoryMemData::default(),
        )?;

        let plugin_utxo_writer =
            GroupUtxoWriter::new(self.col.db, PluginGroup)?;
        plugin_utxo_writer.delete(
            batch,
            txs,
            &plugin_outputs,
            &mut GroupUtxoMemData::default(),
        )?;

        for tx in txs {
            for output_idx in 0..tx.tx.outputs.len() {
                let outpoint = OutPoint {
                    txid: tx.tx.txid(),
                    out_idx: output_idx as u32,
                };
                let db_outpoint = DbOutpoint {
                    tx_num: tx.tx_num,
                    out_idx: output_idx as u32,
                };
                if plugin_outputs.contains_key(&outpoint) {
                    batch.delete_cf(
                        self.col.cf_plugin_outputs,
                        db_serialize::<DbOutpoint>(&db_outpoint)?,
                    );
                }
            }
        }

        Ok(())
    }

    pub(crate) fn add_cfs(columns: &mut Vec<rocksdb::ColumnFamilyDescriptor>) {
        columns.push(rocksdb::ColumnFamilyDescriptor::new(
            CF_PLUGIN_OUTPUTS,
            rocksdb::Options::default(),
        ));
        GroupHistoryWriter::<PluginGroup>::add_cfs(columns);
        GroupUtxoWriter::<PluginGroup>::add_cfs(columns);
    }
}

impl<'a> PluginsReader<'a> {
    /// Create a new [`PluginsReader`].
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(PluginsReader {
            col: PluginsCol::new(db)?,
        })
    }

    /// Read an individual outpoint from the DB.
    pub fn plugin_output(
        &self,
        outpoint: &DbOutpoint,
    ) -> Result<Option<PluginOutput>> {
        let ser_outpoint = db_serialize(outpoint)?;
        let Some(output) =
            self.col.db.get(self.col.cf_plugin_outputs, &ser_outpoint)?
        else {
            return Ok(None);
        };
        Ok(Some(db_deserialize::<PluginOutput>(&output)?))
    }

    /// Read all the given outpoints by [`DbOutpoint`] and return them as map by
    /// [`OutPoint`].
    pub fn plugin_outputs(
        &self,
        outpoints: impl IntoIterator<Item = (OutPoint, DbOutpoint)> + Clone,
    ) -> Result<BTreeMap<OutPoint, PluginOutput>> {
        self.col.fetch_plugin_outputs(outpoints)
    }
}

impl Debug for PluginsCol<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginsCol")
            .field("db", &self.db)
            .field("cf_plugin_outputs", &"..")
            .finish()
    }
}
