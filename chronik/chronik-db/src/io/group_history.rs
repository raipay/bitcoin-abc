// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

use std::{
    collections::BTreeMap, marker::PhantomData, num::NonZeroUsize,
    time::Instant,
};

use abc_rust_error::Result;
use chronik_util::{log, log_chronik};
use lru::LruCache;
use rocksdb::WriteBatch;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    db::{Db, CF},
    group::{tx_members_for_group, Group, GroupQuery},
    index_tx::IndexTx,
    io::{
        group_history::GroupHistoryError::*, merge::catch_merge_errors,
        BlockHeight, BlockReader, TxNum,
    },
    ser::{db_deserialize, db_deserialize_vec, db_serialize, db_serialize_vec},
};

/// Represent page numbers with 32-bit unsigned integers.
type PageNum = u32;
/// Represent num txs with 32-bit unsigned integers.
/// Note: This implies that scripts can at most have 2^32 txs.
type NumTxs = u32;

const CONCAT: u8 = b'C';
const TRIM: u8 = b'T';

const KEY_CACHE_BLOOM: &[u8] = b"bloom";

/// Configuration for group history reader/writers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GroupHistoryConf {
    /// Column family to store the group history pages.
    pub cf_page_name: &'static str,
    /// Column family to store the last page num of the group history.
    pub cf_num_txs_name: &'static str,
    /// Column family to store serialized cache data (e.g. bloom filter) of the
    /// group history.
    pub cf_cache_name: &'static str,
    /// Page size for each member of the group.
    pub page_size: NumTxs,
}

struct GroupHistoryColumn<'a> {
    db: &'a Db,
    cf_page: &'a CF,
    cf_num_txs: &'a CF,
    cf_cache: &'a CF,
}

/// Write txs grouped and paginated to the DB.
///
/// This is primarily meant to store the tx history of an address, but it can
/// also be used to index the history of other groups, especially of new
/// protocols.
///
/// Txs are stored paginated, because in the case of addresses, there already
/// exist addresses with millions of txs. While RocksDB can handle multi MB
/// entries, it would significantly slow down both reading and writing of this
/// address, which could pose a DoS risk.
///
/// Each page is stored at the key `<serialized member> + <4-byte page num>`
///
/// Txs in a member are ordered strictly ascendingly, both within a page, and
/// also between pages, such that the entire tx history of a member can be
/// iterated by going through pages 0..N and going through all of the txs of
/// each page.
///
/// Optionally, there is also a bloom filter speeding up indexing significantly,
/// which let's us know if a member is definitely not in the DB, and we can
/// assume the number of txs for this member is 0. This is a very common case
/// (most scripts are used only a few times, and one of them must be the first),
/// and in this case it allows us to skip reading from the DB completely and
/// just issue a merge_cf call right away.
///
/// We store the bloom filter at shutdown and load it at init. We have to make
/// 100% sure the bloom filter is valid and up-to-date with the rest of the DB,
/// because otherwise we don't have the guarantee that a script isn't in the DB
/// if it's not in the bloom filter. If that were to fail, we could insert the
/// history entry on the wrong page, causing all sorts of problems.
#[derive(Debug)]
pub struct GroupHistoryWriter<'a, G: Group> {
    col: GroupHistoryColumn<'a>,
    conf: GroupHistoryConf,
    group: G,
}

/// Read pages of grouped txs from the DB.
#[derive(Debug)]
pub struct GroupHistoryReader<'a, G: Group> {
    col: GroupHistoryColumn<'a>,
    conf: GroupHistoryConf,
    phantom: PhantomData<G>,
}

/// Settings for group history, e.g. whether to use a bloom filter or LRU cache.
#[derive(Clone, Debug, Default)]
pub struct GroupHistorySettings {
    /// Whether to use a bloom filter to determine if a member has any history
    pub is_bloom_filter_enabled: bool,
    /// Bloom filter false positive rate
    pub false_positive_rate: f64,
    /// Expected number of total distinct members of the group
    pub expected_num_items: usize,
    /// Number of items in the LRU cache for num txs per member
    pub cache_num_txs_size: usize,
}

/// In-memory data for the tx history.
#[derive(Debug, Default)]
pub struct GroupHistoryMemData {
    /// Stats about cache hits, num requests etc.
    pub stats: GroupHistoryStats,
    /// In-memory data to speed up indexing, e.g. LRU caches or bloom filters
    pub cache: GroupHistoryCache,
}

/// In-memory data to speed up indexing, e.g. LRU caches or bloom filters
#[derive(Default)]
pub struct GroupHistoryCache {
    bloom: Option<GroupHistoryBloomFilter>,
    cache_num_txs: Option<LruCache<Vec<u8>, NumTxs>>,
}

