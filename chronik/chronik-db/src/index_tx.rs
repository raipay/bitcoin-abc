// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`IndexTx`] and [`prepare_indexed_txs`].

use std::collections::{hash_map::Entry, BTreeSet, HashMap, VecDeque};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, Tx, TxId};
use thiserror::Error;

use crate::{
    db::Db,
    index_tx::IndexTxError::*,
    io::{TxNum, TxReader},
};

/// Tx in a block to be added to the index, with prepared data to guide
/// indexing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexTx<'a> {
    /// Tx from the node to be indexed.
    pub tx: &'a Tx,
    /// [`TxNum`] of this tx.
    pub tx_num: TxNum,
    /// Whether this tx is a coinbase tx.
    pub is_coinbase: bool,
    /// [`TxNum`]s of the inputs of the tx. Either references tx from the DB or
    /// other txs within the new block.
    ///
    /// Empty for coinbase txs.
    pub input_nums: Vec<TxNum>,
}

/// Cache for tx nums, stores the latest `depth_blocks` blocks of txids
#[derive(Debug, Default)]
pub struct TxNumCache {
    depth_blocks: usize,
    last_blocks: VecDeque<HashMap<TxId, TxNum>>,
}

/// Error indicating something went wrong with a [`IndexTx`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexTxError {
    /// An input references a txid which could neither be found in the DB nor
    /// within the new block.
    #[error("Unknown input spent: {0:?}")]
    UnknownInputSpent(OutPoint),
}

/// Mode [`prepare_indexed_txs_cached`] is used for, to ensure the cache is
/// updated correctly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareUpdateMode {
    /// Txs are being added to the DB
    Add,
    /// Txs are being removed from the DB
    Delete,
    /// Txs are only read from the DB
    Read,
}

/// Prepare txs of a block which is about to be added/removed from the DB with
/// some additional data, either coming from the DB or from within the block.
/// This function works like [`prepare_indexed_txs_cached`] but with the cache
/// disabled.
pub fn prepare_indexed_txs<'a>(
    db: &Db,
    first_tx_num: TxNum,
    txs: &'a [Tx],
) -> Result<Vec<IndexTx<'a>>> {
    prepare_indexed_txs_cached(
        db,
        first_tx_num,
        txs,
        &mut TxNumCache::default(),
        PrepareUpdateMode::Read,
    )
}

