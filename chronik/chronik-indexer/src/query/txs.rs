// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`QueryTxs`], to query txs from mempool/db.

use abc_rust_error::{Result, WrapErr};
use bitcoinsuite_core::{
    ser::BitcoinSer,
    tx::{OutPoint, Tx, TxId, TxMut, Coin},
};
use bitcoinsuite_slp::slpv2;
use bytes::Bytes;
use chronik_bridge::ffi;
use chronik_db::{
    db::Db,
    io::{BlockReader, SpentByReader, TxReader},
    mem::Mempool,
    slpv2::io::Slpv2Reader, slp::io::SlpReader,
};
use chronik_proto::proto;
use thiserror::Error;

use crate::{
    avalanche::Avalanche,
    query::{make_tx_proto, validate_slpv2_tx, OutputsSpent, validate_slp_tx},
};

/// Struct for querying txs from the db/mempool.
#[derive(Debug)]
pub struct QueryTxs<'a> {
    /// Database
    pub db: &'a Db,
    /// Avalanche
    pub avalanche: &'a Avalanche,
    /// Mempool
    pub mempool: &'a Mempool,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum QueryTxError {
    /// Transaction not in mempool nor DB.
    #[error("404: Transaction {0} not found in the index")]
    TxNotFound(TxId),

    #[error("404: Transaction {0} is not an SLPv2 GENESIS")]
    TxNotSlpv2Genesis(TxId),

    /// Transaction in DB without block
    #[error("500: Inconsistent DB: {0} has no block")]
    DbTxHasNoBlock(TxId),

    /// Reading failed, likely corrupted block data
    #[error("500: Reading {0} failed")]
    ReadFailure(TxId),

    ///
    #[error("400: Missing input: {0:?}")]
    MissingInput(OutPoint),
}

use self::QueryTxError::*;

impl<'a> QueryTxs<'a> {
    /// Query a tx by txid from the mempool or DB.
    pub fn tx_by_id(&self, txid: TxId) -> Result<proto::Tx> {
        match self.mempool.tx(&txid) {
            Some(tx) => {
                let (tx_data, _) = validate_slpv2_tx(&tx.tx, self.mempool, self.db)?;
                Ok(make_tx_proto(
                    &tx.tx,
                    &OutputsSpent::new_mempool(
                        self.mempool.spent_by().outputs_spent(&txid),
                    ),
                    tx.time_first_seen,
                    false,
                    None,
                    self.avalanche,
                    validate_slp_tx(&tx.tx, self.mempool, self.db)?,
                    Some(&tx_data),
                ))
            },
            None => {
                let tx_reader = TxReader::new(self.db)?;
                let (tx_num, block_tx) = tx_reader
                    .tx_and_num_by_txid(&txid)?
                    .ok_or(TxNotFound(txid))?;
                let tx_entry = block_tx.entry;
                let block_reader = BlockReader::new(self.db)?;
                let spent_by_reader = SpentByReader::new(self.db)?;
                let block = block_reader
                    .by_height(block_tx.block_height)?
                    .ok_or(DbTxHasNoBlock(txid))?;
                let tx = ffi::load_tx(
                    block.file_num,
                    tx_entry.data_pos,
                    tx_entry.undo_pos,
                )
                .wrap_err(ReadFailure(txid))?;
                let tx = Tx::from(tx);
                let outputs_spent = OutputsSpent::query(
                    &spent_by_reader,
                    &tx_reader,
                    self.mempool.spent_by().outputs_spent(&txid),
                    tx_num,
                )?;
                let (tx_data, _) = validate_slpv2_tx(&tx, self.mempool, self.db)?;
                Ok(make_tx_proto(
                    &tx,
                    &outputs_spent,
                    tx_entry.time_first_seen,
                    tx_entry.is_coinbase,
                    Some(&block),
                    self.avalanche,
                    validate_slp_tx(&tx, self.mempool, self.db)?,
                    Some(&tx_data),
                ))
            }
        }
    }

    /// Query the raw serialized tx by txid.
    ///
    /// Serializes the tx if it's in the mempool, or reads the tx data from the
    /// node's storage otherwise.
    pub fn raw_tx_by_id(&self, txid: &TxId) -> Result<proto::RawTx> {
        let raw_tx = match self.mempool.tx(txid) {
            Some(mempool_tx) => mempool_tx.tx.ser().to_vec(),
            None => {
                let tx_reader = TxReader::new(self.db)?;
                let block_reader = BlockReader::new(self.db)?;
                let block_tx =
                    tx_reader.tx_by_txid(txid)?.ok_or(TxNotFound(*txid))?;
                let block = block_reader
                    .by_height(block_tx.block_height)?
                    .ok_or(DbTxHasNoBlock(*txid))?;
                ffi::load_raw_tx(block.file_num, block_tx.entry.data_pos)
                    .wrap_err(ReadFailure(*txid))?
            }
        };
        Ok(proto::RawTx { raw_tx })
    }