struct GroupHistoryBloomFilter {
    bloom_filter: bloomfilter::Bloom<[u8]>,
    false_positive_rate: f64,
    expected_num_items: usize,
}

/// Stats about cache hits, num requests etc.
#[derive(Clone, Debug, Default)]
pub struct GroupHistoryStats {
    /// Total number of members updated.
    pub n_total: usize,
    /// Num of total hits of the bloom filter
    pub n_bloom_hits: usize,
    /// Num of hits that turned out to be false positives
    pub n_bloom_false_positives: usize,
    /// Number of cache hits for the member's number of txs.
    pub n_num_txs_cache_hit: usize,
    /// Size of the bloom filter in bytes.
    pub n_bloom_num_bytes: usize,
    /// Number of entries fetched from the DB
    pub n_fetched: usize,
    /// Time [s] for insert/delete.
    pub t_total: f64,
    /// Time [s] for grouping txs.
    pub t_group: f64,
    /// Time [s] for serializing members.
    pub t_ser_members: f64,
    /// Time [s] for checking the bloom filter.
    pub t_bloom: f64,
    /// Time [s] for fetching existing tx data.
    pub t_fetch: f64,
    /// Time [s] for counting the set bits in the bloom filter at startup.
    pub t_init_num_bits: f64,
}

/// Error indicating that something went wrong with writing group history data.
#[derive(Debug, Error, PartialEq)]
pub enum GroupHistoryError {
    /// Bad num_txs size
    #[error("Inconsistent DB: Bad num_txs size: {0:?}")]
    BadNumTxsSize(Vec<u8>),

    /// Used merge_cf incorrectly, prefix must either be C or T.
    #[error(
        "Bad usage of merge: Unknown prefix {0:02x}, expected C or T: {}",
        hex::encode(.1),
    )]
    UnknownOperandPrefix(u8, Vec<u8>),

    /// Mismached bloom filter settings: configured <> DB
    #[error(
        "Mismached bloom filter settings: configured fpr \
         {configured_fp_rate}, DB has {db_fp_rate}; configured expected N \
         {configured_expected_n}, DB has {db_fp_rate}"
    )]
    MismatchedBloomFilterSettings {
        /// Configured FP rate
        configured_fp_rate: f64,
        /// FP rate in the DB
        db_fp_rate: f64,
        /// Configured expected N
        configured_expected_n: usize,
        /// Expected N in the DB
        db_expected_n: usize,
    },

    /// Mismatched bloom filter block height: DB <> bloom filter
    #[error(
        "Inconsistent group history bloom filter block height: DB is at \
         {db_height} but bloom filter is for {bloom_height}, consider wiping \
         the bloom filter or -chronikreindex to restore"
    )]
    MismatchedBloomFilterHeight {
        /// Block height of the DB (BlockWriter)
        db_height: BlockHeight,
        /// Block height in the existing bloom filter
        bloom_height: BlockHeight,
    },
}

enum BloomResult {
    Hit,
    HitCached,
    NoHit,
}

struct FetchedNumTxs<'tx, G: Group> {
    members_num_txs: Vec<(NumTxs, BloomResult)>,
    grouped_txs: BTreeMap<G::Member<'tx>, Vec<TxNum>>,
    ser_members: Vec<G::MemberSer<'tx>>,
}

#[derive(Serialize, Deserialize)]
struct SerBloomFilter {
    block_height: BlockHeight,
    bytes: Vec<u8>,
    bitmap_bits: u64,
    k_num: u32,
    sip_keys: [(u64, u64); 2],
    false_positive_rate: f64,
    expected_num_items: usize,
}

pub(crate) fn bytes_to_num_txs(bytes: &[u8]) -> Result<NumTxs> {
    Ok(NumTxs::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| BadNumTxsSize(bytes.to_vec()))?,
    ))
}

fn partial_merge_concat_trim(
    _key: &[u8],
    _existing_value: Option<&[u8]>,
    _operands: &rocksdb::MergeOperands,
) -> Option<Vec<u8>> {
    // We don't use partial merge
    None
}

fn init_concat_trim(
    _key: &[u8],
    existing_value: Option<&[u8]>,
    operands: &rocksdb::MergeOperands,
) -> Result<Vec<u8>> {
    let mut bytes = existing_value.unwrap_or(&[]).to_vec();
    if operands.iter().all(|operand| operand[0] == CONCAT) {
        bytes.reserve_exact(
            operands.iter().map(|operand| operand.len() - 1).sum(),
        );
    }
    Ok(bytes)
}

fn apply_concat_trim(
    _key: &[u8],
    bytes: &mut Vec<u8>,
    operand: &[u8],
) -> Result<()> {
    if operand[0] == CONCAT {
        bytes.extend_from_slice(&operand[1..]);
    } else if operand[0] == TRIM {
        let trim_len = NumTxs::from_be_bytes(operand[1..5].try_into().unwrap());
        bytes.drain(bytes.len() - trim_len as usize..);
    } else {
        return Err(UnknownOperandPrefix(operand[0], operand.to_vec()).into());
    }
    Ok(())
}

