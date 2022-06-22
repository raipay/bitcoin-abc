use std::{collections::HashSet, sync::Arc};

use bitcoinsuite_core::{ecc::Ecc, BitcoinCode, Bytes, Network, Script, Sha256d, UnhashedTx};
use bitcoinsuite_error::{ErrorMeta, Result};
use chronik_bitcoind_rpc::BitcoindRpc;
use chronik_interface::BitcoindInterface;
use chronik_rocksdb::{
    script_payloads, Block, BlockHeight, BlockTxs, IndexDb, IndexMemData, MempoolData,
    MempoolSlpData, MempoolTxEntry, TransientBlockDataReader, TxEntry,
};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::{
    broadcast::Broadcast,
    subscribers::{SubscribeBlockMessage, SubscribeScriptMessage, Subscribers},
    txs::Txs,
    Blocks, ScriptHistory, Tokens, Utxos,
};

pub struct SlpIndexer<BI> {
    pub(crate) db: IndexDb,
    pub(crate) bitcoind_interface: BI,
    pub(crate) data: IndexMemData,
    pub(crate) network: Network,
    pub(crate) ecc: Arc<dyn Ecc + Sync + Send>,
    subscribers: Subscribers,
}

#[derive(Debug, Error, ErrorMeta)]
pub enum SlpIndexerError {
    #[critical()]
    #[error(
        "Index and node diverged: index height is {index_height}, tip is {index_tip}, \
             node height is {node_height}, tip is {node_tip}"
    )]
    IndexDiverged {
        index_height: i32,
        index_tip: Sha256d,
        node_height: i32,
        node_tip: Sha256d,
    },
}

impl<BI: BitcoindInterface> SlpIndexer<BI> {
    pub fn new(
        bitcoind_interface: BI,
        db: IndexDb,
        data: IndexMemData,
        network: Network,
        ecc: Arc<dyn Ecc + Sync + Send>,
    ) -> Result<Self> {
        db.check_db_version()?;
        Ok(SlpIndexer {
            db,
            bitcoind_interface,
            data,
            network,
            ecc,
            subscribers: Subscribers::default(),
        })
    }

    pub fn handle_block_connected(&mut self, block: chronik_interface::Block) -> Result<()> {
        println!("Got BlockConnected {}", block.header.hash);
        let tip = self.db.blocks()?.tip()?;
        let next_height = tip.as_ref().map(|tip| tip.height + 1).unwrap_or(0);
        let txs = Self::_block_txs(&block)?;
        Self::broadcast_block_msg(
            &mut self.subscribers,
            block.header.hash.clone(),
            &txs,
            &block.txs,
            true,
        );
        let db_block = Block {
            hash: block.header.hash.clone(),
            prev_hash: block.header.prev_hash,
            height: next_height,
            n_bits: block.header.n_bits,
            timestamp: block.header.timestamp,
            file_num: block.file_num,
            data_pos: block.data_pos,
        };
        let transient_block_data = self.db.transient_data().read_block(next_height)?;
        let mut transient_data_reader =
            TransientBlockDataReader::new(match &transient_block_data {
                Some(block_data) => &block_data.tx_data,
                None => &[],
            });
        let num_txs = block.txs.len();
        let db_txs = block
            .txs
            .iter()
            .zip(&txs)
            .map(|(block_tx, tx)| {
                let txid = &block_tx.tx.txid;
                let time_first_seen = match self.db_mempool().tx(txid) {
                    Some(entry) => entry.time_first_seen,
                    None => match transient_data_reader.read_for_next_txid(txid) {
                        Some(transient_entry) => transient_entry.time_first_seen,
                        None => 0, // indicates unknown
                    },
                };
                TxEntry {
                    txid: txid.clone(),
                    data_pos: block_tx.data_pos,
                    tx_size: block_tx.tx.raw.len() as u32,
                    undo_pos: block_tx.undo_pos,
                    undo_size: block_tx.undo_size,
                    time_first_seen,
                    is_coinbase: tx.inputs[0].prev_out.is_coinbase(),
                }
            })
            .collect::<Vec<_>>();
        let db_block_txs = BlockTxs {
            txs: db_txs,
            block_height: next_height,
        };
        self.db.insert_block(
            &db_block,
            &db_block_txs,
            &txs,
            |tx_pos, input_idx| &block.txs[tx_pos + 1].tx.spent_coins[input_idx].tx_output,
            &mut self.data,
        )?;
        self.update_transient_data(next_height)?;
        println!(
            "Added block {} with {} txs, height {}",
            block.header.hash, num_txs, next_height,
        );
        Ok(())
    }

