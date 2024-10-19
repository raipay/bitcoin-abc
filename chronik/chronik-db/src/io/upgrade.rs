// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

use std::collections::HashSet;

use abc_rust_error::Result;
use bitcoinsuite_core::{
    script::{
        opcode::{OP_CHECKSIG, OP_RETURN},
        write_var_int, PubKey, UncompressedPubKey,
        COMPRESS_NUM_SPECIAL_SCRIPTS,
    },
    tx::{Tx, TxMut},
};
use bitcoinsuite_slp::slp::consts::{SLP_LOKAD_ID, TOKEN_TYPE_V2};
use bytes::{BufMut, BytesMut};
use chronik_util::log;
use rocksdb::WriteBatch;

use crate::{
    db::{Db, CF, CF_SCRIPT_HISTORY_NUM_TXS},
    groups::ScriptHistoryReader,
    index_tx::prepare_indexed_txs,
    io::{
        token::{
            TokenIndexError::{BlockNotFound, TokenTxNumNotFound},
            TokenReader, TokenWriter,
        },
        BlockReader, TxReader,
    },
};

/// Perform upgrades in the DB.
pub struct UpgradeWriter<'a> {
    db: &'a Db,
    cf_script_history_num_txs: &'a CF,
}

impl<'a> UpgradeWriter<'a> {
    /// Create a new UpgradeWriter pointing to the Db
    pub fn new(db: &'a Db) -> Result<Self> {
        let cf_script_history_num_txs = db.cf(CF_SCRIPT_HISTORY_NUM_TXS)?;
        Ok(UpgradeWriter {
            db,
            cf_script_history_num_txs,
        })
    }

    /// Fix the mint vault txs that have a mint vault input
    pub fn fix_mint_vault_txs(
        &self,
        load_tx: impl Fn(u32, u32, u32) -> Result<Tx>,
    ) -> Result<()> {
        let script_history_reader = ScriptHistoryReader::new(self.db)?;
        let block_reader = BlockReader::new(self.db)?;
        let tx_reader = TxReader::new(self.db)?;
        let token_reader = TokenReader::new(self.db)?;
        let token_writer = TokenWriter::new(self.db)?;
        let mut batch = WriteBatch::default();
        // OP_RETURNs are stored by having the size + 6 prefixed,
        // so we iterate through all sizes.
        for opreturn_size in 0..=223 {
            // Build the DB prefix for SLP MINT VAULT OP_RETURNs Scripts
            let size_prefix = opreturn_size + COMPRESS_NUM_SPECIAL_SCRIPTS;
            let mut prefix = BytesMut::with_capacity(50);
            write_var_int(&mut prefix, size_prefix as u64);
            prefix.put_slice(&[OP_RETURN::N, SLP_LOKAD_ID.len() as u8]);
            prefix.put_slice(&SLP_LOKAD_ID);
            prefix.put_slice(&[0x01, TOKEN_TYPE_V2]);
            let prefix = prefix.freeze();

            // Iterate SLP MINT VAULT scripts for that size
            let iterator = self.db.iterator(
                self.cf_script_history_num_txs,
                &prefix,
                rocksdb::Direction::Forward,
            );

            for entry in iterator {
                let (member_ser, _) = entry?;
                // Iterator keeps going until the end of the DB -> have to break
                if !member_ser.starts_with(&prefix) {
                    break;
                }
                let (num_pages, _) = script_history_reader
                    .member_num_pages_and_txs(&member_ser)?;

                // Iterate through all pages of the SLP OP_RETURN
                for page_num in 0..num_pages as u32 {
                    let Some(txs) = script_history_reader
                        .page_txs(&member_ser, page_num)?
                    else {
                        break;
                    };
                    // Iterate through all txs of the SLP OP_RETURN
                    for tx_num in txs {
                        if token_reader.token_tx(tx_num)?.is_some() {
                            // Skip txs that are already indexed
                            continue;
                        }

                        // Load tx from the node
                        let block_tx = tx_reader
                            .tx_by_tx_num(tx_num)?
                            .ok_or(TokenTxNumNotFound(tx_num))?;
                        let block = block_reader
                            .by_height(block_tx.block_height)?
                            .ok_or(BlockNotFound(block_tx.block_height))?;
                        let tx_entry = block_tx.entry;
                        log!(
                            "Upgrading SLP MINT VAULT with TxId {}\n",
                            tx_entry.txid
                        );
                        let tx = load_tx(
                            block.file_num,
                            tx_entry.data_pos,
                            tx_entry.undo_pos,
                        )?;

                        // Adding a dummy "coinbase" tx
                        let txs = vec![Tx::default(), tx];
                        // Prepare IndexTxs, pretend we have a block with 2 txs
                        let indexed_txs =
                            prepare_indexed_txs(self.db, tx_num - 1, &txs)?;

                        // Ensure we prepared tx correctly
                        assert_eq!(indexed_txs[1].tx_num, tx_num);

                        // Index token tx and write to batch
                        token_writer.insert(&mut batch, &indexed_txs[1..])?;
                    }
                }
            }
        }

        log!(
            "Upgrading, writing {} entries for the token DB fix\n",
            batch.len()
        );

        self.db.write_batch(batch)?;

        Ok(())
    }