fn ser_concat_trim(_key: &[u8], bytes: Vec<u8>) -> Result<Vec<u8>> {
    Ok(bytes)
}

impl<'a> GroupHistoryColumn<'a> {
    fn new(db: &'a Db, conf: &GroupHistoryConf) -> Result<Self> {
        let cf_page = db.cf(conf.cf_page_name)?;
        let cf_num_txs = db.cf(conf.cf_num_txs_name)?;
        let cf_cache = db.cf(conf.cf_cache_name)?;
        Ok(GroupHistoryColumn {
            db,
            cf_page,
            cf_num_txs,
            cf_cache,
        })
    }

    fn get_page_txs(
        &self,
        member_ser: &[u8],
        page_num: PageNum,
    ) -> Result<Option<Vec<TxNum>>> {
        let key = key_for_member_page(member_ser, page_num);
        let value = match self.db.get(self.cf_page, &key)? {
            Some(value) => value,
            None => return Ok(None),
        };
        Ok(Some(db_deserialize_vec::<TxNum>(&value)?))
    }
}

impl<'a, G: Group> GroupHistoryWriter<'a, G> {
    /// Create a new [`GroupHistoryWriter`].
    pub fn new(db: &'a Db, group: G) -> Result<Self> {
        let conf = G::tx_history_conf();
        let col = GroupHistoryColumn::new(db, &conf)?;
        Ok(GroupHistoryWriter { col, conf, group })
    }

    /// Load cache data from the DB to `mem_data` at startup.
    /// For the bloom filter, this is important to get right otherwise we will
    /// get false negatives and a garbled history.
    pub fn init(&self, mem_data: &mut GroupHistoryMemData) -> Result<()> {
        let block_height = BlockReader::new(self.col.db)?.height()?;
        if let Some(ser_bloom_filter) =
            self.col.db.get(self.col.cf_cache, KEY_CACHE_BLOOM)?
        {
            let bloom_filter =
                db_deserialize::<SerBloomFilter>(&ser_bloom_filter)?;
            let t_num_bits = Instant::now();
            let num_bits = bloom_filter
                .bytes
                .iter()
                .map(|byte| byte.count_ones())
                .sum::<u32>();
            mem_data.stats.t_init_num_bits = t_num_bits.elapsed().as_secs_f64();
            if let Some(configured) = &mem_data.cache.bloom {
                if configured.bloom_filter.number_of_bits()
                    != bloom_filter.bitmap_bits
                    || configured.bloom_filter.number_of_hash_functions()
                        != bloom_filter.k_num
                {
                    return Err(MismatchedBloomFilterSettings {
                        configured_fp_rate: configured.false_positive_rate,
                        db_fp_rate: bloom_filter.false_positive_rate,
                        configured_expected_n: configured.expected_num_items,
                        db_expected_n: bloom_filter.expected_num_items,
                    }
                    .into());
                }
                if bloom_filter.block_height != block_height {
                    return Err(MismatchedBloomFilterHeight {
                        db_height: block_height,
                        bloom_height: bloom_filter.block_height,
                    }
                    .into());
                }
                log_chronik!(
                    "Loaded bloom filter for {:?}, with {num_bits} bits set\n",
                    self.conf.cf_page_name,
                );
                mem_data.cache.bloom = Some(GroupHistoryBloomFilter {
                    bloom_filter: bloomfilter::Bloom::from_existing(
                        &bloom_filter.bytes,
                        bloom_filter.bitmap_bits,
                        bloom_filter.k_num,
                        bloom_filter.sip_keys,
                    ),
                    false_positive_rate: configured.false_positive_rate,
                    expected_num_items: configured.expected_num_items,
                });
            } else {
                log!(
                    "Chronik: Ignoring existing bloom filter in DB (for block \
                     height {}, {} available bits, {} bits set, {:.1}% fpr, \
                     {} expected num items); will be wiped at shutdown\n",
                    bloom_filter.block_height,
                    bloom_filter.bitmap_bits,
                    num_bits,
                    bloom_filter.false_positive_rate * 100.0,
                    bloom_filter.expected_num_items,
                );
            }
        }
        Ok(())
    }

