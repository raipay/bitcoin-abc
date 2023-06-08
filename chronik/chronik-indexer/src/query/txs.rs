// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`QueryTxs`], to query txs from mempool/db.

use abc_rust_error::{Result, WrapErr};
use bitcoinsuite_core::{
    hash::{Hashed, Sha256d},
    ser::BitcoinSer,
    tx::{Coin, Tx, TxId, TxMut},
};
use bitcoinsuite_slp::{slp, slpv2};
use chronik_bridge::ffi;
use chronik_db::{
    db::Db,
    io::{BlockReader, SpentByReader, TxReader},
    mem::Mempool,
    slp::data::Protocol,
};
use chronik_proto::proto;
use thiserror::Error;

use crate::{
    avalanche::Avalanche,
    query::{make_tx_proto, token_info, OutputsSpent, SlpDbData},
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

    /// Token not found in mempool nor DB.
    #[error("404: Token {0} not found in the index")]
    TokenNotFound(TxId),

    /// Transaction not in mempool nor DB.
    #[error("400: Input tx {0} not found")]
    InputTxNotFound(TxId),

    /// Transaction in DB without block
    #[error("500: Inconsistent DB: {0} has no block")]
    DbTxHasNoBlock(TxId),

    /// Reading failed, likely corrupted block data
    #[error("500: Reading {0} failed")]
    ReadFailure(TxId),
}

use self::QueryTxError::*;

impl<'a> QueryTxs<'a> {
    /// Query a tx by txid from the mempool or DB.
    pub fn tx_by_id(&self, txid: TxId) -> Result<proto::Tx> {
        match self.mempool.tx(&txid) {
            Some(tx) => Ok(make_tx_proto(
                &tx.tx,
                &OutputsSpent::new_mempool(
                    self.mempool.spent_by().outputs_spent(&txid),
                ),
                tx.time_first_seen,
                false,
                None,
                self.avalanche,
                SlpDbData::from_mempool(self.mempool.slp(), &txid).as_ref(),
            )),
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
                let tx = Tx::from(
                    ffi::load_tx(
                        block.file_num,
                        tx_entry.data_pos,
                        tx_entry.undo_pos,
                    )
                    .wrap_err(ReadFailure(txid))?,
                );
                let outputs_spent = OutputsSpent::query(
                    &spent_by_reader,
                    &tx_reader,
                    self.mempool.spent_by().outputs_spent(&txid),
                    tx_num,
                )?;
                Ok(make_tx_proto(
                    &tx,
                    &outputs_spent,
                    tx_entry.time_first_seen,
                    tx_entry.is_coinbase,
                    Some(&block),
                    self.avalanche,
                    SlpDbData::from_db(self.db, tx_num, &tx)?.as_ref(),
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
        let mut tx = TxMut::deser(&mut raw_tx.into())?;
        for input in tx.inputs.iter_mut() {
            let txid = &input.prev_out.txid;
            if let Some(input_tx) = self.mempool.tx(txid) {
                let output = input_tx
                    .tx
                    .outputs
                    .get(input.prev_out.out_idx as usize)
                    .ok_or(InputTxNotFound(*txid))?;
                input.coin = Some(Coin {
                    output: output.clone(),
                    height: -1,
                    is_coinbase: false,
                });
                continue;
            }
            let tx_reader = TxReader::new(self.db)?;
            let block_reader = BlockReader::new(self.db)?;
            let block_tx =
                tx_reader.tx_by_txid(txid)?.ok_or(InputTxNotFound(*txid))?;
            let block = block_reader
                .by_height(block_tx.block_height)?
                .ok_or(DbTxHasNoBlock(*txid))?;
            let input_tx = Tx::from(
                ffi::load_tx(
                    block.file_num,
                    block_tx.entry.data_pos,
                    block_tx.entry.undo_pos,
                )
                .wrap_err(ReadFailure(*txid))?,
            );
            let output = input_tx
                .outputs
                .get(input.prev_out.out_idx as usize)
                .ok_or(InputTxNotFound(*txid))?;
            input.coin = Some(Coin {
                output: output.clone(),
                height: block.height,
                is_coinbase: block_tx.entry.is_coinbase,
            });
        }
        let tx = Tx::with_txid(TxId::from_tx(&tx), tx);
        let slp =
            SlpDbData::from_tx(self.db, self.mempool.slp(), &tx, |txid| {
                self.mempool.tx(txid).is_some()
            })?;
        Ok(make_tx_proto(
            &tx,
            &OutputsSpent::default(),
            0,
            false,
            None,
            self.avalanche,
            slp.as_ref(),
        ))
    }

    pub fn token_info(&self, token_id_txid: &TxId) -> Result<proto::TokenInfo> {
        let token_info =
            token_info(self.db, self.mempool, self.avalanche, token_id_txid)?
                .ok_or(TokenNotFound(*token_id_txid))?;
        Ok(match token_info.data {
            Protocol::Slp((genesis_info, meta)) => proto::TokenInfo {
                token_id: meta.token_id.to_vec(),
                token_protocol: proto::TokenProtocol::Slpv1 as _,
                slpv1_token_type: meta.token_type.to_u16().into(),
                slpv1_genesis_info: Some(proto::Slpv1GenesisInfo {
                    token_ticker: genesis_info.token_ticker.to_vec(),
                    token_name: genesis_info.token_name.to_vec(),
                    token_document_url: genesis_info
                        .token_document_url
                        .to_vec(),
                    token_document_hash: genesis_info
                        .token_document_hash
                        .map(|hash| hash.to_vec())
                        .unwrap_or_default(),
                    decimals: genesis_info.decimals,
                }),
                block: token_info.block,
                time_first_seen: token_info.time_first_seen,
                ..Default::default()
            },
            Protocol::Slpv2((genesis_info, meta)) => proto::TokenInfo {
                token_id: meta.token_id.to_vec(),
                token_protocol: proto::TokenProtocol::Slpv2 as _,
                slpv2_token_type: meta.token_type.to_u8().into(),
                slpv2_genesis_info: Some(proto::Slpv2GenesisInfo {
                    token_ticker: genesis_info.token_ticker.to_vec(),
                    token_name: genesis_info.token_name.to_vec(),
                    url: genesis_info.url.to_vec(),
                    data: genesis_info.data.to_vec(),
                    auth_pubkey: genesis_info.auth_pubkey.to_vec(),
                    decimals: genesis_info.decimals.into(),
                }),
                block: token_info.block,
                time_first_seen: token_info.time_first_seen,
                ..Default::default()
            },
        })
    }
}
