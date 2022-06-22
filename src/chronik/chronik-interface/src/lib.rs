use bitcoinsuite_core::{Coin, Sha256d};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tx {
    pub txid: Sha256d,
    pub raw: Vec<u8>,
    pub spent_coins: Vec<Coin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub raw: Vec<u8>,
    pub hash: Sha256d,
    pub prev_hash: Sha256d,
    pub n_bits: u32,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub txs: Vec<BlockTx>,
    pub file_num: u32,
    pub data_pos: u32,
    pub undo_pos: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTx {
    pub tx: Tx,
    pub data_pos: u32,
    pub undo_pos: u32,
    pub undo_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolTx {
    pub tx: Tx,
    pub time: i64,
}

pub trait BitcoindInterface: Sync + Send + 'static {
    fn get_block_range(
        &self,
        start_height: i32,
        num_blocks: u32,
    ) -> Result<Vec<Block>, cxx::Exception>;

    fn get_block_slice(
        &self,
        file_num: u32,
        data_pos: u32,
        num_bytes: u32,
    ) -> Result<Vec<u8>, cxx::Exception>;

    fn get_undo_slice(
        &self,
        file_num: u32,
        undo_pos: u32,
        num_bytes: u32,
    ) -> Result<Vec<u8>, cxx::Exception>;

    fn run_rpc_command(&self, command: &str, params: &[&str]) -> Result<String, cxx::Exception>;
}