    /// Write cache data to the DB data at shutdown so we can restore it later.
    pub fn shutdown(&self, mem_data: &mut GroupHistoryMemData) -> Result<()> {
        let block_height = BlockReader::new(self.col.db)?.height()?;
        let mut batch = WriteBatch::default();
        if let Some(bloom) = &mem_data.cache.bloom {
            let bloom_filter = SerBloomFilter {
                block_height,
                bytes: bloom.bloom_filter.bitmap(),
                bitmap_bits: bloom.bloom_filter.number_of_bits(),
                k_num: bloom.bloom_filter.number_of_hash_functions(),
                sip_keys: bloom.bloom_filter.sip_keys(),
                false_positive_rate: bloom.false_positive_rate,
                expected_num_items: bloom.expected_num_items,
            };
            let ser_bloom_filter = db_serialize(&bloom_filter)?;

            batch.put_cf(self.col.cf_cache, KEY_CACHE_BLOOM, &ser_bloom_filter);
        } else {
            let bloom = self.col.db.get(self.col.cf_cache, KEY_CACHE_BLOOM)?;
            if bloom.is_some() {
                log!(
                    "Chronik: Deleting existing bloom filter for {:?}\n",
                    self.conf.cf_page_name,
                );
            }
            // Important: delete bloom filter from DB if unset to prevent false
            // negatives
            batch.delete_cf(self.col.cf_cache, KEY_CACHE_BLOOM);
        }
        self.col.db.write_batch(batch)?;
        Ok(())
    }

    /// Group the txs, then insert them to into each member of the group.
    pub fn insert(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
        aux: &G::Aux,
        mem_data: &mut GroupHistoryMemData,
    ) -> Result<()> {
        let t_start = Instant::now();
        let fetched = self.fetch_members_num_txs(txs, aux, mem_data)?;
        for ((mut new_tx_nums, member_ser), (mut num_txs, bloom_result)) in
            fetched
                .grouped_txs
                .into_values()
                .zip(fetched.ser_members)
                .zip(fetched.members_num_txs)
        {
            let mut page_num = num_txs / self.conf.page_size;
            let mut last_page_num_txs = num_txs % self.conf.page_size;
            loop {
                let space_left =
                    (self.conf.page_size - last_page_num_txs) as usize;
                let num_new_txs = space_left.min(new_tx_nums.len());
                let merge_tx_nums =
                    db_serialize_vec(new_tx_nums.drain(..num_new_txs))?;
                batch.merge_cf(
                    self.col.cf_page,
                    key_for_member_page(member_ser.as_ref(), page_num),
                    [[CONCAT].as_ref(), &merge_tx_nums].concat(),
                );
                num_txs += num_new_txs as NumTxs;
                if new_tx_nums.is_empty() {
                    batch.put_cf(
                        self.col.cf_num_txs,
                        member_ser.as_ref(),
                        num_txs.to_be_bytes(),
                    );
                    if matches!(bloom_result, BloomResult::NoHit) {
                        mem_data.cache.add_to_bloom_filter(member_ser.as_ref());
                    }
                    mem_data.cache.put_num_txs_cache(&member_ser, num_txs);
                    break;
                }
                last_page_num_txs = 0;
                page_num += 1;
            }
        }
        mem_data.stats.t_total += t_start.elapsed().as_secs_f64();
        Ok(())
    }

    /// Group the txs, then delete them from each member of the group.
    pub fn delete(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
        aux: &G::Aux,
        mem_data: &mut GroupHistoryMemData,
    ) -> Result<()> {
        let t_start = Instant::now();

        // On reorg, wipe bloom filter. This is because we cannot ever remove
        // elements from a (normal) bloom filter. In practice, the main
        // benefit for the bloom filter is during IBD, where no reorgs occur.
        // If tx volume picks up and reorgs wiping the bloom filter becomes a
        // problem, we can introduce checkpointing such that recent
        // bloom filters can be recovered. With the addition of Avalanche, the
        // deepest reorg depth should only ever be 1.
        mem_data.cache.bloom = None;

        let fetched = self.fetch_members_num_txs(txs, aux, mem_data)?;
        for ((mut removed_tx_nums, member_ser), (mut num_txs, _)) in fetched
            .grouped_txs
            .into_values()
            .zip(fetched.ser_members)
            .zip(fetched.members_num_txs)
        {
            let mut num_remaining_removes = removed_tx_nums.len();
            let mut page_num = num_txs / self.conf.page_size;
            let mut last_page_num_txs = num_txs % self.conf.page_size;
            loop {
                let num_page_removes =
                    (last_page_num_txs as usize).min(num_remaining_removes);
                let key = key_for_member_page(member_ser.as_ref(), page_num);
                if num_page_removes == last_page_num_txs as usize {
                    batch.delete_cf(self.col.cf_page, key)
                } else {
                    let merge_removed_txs = db_serialize_vec(
                        removed_tx_nums
                            .drain(removed_tx_nums.len() - num_page_removes..),
                    )?;
                    let num_trimmed_bytes = merge_removed_txs.len() as NumTxs;
                    batch.merge_cf(
                        self.col.cf_page,
                        key,
                        [[TRIM].as_ref(), &num_trimmed_bytes.to_be_bytes()]
                            .concat(),
                    );
                }
                num_txs -= num_page_removes as NumTxs;
                num_remaining_removes -= num_page_removes;
                if num_remaining_removes == 0 {
                    if num_txs > 0 {
                        batch.put_cf(
                            self.col.cf_num_txs,
                            member_ser.as_ref(),
                            num_txs.to_be_bytes(),
                        );
                    } else {
                        batch.delete_cf(
                            self.col.cf_num_txs,
                            member_ser.as_ref(),
                        );
                    }
                    mem_data.cache.put_num_txs_cache(member_ser, num_txs);
                    break;
                }
                if page_num > 0 {
                    page_num -= 1;
                    last_page_num_txs = self.conf.page_size;
                }
            }
        }
        mem_data.stats.t_total += t_start.elapsed().as_secs_f64();
        Ok(())
    }

