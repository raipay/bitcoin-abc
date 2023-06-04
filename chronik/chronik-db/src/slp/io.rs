use std::collections::HashMap;

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_slp::{slp, slpv2};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch};
use thiserror::Error;

use crate::{
    db::{Db, CF, CF_SLP_GENESIS_INFO, CF_SLP_TOKEN_META, CF_SLP_TX_DATA},
    index_tx::IndexTx,
    io::TxNum,
    ser::{db_deserialize, db_serialize},
    slp::{
        batch::{BatchDbData, BatchProcessor},
        data::{DbTxData, EitherMeta, Protocol, FLAGS_HAS_GENESIS},
    },
};

pub type TokenNum = u32;

struct Slpv2Col<'a> {
    db: &'a Db,
    cf_genesis_info: &'a CF,
    cf_token_meta: &'a CF,
    cf_tx_data: &'a CF,
}

pub struct SlpWriter<'a> {
    col: Slpv2Col<'a>,
}

pub struct SlpReader<'a> {
    col: Slpv2Col<'a>,
}

#[derive(Debug, Error, PartialEq)]
pub enum Slpv2Error {
    /// TokenNum must be 4 bytes.
    #[error("Inconsistent DB: Invalid token_num bytes: {0:?}")]
    InvalidTokenNumBytes(Vec<u8>),

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
        let cf_genesis_info = db.cf(CF_SLP_GENESIS_INFO)?;
        let cf_token_meta = db.cf(CF_SLP_TOKEN_META)?;
        let cf_tx_data = db.cf(CF_SLP_TX_DATA)?;
        Ok(Slpv2Col {
            db,
            cf_genesis_info,
            cf_token_meta,
            cf_tx_data,
        })
    }

    fn fetch_token_meta(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<EitherMeta>> {
        match self
            .db
            .get(self.cf_token_meta, token_num_to_bytes(token_num))?
        {
            Some(token_meta) => {
                Ok(Some(db_deserialize::<EitherMeta>(&token_meta)?))
            }
            None => Ok(None),
        }
    }

    fn get_or_fetch_token_meta(
        &self,
        token_metas: &mut BiMap<TokenNum, EitherMeta>,
        token_num: TokenNum,
    ) -> Result<Option<EitherMeta>> {
        if let Some(entry) = token_metas.get_by_left(&token_num) {
            return Ok(Some(*entry));
        }
        match self.fetch_token_meta(token_num)? {
            Some(token_meta) => {
                token_metas.insert(token_num, token_meta);
                Ok(Some(token_meta))
            }
            None => Ok(None),
        }
    }

    fn get_tx_data(&self, tx_num: TxNum) -> Result<Option<DbTxData>> {
        match self.db.get(self.cf_tx_data, ser_tx_num(tx_num)?)? {
            Some(data) => Ok(Some(db_deserialize::<DbTxData>(&data)?)),
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
            token_metas: BiMap::new(),
        };
        for tx in txs {
            for &input_tx_num in &tx.input_nums {
                if let Some(db_tx_data) = self.col.get_tx_data(input_tx_num)? {
                    for &token_num in &db_tx_data.token_nums {
                        self.col.get_or_fetch_token_meta(
                            &mut db_data.token_metas,
                            token_num,
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
                db_serialize(&db_tx_data)?,
            );
        }
        for (token_num, new_token) in process_result.new_tokens {
            let (meta, genesis_info) = match new_token {
                Protocol::Slp((meta, info)) => {
                    (Protocol::Slp(meta), Protocol::Slp(info))
                }
                Protocol::Slpv2((meta, info)) => {
                    (Protocol::Slpv2(meta), Protocol::Slpv2(info))
                }
            };
            batch.put_cf(
                self.col.cf_token_meta,
                token_num_to_bytes(token_num),
                db_serialize::<EitherMeta>(&meta)?,
            );
            batch.put_cf(
                self.col.cf_genesis_info,
                token_num_to_bytes(token_num),
                db_serialize::<Protocol<slp::GenesisInfo, slpv2::GenesisInfo>>(
                    &genesis_info,
                )?,
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
                if (db_tx_data.flags & FLAGS_HAS_GENESIS) != 0 {
                    let token_num = db_tx_data.token_nums[0];
                    let ser_token_num = token_num_to_bytes(token_num);
                    batch.delete_cf(self.col.cf_token_meta, ser_token_num);
                    batch.delete_cf(self.col.cf_genesis_info, ser_token_num);
                }
                batch.delete_cf(self.col.cf_tx_data, ser_tx_num(tx.tx_num)?);
            }
        }
        Ok(())
    }

    pub fn clear(&self, batch: &mut WriteBatch) -> Result<()> {
        batch.delete_range_cf(self.col.cf_tx_data, b"".as_ref(), &[0xff; 16]);
        batch.delete_range_cf(
            self.col.cf_token_meta,
            b"".as_ref(),
            &[0xff; 16],
        );
        batch.delete_range_cf(
            self.col.cf_genesis_info,
            b"".as_ref(),
            &[0xff; 16],
        );
        Ok(())
    }

    /// Add the column families used for SLPv2.
    pub(crate) fn add_cfs(columns: &mut Vec<ColumnFamilyDescriptor>) {
        columns.push(ColumnFamilyDescriptor::new(
            CF_SLP_GENESIS_INFO,
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
        /*columns.push(ColumnFamilyDescriptor::new(
            "slpv2_tx_data",
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            "slpv2_token_meta",
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            "slpv2_genesis_data",
            Options::default(),
        ));
        columns.push(ColumnFamilyDescriptor::new(
            "slp_genesis_data",
            Options::default(),
        ));*/
    }
}

impl<'a> SlpReader<'a> {
    pub fn new(db: &'a Db) -> Result<Self> {
        Ok(SlpReader {
            col: Slpv2Col::new(db)?,
        })
    }

    pub fn token_meta_by_token_num(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<EitherMeta>> {
        self.col.fetch_token_meta(token_num)
    }

    pub fn tx_data_by_tx_num(&self, tx_num: TxNum) -> Result<Option<DbTxData>> {
        self.col.get_tx_data(tx_num)
    }

    pub fn token_metas_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<Vec<EitherMeta>>> {
        match self.col.get_tx_data(tx_num)? {
            Some(db_tx_data) => {
                Ok(Some(self.token_metas_from_db(&db_tx_data)?))
            }
            None => return Ok(None),
        }
    }

    pub fn token_metas_and_db_by_tx_num(
        &self,
        tx_num: TxNum,
    ) -> Result<Option<(Vec<EitherMeta>, DbTxData)>> {
        match self.col.get_tx_data(tx_num)? {
            Some(db_tx_data) => {
                Ok(Some((self.token_metas_from_db(&db_tx_data)?, db_tx_data)))
            }
            None => return Ok(None),
        }
    }

    pub fn genesis_data_by_token_num(
        &self,
        token_num: TokenNum,
    ) -> Result<Option<slpv2::GenesisInfo>> {
        match self
            .col
            .db
            .get(self.col.cf_genesis_info, token_num_to_bytes(token_num))?
        {
            Some(genesis_data) => {
                Ok(Some(db_deserialize::<slpv2::GenesisInfo>(&genesis_data)?))
            }
            None => Ok(None),
        }
    }

    fn token_metas_from_db(
        &self,
        db_tx_data: &DbTxData,
    ) -> Result<Vec<EitherMeta>> {
        db_tx_data
            .token_nums
            .iter()
            .map(|&token_num| -> Result<_> {
                Ok(self
                    .col
                    .fetch_token_meta(token_num)?
                    .ok_or(TokenNumNotFound(token_num))?)
            })
            .collect::<Result<Vec<_>>>()
    }
}
