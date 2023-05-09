use std::collections::{hash_map::Entry, BTreeMap, HashMap, HashSet};

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_core::{hash::Sha256d, tx::TxId};
use bitcoinsuite_slp::slp::{self, TxTypeVariant};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use topo_sort::{SortResults, TopoSort};

use crate::{
    db::{Db, CF, CF_SLP_GENESIS_DATA, CF_SLP_TOKEN_META, CF_SLP_TX_DATA},
    index_tx::IndexTx,
    io::TxNum,
    ser::{db_deserialize, db_serialize},
    slp::{
        batch::{BatchDbData, BatchProcessor},
        ser::{
            deser_genesis_data, deser_token_meta, deser_tx_data,
            ser_genesis_data, ser_token_meta, ser_tx_data,
        },
        structs::DbTxData,
    },
};

pub type TokenNum = u32;

struct SlpCol<'a> {
    db: &'a Db,
    cf_genesis_data: &'a CF,
    cf_token_meta: &'a CF,
    cf_tx_data: &'a CF,
}

pub struct SlpWriter<'a> {
    col: SlpCol<'a>,
}

pub struct SlpReader<'a> {
    col: SlpCol<'a>,
}

#[derive(Debug, Error, PartialEq)]
pub enum SlpError {
    /// TokenNum must be 4 bytes.
    #[error("Inconsistent DB: Invalid token_num bytes: {0:?}")]
    InvalidTokenNumBytes(Vec<u8>),

    #[error("Cycle in SLPv2 txs")]
    Cycle,

    #[error("Inconsistend DB: Token num not found")]
    TokenNumNotFound(TokenNum),
}

use self::SlpError::*;

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

impl<'a> SlpCol<'a> {
    fn new(db: &'a Db) -> Result<Self> {
        let cf_genesis_data = db.cf(CF_SLP_GENESIS_DATA)?;
        let cf_token_meta = db.cf(CF_SLP_TOKEN_META)?;
        let cf_tx_data = db.cf(CF_SLP_TX_DATA)?;
        Ok(SlpCol {
            db,
            cf_genesis_data,
            cf_token_meta,
            cf_tx_data,
        })
    }

    fn fetch_token_meta(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<slp::TokenMeta>> {
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
        token_ids: &mut BiMap<TokenNum, slp::TokenId>,
        token_num: TokenNum,
    ) -> Result<Option<slp::TokenId>> {
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

impl<'a> SlpWriter<'a> {
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(SlpWriter {
            col: SlpCol::new(db)?,
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
                    self.col.get_or_fetch_token_id(
                        &mut db_data.token_ids,
                        db_tx_data.token_num,
                    )?;
                    if let Some(group_token_num) = db_tx_data.group_token_num {
                        self.col.get_or_fetch_token_id(
                            &mut db_data.token_ids,
                            group_token_num,
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
                if db_tx_data.tx_type == TxTypeVariant::Genesis {
                    let ser_token_num =
                        token_num_to_bytes(db_tx_data.token_num);
                    batch.delete_cf(self.col.cf_token_meta, ser_token_num);
                    batch.delete_cf(self.col.cf_genesis_data, ser_token_num);
                }
                batch.delete_cf(self.col.cf_tx_data, ser_tx_num(tx.tx_num)?);
            }
        }
        Ok(())
    }

    /// Add the column families used for SLPv2.
    pub(crate) fn add_cfs(columns: &mut Vec<ColumnFamilyDescriptor>) {
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLP_GENESIS_DATA,
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLP_TOKEN_META,
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLP_TX_DATA,
            Options::default(),
        ));
    }
}

impl<'a> SlpReader<'a> {
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(SlpReader {
            col: SlpCol::new(db)?,
        })
    }

    pub fn tx_data_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<slp::TxData>> {
        match self.col.get_tx_data(tx_num)? {
            Some(db_tx_data) => Ok(Some(self.tx_data_from_db(&db_tx_data)?)),
            None => return Ok(None),
        }
    }

    pub fn tx_data_and_db_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<(slp::TxData, DbTxData)>> {
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
    ) -> Result<Option<slp::GenesisInfo>> {
        match self
            .col
            .db
            .get(self.col.cf_genesis_data, token_num_to_bytes(token_num))?
        {
            Some(genesis_data) => Ok(Some(deser_genesis_data(&genesis_data)?)),
            None => return Ok(None),
        }
    }

    fn tx_data_from_db(&self, db_tx_data: &DbTxData) -> Result<slp::TxData> {
        let token_id = self
            .col
            .fetch_token_meta(db_tx_data.token_num)?
            .ok_or(TokenNumNotFound(db_tx_data.token_num))?
            .token_id;
        let group_token_id = match db_tx_data.group_token_num {
            Some(group_token_num) => Some(Box::new(
                self.col
                    .fetch_token_meta(group_token_num)?
                    .ok_or(TokenNumNotFound(group_token_num))?
                    .token_id,
            )),
            None => None,
        };
        let mut burns = Vec::with_capacity(db_tx_data.burns.len());
        for burn in &db_tx_data.burns {
            burns.push(match burn {
                Some(burn) => Some(Box::new(slp::Burn {
                    token: burn.token,
                    token_id: self
                        .col
                        .fetch_token_meta(burn.token_num)?
                        .ok_or(TokenNumNotFound(burn.token_num))?
                        .token_id,
                })),
                None => None,
            });
        }
        Ok(slp::TxData {
            token_id,
            token_type: db_tx_data.token_type,
            tx_type: db_tx_data.tx_type,
            input_tokens: db_tx_data.input_tokens.clone(),
            output_tokens: db_tx_data.output_tokens.clone(),
            slp_burns: burns,
            group_token_id,
        })
    }
}