    fn fetch_members_num_txs<'tx>(
        &self,
        txs: &'tx [IndexTx<'tx>],
        aux: &G::Aux,
        mem_data: &mut GroupHistoryMemData,
    ) -> Result<FetchedNumTxs<'tx, G>> {
        let GroupHistoryMemData { stats, cache } = mem_data;
        let t_group = Instant::now();
        let grouped_txs = self.group_txs(txs, aux);
        stats.t_group += t_group.elapsed().as_secs_f64();

        let t_ser_members = Instant::now();
        let ser_members = grouped_txs
            .keys()
            .map(|key| self.group.ser_member(key))
            .collect::<Vec<_>>();
        stats.t_ser_members += t_ser_members.elapsed().as_secs_f64();

        stats.n_total += grouped_txs.len();

        let t_bloom = Instant::now();
        let mut members_num_txs = Vec::with_capacity(ser_members.len());
        for member_ser in &ser_members {
            if cache.check_bloom_filter(member_ser.as_ref()) {
                stats.n_bloom_hits += 1;
                if let Some(entry) = cache.get_num_txs_cache(member_ser) {
                    stats.n_num_txs_cache_hit += 1;
                    members_num_txs.push((entry, BloomResult::HitCached));
                } else {
                    members_num_txs.push((0, BloomResult::Hit));
                }
            } else {
                members_num_txs.push((0, BloomResult::NoHit));
            }
        }
        stats.t_bloom += t_bloom.elapsed().as_secs_f64();

        let t_fetch = Instant::now();
        let num_txs_keys = ser_members.iter().zip(&members_num_txs).filter_map(
            |(member_ser, (_, bloom_result))| match bloom_result {
                BloomResult::Hit => Some(member_ser.as_ref()),
                BloomResult::HitCached => None,
                BloomResult::NoHit => None,
            },
        );
        let fetched_num_txs =
            self.col
                .db
                .multi_get(self.col.cf_num_txs, num_txs_keys, true)?;
        stats.n_fetched += fetched_num_txs.len();
        for ((member_num_txs, _), db_num_txs) in members_num_txs
            .iter_mut()
            .filter(|(_, bloom_result)| {
                matches!(bloom_result, BloomResult::Hit)
            })
            .zip(fetched_num_txs)
        {
            match db_num_txs {
                Some(db_num_txs) => {
                    *member_num_txs = bytes_to_num_txs(&db_num_txs)?;
                }
                None => {
                    stats.n_bloom_false_positives += 1;
                }
            }
        }
        stats.t_fetch += t_fetch.elapsed().as_secs_f64();

        Ok(FetchedNumTxs {
            members_num_txs,
            grouped_txs,
            ser_members,
        })
    }

    fn group_txs<'tx>(
        &self,
        txs: &'tx [IndexTx<'tx>],
        aux: &G::Aux,
    ) -> BTreeMap<G::Member<'tx>, Vec<TxNum>> {
        let mut group_tx_nums = BTreeMap::<G::Member<'tx>, Vec<TxNum>>::new();
        for index_tx in txs {
            let query = GroupQuery {
                is_coinbase: index_tx.is_coinbase,
                tx: index_tx.tx,
            };
            for member in tx_members_for_group(&self.group, query, aux) {
                let tx_nums = group_tx_nums.entry(member).or_default();
                if let Some(&last_tx_num) = tx_nums.last() {
                    if last_tx_num == index_tx.tx_num {
                        continue;
                    }
                }
                tx_nums.push(index_tx.tx_num);
            }
        }
        group_tx_nums
    }

    pub(crate) fn add_cfs(columns: &mut Vec<rocksdb::ColumnFamilyDescriptor>) {
        let conf = G::tx_history_conf();
        let mut page_options = rocksdb::Options::default();
        let merge_op_name = format!("{}::merge_op_concat", conf.cf_page_name);
        page_options.set_merge_operator(
            merge_op_name.as_str(),
            catch_merge_errors(
                init_concat_trim,
                apply_concat_trim,
                ser_concat_trim,
            ),
            partial_merge_concat_trim,
        );
        columns.push(rocksdb::ColumnFamilyDescriptor::new(
            conf.cf_page_name,
            page_options,
        ));
        columns.push(rocksdb::ColumnFamilyDescriptor::new(
            conf.cf_num_txs_name,
            rocksdb::Options::default(),
        ));
        columns.push(rocksdb::ColumnFamilyDescriptor::new(
            conf.cf_cache_name,
            rocksdb::Options::default(),
        ));
    }
}