    /// Fix incorrectly compressed P2PK scripts
    pub fn fix_p2pk_compression(
        &self,
        load_tx: impl Fn(u32, u32, u32) -> Result<Tx>,
        shutdown_requested: impl Fn() -> bool,
    ) -> Result<()> {
        let script_history_reader = ScriptHistoryReader::new(self.db)?;
        let block_reader = BlockReader::new(self.db)?;
        let tx_reader = TxReader::new(self.db)?;

        // Iterate over all scripts in the DB
        let iterator = self.db.full_iterator(self.cf_script_history_num_txs);

        fn compress_p2pk_script(
            uncompressed_script: &[u8],
            with_bug: bool,
        ) -> Option<[u8; 33]> {
            const COMP_PK_SIZE: u8 = PubKey::SIZE as u8;
            const UNCOMP_PK_SIZE: u8 = UncompressedPubKey::SIZE as u8;
            const OP_CHECKSIG: u8 = OP_CHECKSIG::N;

            match uncompressed_script {
                [COMP_PK_SIZE, pubkey @ .., OP_CHECKSIG]
                    if pubkey.len() == PubKey::SIZE =>
                {
                    if !with_bug && (pubkey[0] == 0x02 || pubkey[0] == 0x03) {
                        return None;
                    }
                    Some(pubkey.try_into().unwrap())
                }
                [UNCOMP_PK_SIZE, pubkey @ .., OP_CHECKSIG]
                    if pubkey.len() == UncompressedPubKey::SIZE =>
                {
                    if !with_bug && pubkey[0] == 0x04 {
                        return None;
                    }
                    let mut bytes = [0; 33];
                    bytes[0] = (pubkey[64] & 0x01) | 0x04;
                    bytes[1..].copy_from_slice(&pubkey[1..][..32]);
                    Some(bytes)
                }
                _ => None,
            }
        }

        let mut num_scanned = 0;
        let mut num_txs = 0;
        let mut num_uncompressed_pk_txs = 0;
        let mut scripts_not_to_upgrade = HashSet::new();
        let mut scripts_to_upgrade = HashSet::new();
        log!("Fixing P2PK scripts");
        for entry in iterator {
            num_scanned += 1;
            if num_scanned % 1000000 == 0 {
                log!(
                    "Scanned {num_scanned} scripts (num_txs = {num_txs}, num_uncompressed_pk_txs = {num_uncompressed_pk_txs}, not to upgrade = {}, to upgrade = {})\n",
                    scripts_not_to_upgrade.len(),
                    scripts_to_upgrade.len(),
                );
            }
            if num_scanned % 100 == 0 && shutdown_requested() {
                log!("Cancelled upgrade\n");
                return Ok(());
            }
            let (key, _) = entry?;

            if key.len() != 33 {
                continue;
            }
            if key[0] == 0x02 || key[0] == 0x03 {
                // These are definitely correct
                continue;
            }
            num_txs += 1;
            if key[0] == 0x04 || key[0] == 0x05 {
                num_uncompressed_pk_txs += 1;
            }

            let (num_pages, _) =
                script_history_reader.member_num_pages_and_txs(&key)?;

            // Iterate through all pages of the Script
            for page_num in 0..num_pages as u32 {
                let Some(txs) =
                    script_history_reader.page_txs(&key, page_num)?
                else {
                    break;
                };
                // Iterate through all txs of the P2PK
                for tx_num in txs {
                    // Load tx from the node
                    let block_tx = tx_reader
                        .tx_by_tx_num(tx_num)?
                        .ok_or(TokenTxNumNotFound(tx_num))?;
                    let block = block_reader
                        .by_height(block_tx.block_height)?
                        .ok_or(BlockNotFound(block_tx.block_height))?;
                    let tx_entry = block_tx.entry;
                    let tx = TxMut::from(load_tx(
                        block.file_num,
                        tx_entry.data_pos,
                        tx_entry.undo_pos,
                    )?);

                    let all_outputs = tx
                        .inputs
                        .into_iter()
                        .filter_map(|tx_input| {
                            tx_input.coin.map(|coin| coin.output)
                        })
                        .chain(tx.outputs);

                    for output in all_outputs {
                        let with_bug = compress_p2pk_script(
                            output.script.bytecode(),
                            true,
                        );
                        let Some(with_bug) = with_bug else { continue };
                        if key.as_ref() != with_bug.as_ref() {
                            continue;
                        }
                        let without_bug = compress_p2pk_script(
                            output.script.bytecode(),
                            false,
                        );
                        if without_bug == Some(with_bug) {
                            // No bug
                            scripts_not_to_upgrade.insert(output.script);
                        } else {
                            // Buggy compression
                            scripts_to_upgrade.insert(output.script);
                        }
                    }
                }
            }
        }

        log!("num_txs = {num_txs}\n");
        log!("num_uncompressed_pk_txs = {num_uncompressed_pk_txs}\n");
        log!(
            "scripts_not_to_upgrade.len() = {}\n",
            scripts_not_to_upgrade.len()
        );
        log!("scripts_to_upgrade.len() = {}\n", scripts_to_upgrade.len());
        let num_scripts_both = scripts_to_upgrade
            .intersection(&scripts_not_to_upgrade)
            .count();
        log!("num_scripts_both = {}\n", num_scripts_both);

        Ok(())
    }
}

impl std::fmt::Debug for UpgradeWriter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpgradeWriter")
            .field("db", &self.db)
            .field("cf_tx", &"..")
            .finish()
    }
}