    pub fn handle_block_disconnected(&mut self, block: chronik_interface::Block) -> Result<()> {
        println!("Got BlockDisconnected {}", block.header.hash);
        let tip = self.db.blocks()?.tip()?;
        let txs = Self::_block_txs(&block)?;
        Self::broadcast_block_msg(
            &mut self.subscribers,
            block.header.hash.clone(),
            &txs,
            &block.txs,
            false,
        );
        let tip = tip.unwrap();
        let txids_fn = |idx: usize| &block.txs[idx].tx.txid;
        self.db.delete_block(
            &block.header.hash,
            tip.height,
            txids_fn,
            &txs,
            |tx_pos, input_idx| &block.txs[tx_pos + 1].tx.spent_coins[input_idx].tx_output,
            &mut self.data,
        )?;
        self.db.transient_data_writer().delete_block(tip.height)?;
        println!(
            "Removed block {} via BlockDisconnected message",
            block.header.hash
        );
        Ok(())
    }

    pub fn handle_tx_added_to_mempool(
        &mut self,
        mempool_tx: chronik_interface::MempoolTx,
    ) -> Result<()> {
        let nng_tx = mempool_tx.tx;
        println!("Got mempool tx {}", nng_tx.txid);
        let mut raw_tx = Bytes::from_bytes(nng_tx.raw);
        let tx = UnhashedTx::deser(&mut raw_tx)?;
        Self::broadcast_msg(
            &mut self.subscribers,
            SubscribeScriptMessage::AddedToMempool(nng_tx.txid.clone()),
            nng_tx
                .spent_coins
                .iter()
                .map(|spent_output| &spent_output.tx_output.script),
            tx.outputs.iter().map(|spent_output| &spent_output.script),
        );
        let entry = MempoolTxEntry {
            tx,
            spent_coins: nng_tx.spent_coins,
            time_first_seen: mempool_tx.time,
        };
        self.db
            .insert_mempool_tx(&mut self.data, nng_tx.txid, entry)?;
        Ok(())
    }

    pub fn handle_tx_removed_from_mempool(&mut self, txid: Sha256d) -> Result<()> {
        println!("Removed mempool tx {}", txid);
        if let Some(tx) = self.db.mempool(&self.data).tx(&txid) {
            Self::broadcast_msg(
                &mut self.subscribers,
                SubscribeScriptMessage::RemovedFromMempool(txid.clone()),
                tx.spent_coins
                    .iter()
                    .map(|spent_coin| &spent_coin.tx_output.script),
                tx.tx.outputs.iter().map(|output| &output.script),
            );
        }
        self.db.remove_mempool_tx(&mut self.data, &txid)?;
        Ok(())
    }

    pub fn bitcoind_rpc(&self) -> BitcoindRpc<BI> {
        BitcoindRpc::new(&self.bitcoind_interface)
    }

    pub fn db(&self) -> &IndexDb {
        &self.db
    }

    pub fn db_mempool(&self) -> &MempoolData {
        self.db.mempool(&self.data)
    }

    pub fn db_mempool_slp(&self) -> &MempoolSlpData {
        self.db.mempool_slp(&self.data)
    }

    pub fn txs(&self) -> Txs<BI> {
        Txs::new(self)
    }

    pub fn blocks(&self) -> Blocks<BI> {
        Blocks::new(self)
    }

    pub fn script_history(&self) -> ScriptHistory<BI> {
        ScriptHistory::new(self)
    }

