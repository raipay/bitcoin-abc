// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`QueryGroupHistory`], to query the tx history of a group.

use std::collections::BTreeSet;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, TxId};
use chronik_db::{
    db::Db,
    group::Group,
    io::{GroupUtxoReader, TxNum, TxReader},
    mem::{Mempool, MempoolGroupUtxos},
    slp::io::SlpReader,
    slpv2::io::Slpv2Reader,
};
use chronik_proto::proto;
use thiserror::Error;

use crate::{
    avalanche::Avalanche,
    query::{
        make_outpoint_proto, make_slp_meta, make_slp_token_proto,
        make_slpv2_token_proto,
    },
};

static EMPTY_MEMBER_UTXOS: BTreeSet<OutPoint> = BTreeSet::new();

/// Query pages of the tx history of a group
#[derive(Debug)]
pub struct QueryGroupUtxos<'a, G: Group> {
    /// Database
    pub db: &'a Db,
    /// Avalanche
    pub avalanche: &'a Avalanche,
    /// Mempool
    pub mempool: &'a Mempool,
    /// The part of the mempool we search for this group's history.
    pub mempool_utxos: &'a MempoolGroupUtxos<G>,
    /// Group to query txs by
    pub group: G,
}

/// Errors indicating something went wrong with [`QueryGroupUtxos`].
#[derive(Debug, Error, PartialEq)]
pub enum QueryGroupUtxosError {
    /// Transaction not in mempool.
    #[error("500: Inconsistent mempool: Transaction {0} not in mempool")]
    MissingMempoolTx(TxId),

    /// tx_num in group index but not in "tx" CF.
    #[error("500: Inconsistent DB: Transaction num {0} not in DB")]
    MissingDbTx(TxNum),

    /// Transaction not in mempool.
    #[error(
        "500: Inconsistent mempool: Transaction for {0:?} exists in mempool, \
         but the output doesn't"
    )]
    MempoolTxOutputsOutOfBounds(OutPoint),
}

use self::QueryGroupUtxosError::*;

impl<'a, G: Group> QueryGroupUtxos<'a, G> {
    /// Return the UTXOs of the given member, from both DB and mempool.
    ///
    /// UTXOs are sorted this way:
    /// - DB UTXOs first, ordered as they appear on the blockchain
    /// - Mempool UTXOs second, ordered by txid:out_idx.
    ///
    /// Note: This call can potentially be expensive on members with many UTXOs.
    pub fn utxos(
        &self,
        member: G::Member<'_>,
    ) -> Result<Vec<proto::ScriptUtxo>> {
        let tx_reader = TxReader::new(self.db)?;
        let utxo_reader = GroupUtxoReader::<G>::new(self.db)?;
        let slp_reader = SlpReader::new(self.db)?;
        let slpv2_reader = Slpv2Reader::new(self.db)?;
        let member_ser = self.group.ser_member(&member);

        // Read UTXO entries from DB and mempool
        let db_utxos =
            utxo_reader.utxos(member_ser.as_ref())?.unwrap_or_default();
        let mempool_utxos = self
            .mempool_utxos
            .utxos(member_ser.as_ref())
            .unwrap_or(&EMPTY_MEMBER_UTXOS);

        // Allocate sufficient space for the result. Note: This over-allocates
        // as many DB UTXOs as are spent in the mempool. Since these are
        // expensive and short-lived, this doesn't really pose a DoS
        // attack vector.
        let mut utxos =
            Vec::with_capacity(db_utxos.len() + mempool_utxos.len());

        // Read + add DB UTXOs
        for db_utxo in db_utxos {
            let tx_num = db_utxo.outpoint.tx_num;
            let out_idx = db_utxo.outpoint.out_idx;

            // Read some tx DB data (without reading the full tx off disk)
            let db_tx =
                tx_reader.tx_by_tx_num(tx_num)?.ok_or(MissingDbTx(tx_num))?;
            let txid = db_tx.entry.txid;

            // Skip UTXOs that are spent in the mempool
            if let Some(spent_entries) =
                self.mempool.spent_by().outputs_spent(&txid)
            {
                if spent_entries.contains_key(&out_idx) {
                    continue;
                }
            }

            let slp_tx_data = slp_reader.tx_data_by_tx_num(tx_num)?;
            let slpv2_tx_data = slpv2_reader.tx_data_by_tx_num(tx_num)?;

            let outpoint = OutPoint { txid, out_idx };
            utxos.push(proto::ScriptUtxo {
                outpoint: Some(make_outpoint_proto(&outpoint)),
                block_height: db_tx.block_height,
                is_coinbase: db_tx.entry.is_coinbase,
                value: db_utxo.value,
                is_final: self.avalanche.is_final_height(db_tx.block_height),
                slp_meta: slp_tx_data.as_ref().map(make_slp_meta),
                slp_token: slp_tx_data.as_ref().and_then(|tx_data| {
                    Some(make_slp_token_proto(
                        tx_data.output_tokens.get(out_idx as usize)?,
                    ))
                }),
                slpv2: slpv2_tx_data.and_then(|tx_data| {
                    Some(make_slpv2_token_proto(
                        &tx_data,
                        tx_data.outputs[out_idx as usize].as_ref()?,
                    ))
                }),
                network: proto::Network::Xec as _,
            });
        }

        // Add mempool UTXOs
        for &mempool_outpoint in mempool_utxos {
            let mempool_tx = self
                .mempool
                .tx(&mempool_outpoint.txid)
                .ok_or(MissingMempoolTx(mempool_outpoint.txid))?;
            let output = mempool_tx
                .tx
                .outputs
                .get(mempool_outpoint.out_idx as usize)
                .ok_or(MempoolTxOutputsOutOfBounds(mempool_outpoint))?;
            let slp_tx_data =
                self.mempool.slp().tx_data(&mempool_outpoint.txid);
            utxos.push(proto::ScriptUtxo {
                outpoint: Some(make_outpoint_proto(&mempool_outpoint)),
                block_height: -1,
                is_coinbase: false,
                value: output.value,
                is_final: false,
                slp_meta: slp_tx_data.map(make_slp_meta),
                slp_token: slp_tx_data.as_ref().and_then(|tx_data| {
                    Some(make_slp_token_proto(
                        tx_data
                            .output_tokens
                            .get(mempool_outpoint.out_idx as usize)?,
                    ))
                }),
                slpv2: self
                    .mempool
                    .slpv2()
                    .tx_data(&mempool_outpoint.txid)
                    .and_then(|tx_data| {
                        Some(make_slpv2_token_proto(
                            tx_data,
                            tx_data.outputs[mempool_outpoint.out_idx as usize]
                                .as_ref()?,
                        ))
                    }),
                network: proto::Network::Xec as _,
            });
        }

        Ok(utxos)
    }
}
