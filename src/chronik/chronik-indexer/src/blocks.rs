use bitcoinsuite_core::{BitcoinCode, BitcoinHeader, LotusHeader, Network, Sha256d};
use bitcoinsuite_error::{ErrorMeta, Result};
use bitcoinsuite_slp::RichTx;
use chronik_rocksdb::{Block, BlockHeight, BlockReader};
use chronik_interface::{BitcoindInterface};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use thiserror::Error;

use crate::SlpIndexer;

pub struct Blocks<'a, BI> {
    indexer: &'a SlpIndexer<BI>,
}

#[derive(Debug, Error, ErrorMeta)]
pub enum BlocksError {
    #[critical()]
    #[error("Inconsistent db, block hash doesn't exist: {0}")]
    InconsistentNoSuchBlock(Sha256d),

    #[critical()]
    #[error("Inconsistent db, txid doesn't exist: {0}")]
    InconsistentNoSuchBlockTx(Sha256d),
}

use self::BlocksError::*;

impl<'a, BI: BitcoindInterface> Blocks<'a, BI> {
    pub fn new(indexer: &'a SlpIndexer<BI>) -> Self {
        Blocks { indexer }
    }

    pub fn height(&self) -> Result<BlockHeight> {
        self.reader()?.height()
    }

    pub fn tip(&self) -> Result<Option<Block>> {
        self.reader()?.tip()
    }

    pub fn by_hash(&self, hash: &Sha256d) -> Result<Option<Block>> {
        self.reader()?.by_hash(hash)
    }

    pub fn by_height(&self, height: BlockHeight) -> Result<Option<Block>> {
        self.reader()?.by_height(height)
    }

    pub fn raw_header(&self, block: &Block) -> Result<Option<Vec<u8>>> {
        let header_size = match self.indexer.network {
            Network::BCH | Network::XEC | Network::XRG => BitcoinHeader::default().ser().len(),
            Network::XPI => LotusHeader::default().ser().len(),
        };
        let header = self.indexer.bitcoind_interface.get_block_slice(
            block.file_num,
            block.data_pos,
            header_size as u32,
        )?;
        Ok(Some(header))
    }

    pub fn block_txs_by_hash(&self, hash: &Sha256d) -> Result<Option<Vec<RichTx>>> {
        let db_blocks = self.indexer.db().blocks()?;
        let block = match db_blocks.by_hash(hash)? {
            Some(block) => block,
            None => return Ok(None),
        };
        self.block_txs_by_height(block.height)
    }

    pub fn block_txs_by_height(&self, height: BlockHeight) -> Result<Option<Vec<RichTx>>> {
        let bitcoind_blocks = self.indexer.bitcoind_interface.get_block_range(height, 1)?;
        let bitcoind_block = match bitcoind_blocks.into_iter().next() {
            Some(bitcoind_block) => bitcoind_block,
            None => return Ok(None),
        };
        let txs = self.indexer.txs();
        let db_txs = self.indexer.db().txs()?;
        let db_blocks = self.indexer.db().blocks()?;
        let block = db_blocks
            .by_hash(&bitcoind_block.header.hash)?
            .ok_or_else(|| InconsistentNoSuchBlock(bitcoind_block.header.hash.clone()))?;
        let txs = bitcoind_block
            .txs
            .into_par_iter()
            .map(|bitcoind_tx| {
                let (tx_num, block_tx) = db_txs
                    .tx_and_num_by_txid(&bitcoind_tx.tx.txid)?
                    .ok_or_else(|| InconsistentNoSuchBlockTx(bitcoind_tx.tx.txid.clone()))?;
                txs.rich_block_tx_prefetched(
                    tx_num,
                    &block_tx,
                    bitcoind_tx.tx.raw.into(),
                    Some(bitcoind_tx.tx.spent_coins).filter(|spent_coins| !spent_coins.is_empty()),
                    &block,
                )
            })
            .collect::<Result<_>>()?;
        Ok(Some(txs))
    }

    fn reader(&self) -> Result<BlockReader> {
        self.indexer.db.blocks()
    }
}