/// Prepare txs of a block which is about to be added/removed from the DB with
/// some additional data, either coming from the DB or from within the block.
/// Uses the provided [`TxNumCache`], and puts the new TxNums into it.
pub fn prepare_indexed_txs_cached<'a>(
    db: &Db,
    first_tx_num: TxNum,
    txs: &'a [Tx],
    cache: &mut TxNumCache,
    update_mode: PrepareUpdateMode,
) -> Result<Vec<IndexTx<'a>>> {
    let mut tx_nums_by_txid = HashMap::with_capacity(txs.len());
    for (tx_idx, tx) in txs.iter().enumerate() {
        tx_nums_by_txid.insert(tx.txid_ref(), first_tx_num + tx_idx as TxNum);
    }
    let mut db_txids = BTreeSet::new();
    for tx in txs {
        for tx_input in &tx.inputs {
            if let Entry::Vacant(entry) =
                tx_nums_by_txid.entry(&tx_input.prev_out.txid)
            {
                if let Some(tx_num) = cache.get(&tx_input.prev_out.txid) {
                    entry.insert(tx_num);
                    continue;
                }
                db_txids.insert(&tx_input.prev_out.txid);
            }
        }
    }
    let tx_reader = TxReader::new(db)?;
    let db_tx_nums = tx_reader.tx_nums_by_txids(db_txids.iter().copied())?;
    let db_txids = db_txids.into_iter().collect::<Vec<_>>();
    let index_txs = txs
        .iter()
        .enumerate()
        .map(|(tx_idx, tx)| {
            let tx_num = first_tx_num + tx_idx as TxNum;
            let is_coinbase = tx_idx == 0;
            let input_nums = if is_coinbase {
                vec![]
            } else {
                tx.inputs
                    .iter()
                    .map(|input| {
                        Ok(match tx_nums_by_txid.get(&input.prev_out.txid) {
                            Some(&tx_num) => tx_num,
                            None => {
                                let tx_num_idx = db_txids
                                    .binary_search(&&input.prev_out.txid)
                                    .map_err(|_| {
                                        UnknownInputSpent(input.prev_out)
                                    })?;
                                db_tx_nums[tx_num_idx]
                                    .ok_or(UnknownInputSpent(input.prev_out))?
                            }
                        })
                    })
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(IndexTx {
                tx,
                tx_num,
                is_coinbase,
                input_nums,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    match update_mode {
        PrepareUpdateMode::Add => cache.add_to_cache(&index_txs),
        PrepareUpdateMode::Delete => cache.pop_latest(),
        PrepareUpdateMode::Read => {}
    }
    Ok(index_txs)
}

impl TxNumCache {
    /// Create a cache for tx nums caching the latest `depth_blocks` txs.
    pub fn new(depth_blocks: usize) -> TxNumCache {
        TxNumCache {
            depth_blocks,
            last_blocks: VecDeque::with_capacity(depth_blocks),
        }
    }

    fn add_to_cache(&mut self, index_txs: &[IndexTx<'_>]) {
        if self.depth_blocks == 0 {
            return;
        }
        if self.last_blocks.len() >= self.depth_blocks {
            self.last_blocks.pop_back();
        }
        let latest_block = index_txs
            .iter()
            .map(|tx| (tx.tx.txid(), tx.tx_num))
            .collect::<HashMap<_, _>>();
        self.last_blocks.push_front(latest_block);
    }

    fn pop_latest(&mut self) {
        self.last_blocks.pop_front();
    }

    fn get(&self, txid: &TxId) -> Option<u64> {
        for block in &self.last_blocks {
            if let Some(&tx_num) = block.get(txid) {
                return Some(tx_num);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use abc_rust_error::Result;
    use bitcoinsuite_core::tx::{OutPoint, Tx, TxId, TxInput, TxMut};
    use pretty_assertions::assert_eq;
    use rocksdb::WriteBatch;

    use crate::{
        db::Db,
        index_tx::{prepare_indexed_txs, IndexTx},
        io::{BlockTxs, TxEntry, TxNum, TxWriter, TxsMemData},
    };

    #[test]
    fn test_prepare_indexed_txs() -> Result<()> {
        abc_rust_error::install();
        let tempdir = tempdir::TempDir::new("chronik-db--indexed_txs")?;
        let db = Db::open(tempdir.path())?;
        let tx_writer = TxWriter::new(&db)?;
        let mut txs_mem_data = TxsMemData::default();
        let mut batch = WriteBatch::default();
        let db_txs = (100..110)
            .map(|txid_num: u8| TxEntry {
                txid: TxId::from([txid_num; 32]),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let first_tx_num = db_txs.len() as TxNum;
        tx_writer.insert(
            &mut batch,
            &BlockTxs {
                txs: db_txs,
                block_height: 0,
            },
            &mut txs_mem_data,
        )?;
        db.write_batch(batch)?;

        fn make_tx<const N: usize>(
            txid_num: u8,
            input_txid_nums: [u8; N],
        ) -> Tx {
            Tx::with_txid(
                TxId::from([txid_num; 32]),
                TxMut {
                    inputs: input_txid_nums
                        .into_iter()
                        .map(|input_txid_num| TxInput {
                            prev_out: OutPoint {
                                txid: TxId::from([input_txid_num; 32]),
                                out_idx: 0,
                            },
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                },
            )
        }

        let new_txs = vec![
            make_tx(110, [0]),
            make_tx(111, [110, 100, 101]),
            make_tx(112, [111, 109, 103]),
        ];

        assert_eq!(
            prepare_indexed_txs(&db, first_tx_num, &new_txs)?,
            vec![
                IndexTx {
                    tx: &new_txs[0],
                    tx_num: 10,
                    is_coinbase: true,
                    input_nums: vec![],
                },
                IndexTx {
                    tx: &new_txs[1],
                    tx_num: 11,
                    is_coinbase: false,
                    input_nums: vec![10, 0, 1],
                },
                IndexTx {
                    tx: &new_txs[2],
                    tx_num: 12,
                    is_coinbase: false,
                    input_nums: vec![11, 9, 3],
                },
            ],
        );

        Ok(())
    }
}
