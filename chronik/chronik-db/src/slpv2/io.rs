use std::collections::{hash_map::Entry, BTreeMap, HashMap, HashSet};

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_core::{hash::Sha256d, tx::TxId};
use bitcoinsuite_slp::slpv2::{
    self, SectionData, SectionType, TokenId, TokenMeta, TxData,
};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use topo_sort::{SortResults, TopoSort};

use crate::{
    db::{
        Db, CF, CF_SLPV2_GENESIS_DATA, CF_SLPV2_TOKEN_META, CF_SLPV2_TX_DATA,
    },
    index_tx::IndexTx,
    io::TxNum,
    ser::{db_deserialize, db_serialize},
    slpv2::{
        batch::{BatchDbData, BatchProcessor},
        ser::{
            deser_genesis_data, deser_token_meta, deser_tx_data,
            ser_genesis_data, ser_token_meta, ser_tx_data,
        },
        structs::DbTxData,
    },
};

pub type TokenNum = u32;

struct Slpv2Col<'a> {
    db: &'a Db,
    cf_genesis_data: &'a CF,
    cf_token_meta: &'a CF,
    cf_tx_data: &'a CF,
}

pub struct Slpv2Writer<'a> {
    col: Slpv2Col<'a>,
}

pub struct Slpv2Reader<'a> {
    col: Slpv2Col<'a>,
}

#[derive(Debug, Error, PartialEq)]
pub enum Slpv2Error {
    /// TokenNum must be 4 bytes.
    #[error("Inconsistent DB: Invalid token_num bytes: {0:?}")]
    InvalidTokenNumBytes(Vec<u8>),

    #[error("Cycle in SLPv2 txs")]
    Cycle,

    #[error("Inconsistend DB: Token num not found")]
    TokenNumNotFound(TokenNum),
}

use self::Slpv2Error::*;

fn ser_tx_num(tx_num: TxNum) -> Result<Vec<u8>> {
    db_serialize(&tx_num)
}

pub(crate) fn token_num_to_bytes(token_num: TokenNum) -> [u8; 4] {
    token_num.to_be_bytes()
}

pub(crate) fn bytes_to_token_num(bytes: &[u8]) -> Result<TokenNum> {
    Ok(TokenNum::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| InvalidTokenNumBytes(bytes.to_vec()))?,
    ))
}

impl<'a> Slpv2Col<'a> {
    fn new(db: &'a Db) -> Result<Self> {
        let cf_genesis_data = db.cf(CF_SLPV2_GENESIS_DATA)?;
        let cf_token_meta = db.cf(CF_SLPV2_TOKEN_META)?;
        let cf_tx_data = db.cf(CF_SLPV2_TX_DATA)?;
        Ok(Slpv2Col {
            db,
            cf_genesis_data,
            cf_token_meta,
            cf_tx_data,
        })
    }

    fn fetch_token_meta(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<TokenMeta>> {
        match self
            .db
            .get(self.cf_token_meta, token_num_to_bytes(token_num))?
        {
            Some(token_meta) => Ok(Some(deser_token_meta(&token_meta)?)),
            None => Ok(None),
        }
    }

    fn get_or_fetch_token_id(
        &self,
        token_ids: &mut BiMap<TokenNum, TokenId>,
        token_num: TokenNum,
    ) -> Result<Option<TokenId>> {
        if let Some(entry) = token_ids.get_by_left(&token_num) {
            return Ok(Some(*entry));
        }
        match self.fetch_token_meta(token_num)? {
            Some(token_meta) => {
                token_ids.insert(token_num, token_meta.token_id);
                Ok(Some(token_meta.token_id))
            }
            None => Ok(None),
        }
    }

    fn get_tx_data(&self, tx_num: TxNum) -> Result<Option<DbTxData>> {
        match self.db.get(self.cf_tx_data, ser_tx_num(tx_num)?)? {
            Some(data) => Ok(Some(deser_tx_data(&data)?)),
            None => Ok(None),
        }
    }

    fn get_last_token_num(&self) -> Result<Option<TokenNum>> {
        let mut iter = self.db.iterator_end(self.cf_token_meta);
        match iter.next() {
            Some(result) => {
                let (key, _) = result?;
                Ok(Some(bytes_to_token_num(&key)?))
            }
            None => Ok(None),
        }
    }
}

impl<'a> Slpv2Writer<'a> {
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(Slpv2Writer {
            col: Slpv2Col::new(db)?,
        })
    }

