use bitcoinsuite_core::{Coin, Script, Sha256d, TxOutput};
use chronik_interface::{BitcoindInterface, Block, BlockHeader, BlockTx, MempoolTx, Tx};

use crate::ffi;

pub struct BridgeInterface {
    bridge: cxx::UniquePtr<ffi::ChronikBridge>,
}

impl BridgeInterface {
    pub fn new(bridge: cxx::UniquePtr<ffi::ChronikBridge>) -> Self {
        BridgeInterface { bridge }
    }
}

impl BitcoindInterface for BridgeInterface {
    fn get_block_range(
        &self,
        start_height: i32,
        num_blocks: u32,
    ) -> Result<Vec<Block>, cxx::Exception> {
        let blocks = self.bridge.get_block_range(start_height, num_blocks)?;
        let blocks = blocks.into_iter().map(map_ffi_block).collect::<Vec<_>>();
        Ok(blocks)
    }

    fn get_block_slice(
        &self,
        file_num: u32,
        data_pos: u32,
        num_bytes: u32,
    ) -> Result<Vec<u8>, cxx::Exception> {
        self.bridge.get_block_slice(file_num, data_pos, num_bytes)
    }

    fn get_undo_slice(
        &self,
        file_num: u32,
        undo_pos: u32,
        num_bytes: u32,
    ) -> Result<Vec<u8>, cxx::Exception> {
        self.bridge.get_undo_slice(file_num, undo_pos, num_bytes)
    }

    fn run_rpc_command(&self, command: &str, params: &[&str]) -> Result<String, cxx::Exception> {
        self.bridge.run_rpc_command(command, params)
    }
}

pub fn map_ffi_block(block: ffi::Block) -> Block {
    Block {
        header: map_ffi_block_header(block.header),
        txs: block.txs.into_iter().map(map_ffi_block_tx).collect(),
        file_num: block.file_num,
        data_pos: block.data_pos,
        undo_pos: block.undo_pos,
    }
}

pub fn map_ffi_block_header(block_header: ffi::BlockHeader) -> BlockHeader {
    BlockHeader {
        raw: block_header.raw,
        hash: Sha256d::new(block_header.hash),
        prev_hash: Sha256d::new(block_header.prev_hash),
        n_bits: block_header.n_bits,
        timestamp: block_header.timestamp,
    }
}

pub fn map_ffi_block_tx(block_tx: ffi::BlockTx) -> BlockTx {
    BlockTx {
        tx: map_ffi_tx(block_tx.tx),
        data_pos: block_tx.data_pos,
        undo_pos: block_tx.undo_pos,
        undo_size: block_tx.undo_size,
    }
}

pub fn map_ffi_tx(tx: ffi::Tx) -> Tx {
    Tx {
        txid: Sha256d::new(tx.txid),
        raw: tx.raw,
        spent_coins: tx.spent_coins.into_iter().map(map_ffi_coin).collect(),
    }
}

pub fn map_ffi_coin(coin: ffi::Coin) -> Coin {
    Coin {
        tx_output: TxOutput {
            value: coin.tx_output.value,
            script: Script::new(coin.tx_output.script.into()),
        },
        height: Some(coin.height).filter(|&height| height != 1),
        is_coinbase: coin.is_coinbase,
    }
}

pub fn map_ffi_mempool_tx(mempool_tx: ffi::MempoolTx) -> MempoolTx {
    MempoolTx {
        tx: map_ffi_tx(mempool_tx.tx),
        time: mempool_tx.time,
    }
}