    pub fn validate_tx(&self, raw_tx: Vec<u8>) -> Result<proto::Tx> {
        let mut bytes = Bytes::from(raw_tx);
        let mut tx = TxMut::deser(&mut bytes)?;
        for input in tx.inputs.iter_mut() {
            if let Some(input_tx) = self.mempool.tx(&input.prev_out.txid) {
                let output = input_tx
                    .tx
                    .outputs
                    .get(input.prev_out.out_idx as usize)
                    .ok_or(MissingInput(input.prev_out))?;
                input.coin = Some(Coin {
                    output: output.clone(),
                    height: -1,
                    is_coinbase: false,
                });
                continue;
            }
            let tx_reader = TxReader::new(self.db)?;
            let block_reader = BlockReader::new(self.db)?;
            let block_tx = tx_reader
                .tx_by_txid(&input.prev_out.txid)?
                .ok_or(MissingInput(input.prev_out))?;
            let block = block_reader
                .by_height(block_tx.block_height)?
                .ok_or(DbTxHasNoBlock(input.prev_out.txid))?;
            let input_tx = ffi::load_tx(
                block.file_num,
                block_tx.entry.data_pos,
                block_tx.entry.undo_pos,
            )
            .wrap_err(ReadFailure(input.prev_out.txid))?;
            let input_tx = Tx::from(input_tx);
            let output = input_tx
                .outputs
                .get(input.prev_out.out_idx as usize)
                .ok_or(MissingInput(input.prev_out))?;
            input.coin = Some(Coin {
                output: output.clone(),
                height: block.height,
                is_coinbase: block_tx.entry.is_coinbase,
            });
        }
        // TODO: Don't use "0000...0000" txid
        let tx = Tx::with_txid(TxId::default(), tx);
        let (slpv2_tx_data, _) = validate_slpv2_tx(&tx, self.mempool, self.db)?;
        Ok(make_tx_proto(
            &tx,
            &OutputsSpent::default(),
            0,
            false,
            None,
            self.avalanche,
            validate_slp_tx(&tx, self.mempool, self.db)?,
            Some(&slpv2_tx_data),
        ))
    }

    pub fn slpv2_token_info(
        &self,
        token_id: &slpv2::TokenId,
    ) -> Result<proto::Slpv2TokenInfo> {
        let genesis_txid = TxId::from(token_id.to_bytes());
        match self.mempool.tx(&genesis_txid) {
            Some(mempool_tx) => {
                let tx_data = self
                    .mempool
                    .slpv2()
                    .tx_data(&genesis_txid)
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                let section = tx_data
                    .sections
                    .get(0)
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                let genesis_data = self
                    .mempool
                    .slpv2()
                    .genesis_data(&genesis_txid)
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                Ok(proto::Slpv2TokenInfo {
                    token_id: token_id.to_vec(),
                    token_type: match section.meta.token_type {
                        slpv2::TokenType::Standard => {
                            proto::Slpv2TokenType::Standard as _
                        }
                    },
                    genesis_data: Some(proto::Slpv2GenesisData {
                        token_ticker: genesis_data.token_ticker.to_vec(),
                        token_name: genesis_data.token_name.to_vec(),
                        url: genesis_data.url.to_vec(),
                        data: genesis_data.data.to_vec(),
                        auth_pubkey: genesis_data.auth_pubkey.to_vec(),
                        decimals: genesis_data.decimals as u32,
                    }),
                    block: None,
                    time_first_seen: mempool_tx.time_first_seen,
                })
            }
            None => {
                let block_reader = BlockReader::new(self.db)?;
                let tx_reader = TxReader::new(self.db)?;
                let slpv2_reader = Slpv2Reader::new(self.db)?;

                let (tx_num, block_tx) = tx_reader
                    .tx_and_num_by_txid(&genesis_txid)?
                    .ok_or(TxNotFound(genesis_txid))?;
                let (tx_data, db_tx_data) = slpv2_reader
                    .tx_data_and_db_by_tx_num(tx_num)?
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                let section = tx_data
                    .sections
                    .get(0)
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                let db_section = db_tx_data
                    .sections
                    .get(0)
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                if section.section_type != slpv2::SectionType::GENESIS {
                    return Err(TxNotSlpv2Genesis(genesis_txid).into());
                }
                let genesis_data = slpv2_reader
                    .genesis_data_by_token_num(db_section.token_num)?
                    .ok_or(TxNotSlpv2Genesis(genesis_txid))?;
                let block = block_reader
                    .by_height(block_tx.block_height)?
                    .ok_or(DbTxHasNoBlock(genesis_txid))?;
                Ok(proto::Slpv2TokenInfo {
                    token_id: token_id.to_vec(),
                    token_type: match section.meta.token_type {
                        slpv2::TokenType::Standard => {
                            proto::Slpv2TokenType::Standard as _
                        }
                    },
                    genesis_data: Some(proto::Slpv2GenesisData {
                        token_ticker: genesis_data.token_ticker.to_vec(),
                        token_name: genesis_data.token_name.to_vec(),
                        url: genesis_data.url.to_vec(),
                        data: genesis_data.data.to_vec(),
                        auth_pubkey: genesis_data.auth_pubkey.to_vec(),
                        decimals: genesis_data.decimals as u32,
                    }),
                    block: Some(proto::BlockMetadata {
                        hash: block.hash.to_vec(),
                        height: block.height,
                        timestamp: block.timestamp,
                        is_final: self.avalanche.is_final_height(block.height),
                    }),
                    time_first_seen: block_tx.entry.time_first_seen,
                })
            }
        }
    }
}
