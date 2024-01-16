use std::borrow::Cow;

use abc_rust_error::Result;
use bitcoinsuite_core::{
    hash::Hashed,
    tx::{OutPoint, Tx, TxId},
};
use bitcoinsuite_slp::{
    color::ColoredTx,
    structs::{GenesisInfo, Token, TokenMeta, TxType},
    token_tx::TokenTx,
    token_type::{AlpTokenType, SlpTokenType, TokenType},
    verify::{SpentToken, VerifyContext},
};
use chronik_db::{
    db::Db,
    io::{token::TokenReader, BlockHeight, BlockReader, TxNum, TxReader},
    mem::{Mempool, MempoolTokens, MempoolTokensError},
};
use chronik_proto::proto;
use chronik_util::log;
use thiserror::Error;

use crate::{avalanche::Avalanche, query::slp::SlpDbDataError::*};

/// Helper struct to bundle token data coming from the DB or mempool.
#[derive(Debug)]
pub struct SlpDbData<'a> {
    /// Token inputs of the token tx
    pub inputs: Cow<'a, [Option<SpentToken>]>,
    /// Verified token data of the tx
    pub tx: Cow<'a, TokenTx>,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum SlpDbDataError {
    /// Transaction not in mempool nor DB.
    #[error("400: Tx {0} not found in mempool nor DB")]
    TxNotFound(TxId),

    /// TxInput has no coin.
    #[error("400: TxInput {0:?} has no coin")]
    TxInputHasNoCoin(OutPoint),

    /// Transaction token inputs couldn't be queried from the DB
    #[error("400: {0}")]
    BadTxInputs(MempoolTokensError),

    /// Token num not found.
    #[error("500: Inconsistent DB: Token num {0} not found")]
    TokenTxNumDoesntExist(TxNum),

    /// Token data not found in mempool but should be there
    #[error("500: Inconsistent DB: TxData for token {0} not in mempool")]
    TokenTxDataNotInMempool(TxId),

    /// Mempool tx not found in mempool but should be there
    #[error("500: Inconsistent DB: MempoolTx for token {0} not in mempool")]
    TokenTxNotInMempool(TxId),

    /// Block not found in DB but should be there
    #[error("500: Inconsistent DB: Missing block for height {0}")]
    MissingBlockForHeight(BlockHeight),
}

impl<'a> SlpDbData<'a> {
    /// Load token data from the mempool
    pub fn from_mempool(
        mempool: &'a MempoolTokens,
        txid: &TxId,
    ) -> Option<Self> {
        let token_tx = mempool.token_tx(txid);
        let token_inputs = mempool.tx_token_inputs(txid);
        if token_tx.is_none() && token_inputs.is_none() {
            return None;
        }
        Some(SlpDbData {
            inputs: token_inputs
                .map(Cow::Borrowed)
                .unwrap_or(Cow::Borrowed(&[])),
            tx: token_tx.map(Cow::Borrowed).unwrap_or_else(|| {
                let context = VerifyContext {
                    genesis_info: None,
                    spent_tokens: token_inputs.as_ref().unwrap(),
                    spent_scripts: None,
                    override_has_mint_vault: Some(false),
                };
                Cow::Owned(context.verify(ColoredTx::default()))
            }),
        })
    }

    /// Load token data from the DB of a mined tx
    pub fn from_db(db: &Db, tx_num: TxNum, tx: &Tx) -> Result<Option<Self>> {
        let colored = ColoredTx::color_tx(tx);

        let token_reader = TokenReader::new(db)?;

        let (spent_tokens, db_tx_data) =
            match token_reader.spent_tokens_and_db_tx(tx_num)? {
                Some(db_data) => db_data,
                None if colored.is_none() => return Ok(None),
                _ => Default::default(),
            };

        let context = VerifyContext {
            genesis_info: None,
            spent_tokens: &spent_tokens,
            spent_scripts: None,
            override_has_mint_vault: Some(db_tx_data.has_mint_vault()),
        };
        let verified = context.verify(colored.unwrap_or_default());
        Ok(Some(SlpDbData {
            inputs: Cow::Owned(spent_tokens),
            tx: Cow::Owned(verified),
        }))
    }