impl GroupHistoryMemData {
    /// Create a new [`GroupHistoryMemData`] using the given
    /// [`GroupHistorySettings`].
    pub fn new(settings: GroupHistorySettings) -> Self {
        let mut stats = GroupHistoryStats::default();
        let cache_num_txs =
            NonZeroUsize::new(settings.cache_num_txs_size).map(LruCache::new);
        let cache = match settings.is_bloom_filter_enabled {
            false => GroupHistoryCache {
                bloom: None,
                cache_num_txs,
            },
            true => GroupHistoryCache {
                bloom: Some({
                    let bloom_filter = bloomfilter::Bloom::new_for_fp_rate(
                        settings.expected_num_items,
                        settings.false_positive_rate,
                    );
                    stats.n_bloom_num_bytes =
                        (bloom_filter.number_of_bits() as usize + 7) / 8;
                    GroupHistoryBloomFilter {
                        bloom_filter,
                        false_positive_rate: settings.false_positive_rate,
                        expected_num_items: settings.expected_num_items,
                    }
                }),
                cache_num_txs,
            },
        };
        GroupHistoryMemData { cache, stats }
    }
}

impl GroupHistoryCache {
    fn check_bloom_filter(&self, member_ser: &[u8]) -> bool {
        if let Some(bloom) = &self.bloom {
            bloom.bloom_filter.check(member_ser)
        } else {
            true
        }
    }

    fn add_to_bloom_filter(&mut self, member_ser: &[u8]) {
        if let Some(bloom) = &mut self.bloom {
            bloom.bloom_filter.set(member_ser);
        }
    }

    fn get_num_txs_cache(
        &self,
        member_ser: impl AsRef<[u8]>,
    ) -> Option<NumTxs> {
        // we can use peek here because put_num_txs_cache will be called later
        // anyway, updating the LRU position
        self.cache_num_txs
            .as_ref()?
            .peek(member_ser.as_ref())
            .copied()
    }

    fn put_num_txs_cache(
        &mut self,
        member_ser: impl AsRef<[u8]>,
        num_txs: NumTxs,
    ) {
        if let Some(cache) = &mut self.cache_num_txs {
            cache.put(member_ser.as_ref().to_vec(), num_txs);
        }
    }
}

impl std::fmt::Debug for GroupHistoryColumn<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroupHistoryColumn {{ .. }}")
    }
}

impl std::fmt::Debug for GroupHistoryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroupHistoryCache {{ .. }}")
    }
}

impl<'a, G: Group> GroupHistoryReader<'a, G> {
    /// Create a new [`GroupHistoryReader`].
    pub fn new(db: &'a Db) -> Result<Self> {
        let conf = G::tx_history_conf();
        let col = GroupHistoryColumn::new(db, &conf)?;
        Ok(GroupHistoryReader {
            col,
            conf,
            phantom: PhantomData,
        })
    }

    /// Read the tx_nums for the given member on the given page, or None, if the
    /// page doesn't exist in the DB.
    pub fn page_txs(
        &self,
        member_ser: &[u8],
        page_num: PageNum,
    ) -> Result<Option<Vec<TxNum>>> {
        self.col.get_page_txs(member_ser, page_num)
    }

    /// Total number of pages and txs for this serialized member.
    /// The result tuple is (num_pages, num_txs).
    pub fn member_num_pages_and_txs(
        &self,
        member_ser: &[u8],
    ) -> Result<(usize, usize)> {
        let num_txs = match self.col.db.get(self.col.cf_num_txs, member_ser)? {
            Some(bytes) => bytes_to_num_txs(&bytes)?,
            None => return Ok((0, 0)),
        };
        let num_pages =
            (num_txs + self.conf.page_size - 1) / self.conf.page_size;
        Ok((num_pages as usize, num_txs as usize))
    }

    /// Size of pages the data is stored in.
    pub fn page_size(&self) -> usize {
        self.conf.page_size as usize
    }
}