    pub fn utxos(&self) -> Utxos<BI> {
        Utxos::new(self)
    }

    pub fn tokens(&self) -> Tokens<BI> {
        Tokens::new(self)
    }

    pub fn broadcast(&self) -> Broadcast<BI> {
        Broadcast::new(self)
    }

    pub fn subscribers_mut(&mut self) -> &mut Subscribers {
        &mut self.subscribers
    }

    fn _block_txs(block: &chronik_interface::Block) -> Result<Vec<UnhashedTx>> {
        block
            .txs
            .iter()
            .map(|tx| {
                let mut raw_tx = Bytes::from_slice(&tx.tx.raw);
                UnhashedTx::deser(&mut raw_tx).map_err(Into::into)
            })
            .collect()
    }

    fn broadcast_msg<'a>(
        subscribers: &mut Subscribers,
        msg: SubscribeScriptMessage,
        spent_scripts: impl IntoIterator<Item = &'a Script>,
        output_scripts: impl IntoIterator<Item = &'a Script>,
    ) {
        let mut notified_payloads = HashSet::new();
        for script in spent_scripts.into_iter().chain(output_scripts) {
            for script_payload in script_payloads(script) {
                let script_payload = script_payload.payload;
                if !notified_payloads.contains(&script_payload) {
                    subscribers.broadcast_to_script(&script_payload, msg.clone());
                    notified_payloads.insert(script_payload);
                }
            }
        }
    }

    fn broadcast_block_msg(
        subscribers: &mut Subscribers,
        block_hash: Sha256d,
        txs: &[UnhashedTx],
        block_txs: &[chronik_interface::BlockTx],
        is_confirmed: bool,
    ) {
        subscribers.broadcast_to_blocks(if is_confirmed {
            SubscribeBlockMessage::BlockConnected(block_hash)
        } else {
            SubscribeBlockMessage::BlockDisconnected(block_hash)
        });
        for (tx, block_tx) in txs.iter().zip(block_txs) {
            let spent_scripts = block_tx
                .tx
                .spent_coins
                .iter()
                .map(|spent_coin| &spent_coin.tx_output.script);
            Self::broadcast_msg(
                subscribers,
                match is_confirmed {
                    true => SubscribeScriptMessage::Confirmed(block_tx.tx.txid.clone()),
                    false => SubscribeScriptMessage::Reorg(block_tx.tx.txid.clone()),
                },
                spent_scripts,
                tx.outputs.iter().map(|output| &output.script),
            )
        }
    }

    fn update_transient_data(&mut self, tip_height: BlockHeight) -> Result<()> {
        let next_block_height = self.db.transient_data().next_block_height().unwrap();
        // Only update if transient data caught up 12 blocks deep.
        // This overlaps with run_transient_data_catchup in case there is a race condition.
        // Since this requires Write access and run_transient_data_catchup requires Read access,
        // both will never update simultaneously.
        if next_block_height + 12 <= tip_height {
            return Ok(());
        }
        for block_height in next_block_height..=tip_height {
            self.db.transient_data_writer().update_block(block_height)?;
        }
        Ok(())
    }
}

pub async fn run_transient_data_catchup<BI: BitcoindInterface>(
    slp_indexer: &RwLock<SlpIndexer<BI>>,
) -> Result<()> {
    loop {
        let slp_indexer = slp_indexer.read().await;
        let tip = match slp_indexer.db().blocks().unwrap().tip().unwrap() {
            Some(tip) => tip,
            None => break,
        };
        let next_block_height = slp_indexer
            .db()
            .transient_data()
            .next_block_height()
            .unwrap();
        // Stop when we're 10 blocks away from tip
        if next_block_height + 10 > tip.height {
            break;
        }
        slp_indexer
            .db()
            .transient_data_writer()
            .update_block(next_block_height)
            .unwrap();
        if next_block_height % 100 == 0 {
            println!("Synced transient data up to height {}", next_block_height);
        }
    }
    Ok(())
}