    /// Load token data of a tx not in the mempool or DB.
    /// The inputs of `tx` are expected to have `Coin`s set to validate SLP V2
    /// Mint Vault MINT txs.
    pub fn from_unbroadcast_tx(
        db: &Db,
        mempool: &'a Mempool,
        tx: &Tx,
    ) -> Result<Option<Self>> {
        let colored = ColoredTx::color_tx(tx);

        let tx_reader = TxReader::new(db)?;
        let token_reader = TokenReader::new(db)?;
        let spent_tokens = mempool
            .tokens()
            .fetch_tx_spent_tokens(tx, db, |txid| mempool.tx(txid).is_some())?
            .map_err(BadTxInputs)?;

        let colored = colored.unwrap_or_default();
        let mut spent_scripts = None;
        let mut genesis_info = None;
        if let Some(first_section) = colored.sections.get(0) {
            if first_section.is_mint_vault_mint() {
                let spent_scripts =
                    spent_scripts.insert(Vec::with_capacity(tx.inputs.len()));
                for tx_input in &tx.inputs {
                    let coin = tx_input
                        .coin
                        .as_ref()
                        .ok_or(TxInputHasNoCoin(tx_input.prev_out))?;
                    spent_scripts.push(coin.output.script.clone());
                }
                let genesis_tx_num = tx_reader
                    .tx_num_by_txid(first_section.meta.token_id.txid())?;
                if let Some(genesis_tx_num) = genesis_tx_num {
                    if let Some(db_genesis_info) =
                        token_reader.genesis_info(genesis_tx_num)?
                    {
                        genesis_info = Some(db_genesis_info);
                    }
                }
            }
        }

        let context = VerifyContext {
            genesis_info: genesis_info.as_ref(),
            spent_tokens: &spent_tokens,
            spent_scripts: spent_scripts.as_deref(),
            override_has_mint_vault: None,
        };
        let verified = context.verify(colored);

        log!("verified {} = {:?}", tx.txid(), verified);

        Ok(Some(SlpDbData {
            inputs: Cow::Owned(spent_tokens),
            tx: Cow::Owned(verified),
        }))
    }

    /// Built token data for a tx input
    pub fn input_token_proto(&self, input_idx: usize) -> Option<proto::Token> {
        let spent_token = self.inputs.get(input_idx)?.as_ref()?;
        let token = &spent_token.token;
        let entry_idx = self
            .tx
            .entries
            .iter()
            .position(|section| section.meta == token.meta)
            .map(|section| section as i32)
            .unwrap_or(-1);
        Some(proto::Token {
            token_id: token.meta.token_id.to_string(),
            token_type: Some(make_token_type_proto(token.meta.token_type)),
            entry_idx,
            amount: token.variant.amount(),
            is_mint_baton: token.variant.is_mint_baton(),
        })
    }

    /// Built token data for a tx output
    pub fn output_token_proto(
        &self,
        output_idx: usize,
    ) -> Option<proto::Token> {
        let token_output = self.tx.outputs.get(output_idx)?.as_ref()?;
        let token = self.tx.token(token_output);

        Some(proto::Token {
            token_id: token.meta.token_id.to_string(),
            token_type: Some(make_token_type_proto(token.meta.token_type)),
            entry_idx: token_output.token_idx as _,
            amount: token.variant.amount() as _,
            is_mint_baton: token.variant.is_mint_baton(),
        })
    }

    /// Built token entry protobuf data for a token tx
    pub fn entries_proto(&self) -> Vec<proto::TokenEntry> {
        self.tx
            .entries
            .iter()
            .map(|entry| proto::TokenEntry {
                token_id: entry.meta.token_id.to_string(),
                token_type: Some(make_token_type_proto(entry.meta.token_type)),
                tx_type: match entry.tx_type {
                    Some(TxType::GENESIS) => proto::TokenTxType::Genesis,
                    Some(TxType::MINT) => proto::TokenTxType::Mint,
                    Some(TxType::SEND) => proto::TokenTxType::Send,
                    Some(TxType::UNKNOWN) => proto::TokenTxType::Unknown,
                    Some(TxType::BURN) => proto::TokenTxType::Burn,
                    None => proto::TokenTxType::None,
                } as _,
                genesis_info: entry
                    .genesis_info
                    .as_ref()
                    .map(make_genesis_info_proto),
                group_token_id: entry
                    .group_token_meta
                    .as_ref()
                    .map_or(String::new(), |meta| meta.token_id.to_string()),
                burn_error: entry
                    .burn_error
                    .as_ref()
                    .map_or(String::new(), |burn_error| burn_error.to_string()),
                failed_colorings: entry
                    .failed_colorings
                    .iter()
                    .map(|failed_coloring| proto::TokenFailedColoring {
                        pushdata_idx: failed_coloring.pushdata_idx as _,
                        error: failed_coloring.error.to_string(),
                    })
                    .collect(),
                actual_burn_amount: entry.actual_burn_amount.to_string(),
                intentional_burn: entry
                    .intentional_burn_amount
                    .unwrap_or_default(),
                burns_mint_batons: entry.burns_mint_batons,
            })
            .collect()
    }
}

/// Read just the output data of a token tx
pub fn db_output(
    db: &Db,
    tx_num: TxNum,
    out_idx: u32,
) -> Result<Option<SpentToken>> {
    let token_reader = TokenReader::new(db)?;
    let Some(db_token_tx) = token_reader.token_tx(tx_num)? else {
        return Ok(None);
    };
    db_token_tx.spent_token(&db_token_tx.outputs[out_idx as usize], |tx_num| {
        Ok(token_reader
            .token_meta(tx_num)?
            .ok_or(TokenTxNumDoesntExist(tx_num))?)
    })
}

