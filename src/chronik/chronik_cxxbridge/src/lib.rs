mod bridge_ecc;
mod bridge_interface;

use std::{path::Path, sync::Arc};

use bitcoinsuite_core::{Network, Sha256d};
use bitcoinsuite_error::Result;
use chronik_http::ChronikServer;
use chronik_rocksdb::IndexDb;
use tokio::sync::RwLock;

use crate::{
    bridge_ecc::BridgeEcc,
    bridge_interface::{map_ffi_block, map_ffi_mempool_tx, BridgeInterface},
};

#[cxx::bridge(namespace = "chronik_bridge")]
pub mod ffi {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Tx {
        txid: [u8; 32],
        raw: Vec<u8>,
        spent_coins: Vec<Coin>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TxOutput {
        value: i64,
        script: Vec<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Coin {
        tx_output: TxOutput,
        height: i32, // -1 for no height
        is_coinbase: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BlockHeader {
        raw: Vec<u8>,
        hash: [u8; 32],
        prev_hash: [u8; 32],
        n_bits: u32,
        timestamp: i64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Block {
        header: BlockHeader,
        txs: Vec<BlockTx>,
        file_num: u32,
        data_pos: u32,
        undo_pos: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BlockTx {
        tx: Tx,
        data_pos: u32,
        undo_pos: u32,
        undo_size: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MempoolTx {
        tx: Tx,
        time: i64,
    }

    struct SetupParams {
        bridge: UniquePtr<ChronikBridge>,
        datadir: String,
        chronik_host: String,
    }

    extern "Rust" {
        type ChronikIndexer;

        fn setup_indexer(params: SetupParams) -> Result<Box<ChronikIndexer>>;
        fn handle_tx_added_to_mempool(&self, mempool_tx: MempoolTx);
        fn handle_tx_removed_from_mempool(&self, txid: [u8; 32]);
        fn handle_block_connected(&self, block: Block);
        fn handle_block_disconnected(&self, block: Block);
    }

    unsafe extern "C++" {
        include!("chronik_cpp/chronik_bridge.h");

        type ChronikBridge;

        fn get_block_range(&self, start_height: i32, num_blocks: u32) -> Result<Vec<Block>>;
        fn get_block_slice(&self, file_num: u32, data_pos: u32, num_bytes: u32) -> Result<Vec<u8>>;
        fn get_undo_slice(&self, file_num: u32, undo_pos: u32, num_bytes: u32) -> Result<Vec<u8>>;
        fn run_rpc_command(&self, command: &str, params: &[&str]) -> Result<String>;

        // libsecp256k1 bindings
        fn serialize_pubkey_uncompressed(pubkey: [u8; 33]) -> [u8; 65];
    }
}

unsafe impl Sync for ffi::ChronikBridge {}
unsafe impl Send for ffi::ChronikBridge {}

struct ChronikIndexer {
    indexer: Arc<RwLock<chronik_indexer::SlpIndexer<BridgeInterface>>>,
    // Having this here ensures HTTP server, outstanding requests etc. will get stopped when `ChronikIndexer` is dropped.
    runtime: tokio::runtime::Runtime,
}

fn setup_indexer(params: ffi::SetupParams) -> Result<Box<ChronikIndexer>> {
    let bridge = BridgeInterface::new(params.bridge);
    let datadir = Path::new(&params.datadir);
    let db_path = datadir.join("chronik.rocksdb");
    let transient_path = datadir.join("chronik_transient_data.rocksdb");
    println!("opening chronik DB at = {}", db_path.to_string_lossy());
    println!(
        "opening chronik transient DB at = {}",
        transient_path.to_string_lossy()
    );
    let db = chronik_rocksdb::Db::open(&db_path)?;
    let transient_data = chronik_rocksdb::TransientData::open(&transient_path)?;
    let script_txs_conf = chronik_rocksdb::ScriptTxsConf { page_size: 1000 };
    let index_db = IndexDb::new(db, transient_data, script_txs_conf);
    let mem_data = chronik_rocksdb::IndexMemData::new(100_000);
    let ecc = Arc::new(BridgeEcc);
    let indexer = chronik_indexer::SlpIndexer::new(bridge, index_db, mem_data, Network::XEC, ecc)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let indexer = Box::new(ChronikIndexer {
        indexer: Arc::new(RwLock::new(indexer)),
        runtime,
    });
    let server = ChronikServer {
        addr: params.chronik_host.parse().unwrap(),
        slp_indexer: Arc::clone(&indexer.indexer),
    };
    indexer
        .runtime
        .spawn(async move { server.run().await.unwrap() });
    Ok(indexer)
}

impl ChronikIndexer {
    fn handle_tx_added_to_mempool(&self, mempool_tx: ffi::MempoolTx) {
        let mut indexer = self.indexer.blocking_write();
        indexer
            .handle_tx_added_to_mempool(map_ffi_mempool_tx(mempool_tx))
            .unwrap();
    }

    fn handle_tx_removed_from_mempool(&self, txid: [u8; 32]) {
        let mut indexer = self.indexer.blocking_write();
        let txid = Sha256d::new(txid);
        indexer.handle_tx_removed_from_mempool(txid).unwrap();
    }

    fn handle_block_connected(&self, block: ffi::Block) {
        let mut indexer = self.indexer.blocking_write();
        indexer
            .handle_block_connected(map_ffi_block(block))
            .unwrap();
    }

    fn handle_block_disconnected(&self, block: ffi::Block) {
        let mut indexer = self.indexer.blocking_write();
        indexer
            .handle_block_disconnected(map_ffi_block(block))
            .unwrap();
    }
}