fn key_for_member_page(member_ser: &[u8], page_num: PageNum) -> Vec<u8> {
    [member_ser, &page_num.to_be_bytes()].concat()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use abc_rust_error::Result;
    use bitcoinsuite_core::tx::Tx;
    use rocksdb::WriteBatch;

    use crate::{
        db::Db,
        index_tx::prepare_indexed_txs,
        io::{
            group_history::PageNum,
            merge::{check_for_errors, MERGE_ERROR_LOCK},
            BlockTxs, GroupHistoryMemData, GroupHistoryReader,
            GroupHistoryWriter, TxEntry, TxNum, TxWriter, TxsMemData,
        },
        test::{make_value_tx, ser_value, ValueGroup},
    };

    #[test]
    fn test_value_group_history() -> Result<()> {
        let _guard = MERGE_ERROR_LOCK.lock().unwrap();
        abc_rust_error::install();
        let tempdir = tempdir::TempDir::new("chronik-db--group_history")?;
        let mut cfs = Vec::new();
        GroupHistoryWriter::<ValueGroup>::add_cfs(&mut cfs);
        TxWriter::add_cfs(&mut cfs);
        let db = Db::open_with_cfs(tempdir.path(), cfs)?;
        let tx_writer = TxWriter::new(&db)?;
        let group_writer = GroupHistoryWriter::new(&db, ValueGroup)?;
        let group_reader = GroupHistoryReader::<ValueGroup>::new(&db)?;
        let mem_data = RefCell::new(GroupHistoryMemData::default());
        let txs_mem_data = RefCell::new(TxsMemData::default());

        let block_height = RefCell::new(-1);
        let txs_batch = |txs: &[Tx]| BlockTxs {
            txs: txs
                .iter()
                .map(|tx| TxEntry {
                    txid: tx.txid(),
                    ..Default::default()
                })
                .collect(),
            block_height: *block_height.borrow(),
        };
        let connect_block = |txs: &[Tx]| -> Result<()> {
            let mut batch = WriteBatch::default();
            *block_height.borrow_mut() += 1;
            let first_tx_num = tx_writer.insert(
                &mut batch,
                &txs_batch(txs),
                &mut txs_mem_data.borrow_mut(),
            )?;
            let index_txs = prepare_indexed_txs(&db, first_tx_num, txs)?;
            group_writer.insert(
                &mut batch,
                &index_txs,
                &(),
                &mut mem_data.borrow_mut(),
            )?;
            db.write_batch(batch)?;
            Ok(())
        };
        let disconnect_block = |txs: &[Tx]| -> Result<()> {
            let mut batch = WriteBatch::default();
            let first_tx_num = tx_writer.delete(
                &mut batch,
                &txs_batch(txs),
                &mut txs_mem_data.borrow_mut(),
            )?;
            let index_txs = prepare_indexed_txs(&db, first_tx_num, txs)?;
            group_writer.delete(
                &mut batch,
                &index_txs,
                &(),
                &mut mem_data.borrow_mut(),
            )?;
            db.write_batch(batch)?;
            *block_height.borrow_mut() -= 1;
            Ok(())
        };

        let read_page =
            |val: i64, page_num: PageNum| -> Result<Option<Vec<TxNum>>> {
                group_reader.page_txs(&ser_value(val), page_num)
            };

        let read_num_pages_and_txs = |val: i64| -> Result<(usize, usize)> {
            group_reader.member_num_pages_and_txs(&ser_value(val))
        };

        // Only adds an entry for value=10 (coinbase inputs are ignored)
        let block0 = [make_value_tx(0, [0xffff], [10])];
        connect_block(&block0)?;
        assert_eq!(read_page(0xffff, 0)?, None);
        assert_eq!(read_num_pages_and_txs(0xffff)?, (0, 0));

        assert_eq!(read_page(10, 0)?, Some(vec![0]));
        assert_eq!(read_num_pages_and_txs(10)?, (1, 1));

        // Block that adds a lot of pages to value=10, one entry to value=20
        let block1 = [
            make_value_tx(1, [0xffff], [10]),
            make_value_tx(2, [10], []),
            make_value_tx(3, [20], []), // value=20
            make_value_tx(4, [10], []),
            make_value_tx(5, [10], []),
            make_value_tx(6, [10], []),
            make_value_tx(7, [10], []),
            make_value_tx(8, [10], []),
            make_value_tx(9, [10], []),
        ];
        connect_block(&block1)?;
        assert_eq!(read_page(0xffff, 0)?, None);
        assert_eq!(read_page(10, 0)?, Some(vec![0, 1, 2, 4]));
        assert_eq!(read_page(10, 1)?, Some(vec![5, 6, 7, 8]));
        assert_eq!(read_page(10, 2)?, Some(vec![9]));
        assert_eq!(read_page(10, 3)?, None);
        assert_eq!(read_num_pages_and_txs(10)?, (3, 9));

        assert_eq!(read_page(20, 0)?, Some(vec![3]));
        assert_eq!(read_page(20, 1)?, None);
        assert_eq!(read_num_pages_and_txs(20)?, (1, 1));

        // Only tx_num=0 remains
        // The other pages have been removed from the DB entirely
        disconnect_block(&block1)?;
        assert_eq!(read_page(0xffff, 0)?, None);
        assert_eq!(read_page(10, 0)?, Some(vec![0]));
        assert_eq!(read_page(10, 1)?, None);
        assert_eq!(read_page(10, 2)?, None);
        assert_eq!(read_page(20, 0)?, None);

        // Re-org block, with all kinds of input + output values
        let block1 = [
            make_value_tx(1, [0xffff], [10]),
            make_value_tx(2, [10], [10, 20, 30]),
            make_value_tx(3, [10, 40], [10, 10, 40]),
            make_value_tx(4, [10], [40, 30, 40]),
        ];
        connect_block(&block1)?;
        // all txs add to value=10, with 2 pages
        assert_eq!(read_page(10, 0)?, Some(vec![0, 1, 2, 3]));
        assert_eq!(read_page(10, 1)?, Some(vec![4]));
        assert_eq!(read_num_pages_and_txs(10)?, (2, 5));
        // only tx_num=2 adds to value=20
        assert_eq!(read_page(20, 0)?, Some(vec![2]));
        assert_eq!(read_num_pages_and_txs(20)?, (1, 1));
        // tx_num=2 and tx_num=4 add to value=30
        assert_eq!(read_page(30, 0)?, Some(vec![2, 4]));
        assert_eq!(read_num_pages_and_txs(30)?, (1, 2));
        // tx_num=3 and tx_num=4 add to value=40
        assert_eq!(read_page(40, 0)?, Some(vec![3, 4]));
        assert_eq!(read_num_pages_and_txs(40)?, (1, 2));

        // Delete that block also
        disconnect_block(&block1)?;
        assert_eq!(read_page(0xffff, 0)?, None);
        assert_eq!(read_page(10, 0)?, Some(vec![0]));
        assert_eq!(read_page(10, 1)?, None);
        assert_eq!(read_page(20, 0)?, None);
        assert_eq!(read_page(30, 0)?, None);
        assert_eq!(read_page(40, 0)?, None);

        // Add it back in
        connect_block(&block1)?;
        // Add new block, adding 1 tx to 20, 6 txs to 30, 4 txs to 40
        let block2 = [
            make_value_tx(5, [0xffff], [40, 30]),
            make_value_tx(6, [30, 10], [30]),
            make_value_tx(7, [10], [30]),
            make_value_tx(8, [40], [30]),
            make_value_tx(9, [10], [20]),
        ];
        connect_block(&block2)?;
        assert_eq!(read_page(10, 0)?, Some(vec![0, 1, 2, 3]));
        assert_eq!(read_page(10, 1)?, Some(vec![4, 6, 7, 9]));
        assert_eq!(read_page(10, 2)?, None);
        assert_eq!(read_num_pages_and_txs(10)?, (2, 8));

        assert_eq!(read_page(20, 0)?, Some(vec![2, 9]));
        assert_eq!(read_page(20, 1)?, None);
        assert_eq!(read_num_pages_and_txs(20)?, (1, 2));

        assert_eq!(read_page(30, 0)?, Some(vec![2, 4, 5, 6]));
        assert_eq!(read_page(30, 1)?, Some(vec![7, 8]));
        assert_eq!(read_page(30, 2)?, None);
        assert_eq!(read_num_pages_and_txs(30)?, (2, 6));

        assert_eq!(read_page(40, 0)?, Some(vec![3, 4, 5, 8]));
        assert_eq!(read_page(40, 1)?, None);
        assert_eq!(read_num_pages_and_txs(40)?, (1, 4));

        // Remove all blocks
        disconnect_block(&block2)?;
        assert_eq!(read_page(10, 0)?, Some(vec![0, 1, 2, 3]));
        assert_eq!(read_page(10, 1)?, Some(vec![4]));
        assert_eq!(read_page(20, 0)?, Some(vec![2]));
        assert_eq!(read_page(30, 0)?, Some(vec![2, 4]));
        assert_eq!(read_page(30, 1)?, None);
        assert_eq!(read_page(40, 0)?, Some(vec![3, 4]));
        assert_eq!(read_page(40, 1)?, None);

        disconnect_block(&block1)?;
        assert_eq!(read_page(10, 0)?, Some(vec![0]));
        assert_eq!(read_page(10, 1)?, None);
        assert_eq!(read_page(20, 0)?, None);
        assert_eq!(read_page(30, 0)?, None);
        assert_eq!(read_page(40, 0)?, None);

        disconnect_block(&block0)?;
        assert_eq!(read_page(10, 0)?, None);

        drop(db);
        rocksdb::DB::destroy(&rocksdb::Options::default(), tempdir.path())?;
        let _ = check_for_errors();

        Ok(())
    }
}