/// Build a protobuf token type
pub fn make_token_type_proto(token_type: TokenType) -> proto::TokenType {
    proto::TokenType {
        token_type: Some(match token_type {
            TokenType::Slp(slp) => {
                use proto::SlpTokenType::*;
                proto::token_type::TokenType::Slp(match slp {
                    SlpTokenType::Fungible => Fungible as _,
                    SlpTokenType::MintVault => MintVault as _,
                    SlpTokenType::Nft1Group => Nft1Group as _,
                    SlpTokenType::Nft1Child => Nft1Child as _,
                    SlpTokenType::Unknown(unknown) => unknown as _,
                })
            }
            TokenType::Alp(alp) => {
                use proto::AlpTokenType::*;
                proto::token_type::TokenType::Alp(match alp {
                    AlpTokenType::Standard => Standard as _,
                    AlpTokenType::Unknown(unknown) => unknown as _,
                })
            }
        }),
    }
}

/// Build protobuf genesis info
pub fn make_genesis_info_proto(
    genesis_info: &GenesisInfo,
) -> proto::GenesisInfo {
    proto::GenesisInfo {
        token_ticker: genesis_info.token_ticker.to_vec(),
        token_name: genesis_info.token_name.to_vec(),
        url: genesis_info.url.to_vec(),
        hash: genesis_info
            .hash
            .as_ref()
            .map_or(vec![], |hash| hash.to_vec()),
        mint_vault_scripthash: genesis_info
            .mint_vault_scripthash
            .map_or(vec![], |hash| hash.to_le_vec()),
        data: genesis_info
            .data
            .as_ref()
            .map_or(vec![], |data| data.to_vec()),
        auth_pubkey: genesis_info
            .auth_pubkey
            .as_ref()
            .map_or(vec![], |pubkey| pubkey.to_vec()),
        decimals: genesis_info.decimals as _,
    }
}

/// Build a token for UTXO data
pub fn make_utxo_token_proto(token: &Token) -> proto::Token {
    proto::Token {
        token_id: token.meta.token_id.to_string(),
        token_type: Some(make_token_type_proto(token.meta.token_type)),
        entry_idx: -1,
        amount: token.variant.amount(),
        is_mint_baton: token.variant.is_mint_baton(),
    }
}

/// Info about a token in the DB/Mempool
#[derive(Debug)]
pub struct TokenInfo {
    /// Meta of the token
    pub meta: TokenMeta,
    /// Info in the GENESIS tx of the token
    pub genesis_info: GenesisInfo,
    /// Block the token GENESIS has been mined in, if it's been mined already
    pub block: Option<proto::BlockMetadata>,
    /// First time the GENESIS tx of the token has been seen on the network
    pub time_first_seen: i64,
}

/// Read token info from the DB or mempool
pub fn token_info(
    db: &Db,
    mempool: &Mempool,
    avalanche: &Avalanche,
    token_id_txid: &TxId,
) -> Result<Option<TokenInfo>> {
    if let Some(genesis_info) = mempool.tokens().genesis_info(token_id_txid) {
        let token_tx = mempool
            .tokens()
            .token_tx(token_id_txid)
            .ok_or(TokenTxDataNotInMempool(*token_id_txid))?;
        let mempool_tx = mempool
            .tx(token_id_txid)
            .ok_or(TokenTxNotInMempool(*token_id_txid))?;
        return Ok(Some(TokenInfo {
            meta: token_tx.entries[0].meta,
            genesis_info: genesis_info.clone(),
            block: None,
            time_first_seen: mempool_tx.time_first_seen,
        }));
    }
    let tx_reader = TxReader::new(db)?;
    let token_reader = TokenReader::new(db)?;
    let block_reader = BlockReader::new(db)?;
    let (tx_num, block_tx) =
        match tx_reader.tx_and_num_by_txid(token_id_txid)? {
            Some(tuple) => tuple,
            None => return Ok(None),
        };
    let block = block_reader
        .by_height(block_tx.block_height)?
        .ok_or(MissingBlockForHeight(block_tx.block_height))?;
    let genesis_info = match token_reader.genesis_info(tx_num)? {
        Some(db_genesis) => db_genesis,
        None => return Ok(None),
    };
    let meta = token_reader
        .token_meta(tx_num)?
        .ok_or(TokenTxNumDoesntExist(tx_num))?;
    Ok(Some(TokenInfo {
        meta,
        genesis_info,
        block: Some(proto::BlockMetadata {
            height: block_tx.block_height,
            hash: block.hash.to_vec(),
            timestamp: block.timestamp,
            is_final: avalanche.is_final_height(block_tx.block_height),
        }),
        time_first_seen: block_tx.entry.time_first_seen,
    }))
}
