// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

use crate::{
    index_tx::TxNumCache,
    io::{
        GroupHistoryMemData, GroupHistoryStats, GroupUtxoMemData,
        GroupUtxoStats, SpentByMemData, SpentByStats, TxsMemData, TxsStats,
    },
};

/// In-memory data for Chronik, e.g. caches, perf statistics.
#[derive(Debug)]
pub struct MemData {
    /// In-memory data for indexing txs.
    pub txs: TxsMemData,
    /// In-memory TxNumCache.
    pub tx_num_cache: TxNumCache,
    /// In-memory data for indexing script tx history.
    pub script_history: GroupHistoryMemData,
    /// In-memory data for indexing script UTXOs.
    pub script_utxos: GroupUtxoMemData,
    /// In-memory data for indexing spent-by data.
    pub spent_by: SpentByMemData,
}

/// Only the stats data from [`MemData`]
#[derive(Debug)]
pub struct StatsData {
    /// Stats data for indexing txs.
    pub txs: TxsStats,
    /// Stats data for indexing script tx history.
    pub script_history: GroupHistoryStats,
    /// Stats data for indexing script UTXOs.
    pub script_utxos: GroupUtxoStats,
    /// Stats data for indexing spent-by data.
    pub spent_by: SpentByStats,
}

/// Config for in-memory data for Chronik.
#[derive(Clone, Debug)]
pub struct MemDataConf {
    /// How many blocks deep the tx num cache goes.
    pub tx_num_cache_depth: usize,
    /// How big each tx num cache bucket can get.
    pub tx_num_cache_bucket_size: usize,
}

impl MemData {
    /// Create a new [`MemData`] from the given configuration.
    pub fn new(conf: MemDataConf) -> Self {
        MemData {
            txs: TxsMemData::default(),
            tx_num_cache: TxNumCache::new(
                conf.tx_num_cache_depth,
                conf.tx_num_cache_bucket_size,
            ),
            script_history: GroupHistoryMemData::default(),
            script_utxos: GroupUtxoMemData::default(),
            spent_by: SpentByMemData::default(),
        }
    }

    /// Only the stats data from this [`MemData`].
    pub fn stats(&self) -> StatsData {
        StatsData {
            txs: self.txs.stats.clone(),
            script_history: self.script_history.stats.clone(),
            script_utxos: self.script_utxos.stats.clone(),
            spent_by: self.spent_by.stats.clone(),
        }
    }
}