    pub fn insert(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
    ) -> Result<()> {
        let last_token_num = self.col.get_last_token_num()?;
        let mut batch_processor = BatchProcessor::prepare(txs);
        if last_token_num.is_none() && !batch_processor.has_any_genesis() {
            // Short circuit: No tokens in the DB and no GENESIS in the batch
            return Ok(());
        }
        let mut db_data = BatchDbData {
            next_token_num: match last_token_num {
                Some(last_token_num) => last_token_num + 1,
                None => 0,
            },
            data: HashMap::new(),
            token_ids: BiMap::new(),
        };
        for tx in txs {
            for &input_tx_num in &tx.input_nums {
                if let Some(db_tx_data) = self.col.get_tx_data(input_tx_num)? {
                    for section in &db_tx_data.sections {
                        self.col.get_or_fetch_token_id(
                            &mut db_data.token_ids,
                            section.token_num,
                        )?;
                    }
                    db_data.data.insert(input_tx_num, db_tx_data);
                }
            }
        }
        let process_result = batch_processor.verify(&mut db_data)?;
        for (tx_num, db_tx_data) in process_result.new_tx_data {
            batch.put_cf(
                self.col.cf_tx_data,
                ser_tx_num(tx_num)?,
                ser_tx_data(&db_tx_data)?,
            );
        }
        for (token_num, meta, genesis_data) in process_result.new_tokens {
            batch.put_cf(
                self.col.cf_token_meta,
                token_num_to_bytes(token_num),
                ser_token_meta(&meta)?,
            );
            batch.put_cf(
                self.col.cf_genesis_data,
                token_num_to_bytes(token_num),
                ser_genesis_data(&genesis_data)?,
            );
        }
        Ok(())
    }

    pub fn delete(
        &self,
        batch: &mut WriteBatch,
        txs: &[IndexTx<'_>],
    ) -> Result<()> {
        for tx in txs {
            if let Some(db_tx_data) = self.col.get_tx_data(tx.tx_num)? {
                for section in &db_tx_data.sections {
                    if section.section_type == SectionType::GENESIS {
                        let ser_token_num =
                            token_num_to_bytes(section.token_num);
                        batch.delete_cf(self.col.cf_token_meta, ser_token_num);
                        batch
                            .delete_cf(self.col.cf_genesis_data, ser_token_num);
                    }
                }
                batch.delete_cf(self.col.cf_tx_data, ser_tx_num(tx.tx_num)?);
            }
        }
        Ok(())
    }

    pub fn clear(&self, batch: &mut WriteBatch) -> Result<()> {
        batch.delete_range_cf(self.col.cf_tx_data, b"".as_ref(), &[0xff; 16]);
        batch.delete_range_cf(self.col.cf_token_meta, b"".as_ref(), &[0xff; 16]);
        batch.delete_range_cf(self.col.cf_genesis_data, b"".as_ref(), &[0xff; 16]);
        Ok(())
    }

    /// Add the column families used for SLPv2.
    pub(crate) fn add_cfs(columns: &mut Vec<ColumnFamilyDescriptor>) {
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLPV2_GENESIS_DATA,
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLPV2_TOKEN_META,
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLPV2_TX_DATA,
            Options::default(),
        ));
    }
}

impl<'a> Slpv2Reader<'a> {
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(Slpv2Reader {
            col: Slpv2Col::new(db)?,
        })
    }

    pub fn tx_data_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<slpv2::TxData>> {
        match self.col.get_tx_data(tx_num)? {
            Some(db_tx_data) => Ok(Some(self.tx_data_from_db(&db_tx_data)?)),
            None => return Ok(None),
        }
    }

    pub fn tx_data_and_db_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<(slpv2::TxData, DbTxData)>> {
        match self.col.get_tx_data(tx_num)? {
            Some(db_tx_data) => {
                Ok(Some((self.tx_data_from_db(&db_tx_data)?, db_tx_data)))
            }
            None => return Ok(None),
        }
    }

    pub fn genesis_data_by_token_num(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<slpv2::GenesisData>> {
        match self
            .col
            .db
            .get(self.col.cf_genesis_data, token_num_to_bytes(token_num))?
        {
            Some(genesis_data) => Ok(Some(deser_genesis_data(&genesis_data)?)),
            None => return Ok(None),
        }
    }

    fn tx_data_from_db(&self, db_tx_data: &DbTxData) -> Result<TxData> {
        let mut sections = Vec::with_capacity(db_tx_data.sections.len());
        for section in &db_tx_data.sections {
            sections.push(SectionData {
                meta: self
                    .col
                    .fetch_token_meta(section.token_num)?
                    .ok_or(TokenNumNotFound(section.token_num))?,
                section_type: section.section_type,
                required_input_sum: section.required_input_sum,
            });
        }
        let mut burn_token_ids =
            Vec::with_capacity(db_tx_data.burn_token_nums.len());
        for &burn_token_num in &db_tx_data.burn_token_nums {
            burn_token_ids.push(
                self.col
                    .fetch_token_meta(burn_token_num)?
                    .ok_or(TokenNumNotFound(burn_token_num))?
                    .token_id,
            );
        }
        Ok(TxData {
            sections,
            burn_token_ids,
            inputs: db_tx_data.input_tokens.clone(),
            outputs: db_tx_data.output_tokens.clone(),
        })
    }
}
