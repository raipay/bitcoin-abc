use abc_rust_error::Result;
use bitcoinsuite_slp::slp;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{
    ser::{db_deserialize, db_serialize},
    slp::{io::TokenNum, structs::{DbTxData, DbBurn}},
};

#[derive(Deserialize, Serialize, Clone)]
struct SerToken {
    amount: u64,
    is_mint_baton: bool,
}

#[derive(Deserialize, Serialize, Clone)]
enum SerTxType {
    GENESIS = 0,
    MINT = 1,
    SEND = 2,
    BURN = 3,
    UNKNOWN = 4,
}

#[derive(Deserialize, Serialize, Clone)]
enum SerTokenType {
    Fungible = 1,
    Nft1Group = 2,
    Nft1Child = 3,
    Unknown = 0x7f,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerTxData {
    token_num: TokenNum,
    token_type: SerTokenType,
    tx_type: SerTxType,
    burns: Vec<Option<SerBurn>>,
    input_tokens: Vec<SerToken>,
    output_tokens: Vec<SerToken>,
    group_token_num: Option<TokenNum>,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerGenesisInfo {
    token_ticker: Vec<u8>,
    token_name: Vec<u8>,
    token_document_url: Vec<u8>,
    token_document_hash: Option<[u8; 32]>,
    decimals: u8,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerTokenMeta {
    token_id: [u8; 32],
    token_type: SerTokenType,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerBurn {
    token_num: TokenNum,
    token: SerToken,
}

pub fn ser_tx_data(tx_data: &DbTxData) -> Result<Vec<u8>> {
    let ser_tx_data = SerTxData {
        token_num: tx_data.token_num,
        token_type: to_ser_token_type(tx_data.token_type),
        tx_type: match tx_data.tx_type {
            slp::TxTypeVariant::Genesis => SerTxType::GENESIS,
            slp::TxTypeVariant::Mint => SerTxType::MINT,
            slp::TxTypeVariant::Send => SerTxType::SEND,
            slp::TxTypeVariant::Unknown => SerTxType::UNKNOWN,
            slp::TxTypeVariant::Burn => SerTxType::BURN,
        },
        burns: tx_data
            .burns
            .iter()
            .map(|burn| burn.as_ref().map(|burn| SerBurn {
                token_num: burn.token_num,
                token: to_ser_token_output(&burn.token),
            }))
            .collect(),
        input_tokens: tx_data
            .input_tokens
            .iter()
            .map(to_ser_token_output)
            .collect(),
        output_tokens: tx_data
            .output_tokens
            .iter()
            .map(to_ser_token_output)
            .collect(),
        group_token_num: tx_data.group_token_num,
    };
    db_serialize(&ser_tx_data)
}

pub fn deser_tx_data(data: &[u8]) -> Result<DbTxData> {
    let tx_data = db_deserialize::<SerTxData>(data)?;
    Ok(DbTxData {
        token_num: tx_data.token_num,
        token_type: from_ser_token_type(tx_data.token_type),
        tx_type: match tx_data.tx_type {
            SerTxType::GENESIS => slp::TxTypeVariant::Genesis,
            SerTxType::MINT => slp::TxTypeVariant::Mint,
            SerTxType::SEND => slp::TxTypeVariant::Send,
            SerTxType::UNKNOWN => slp::TxTypeVariant::Unknown,
            SerTxType::BURN => slp::TxTypeVariant::Burn,
        },
        burns: tx_data
            .burns
            .iter()
            .map(|burn| burn.as_ref().map(|burn| DbBurn {
                token_num: burn.token_num,
                token: from_ser_token_output(&burn.token),
            }))
            .collect(),
        input_tokens: tx_data
            .input_tokens
            .iter()
            .map(from_ser_token_output)
            .collect(),
        output_tokens: tx_data
            .output_tokens
            .iter()
            .map(from_ser_token_output)
            .collect(),
        group_token_num: tx_data.group_token_num,
    })
}

pub fn ser_genesis_data(genesis_data: &slp::GenesisInfo) -> Result<Vec<u8>> {
    let ser_genesis_data = SerGenesisInfo {
        token_ticker: genesis_data.token_ticker.to_vec(),
        token_name: genesis_data.token_name.to_vec(),
        token_document_url: genesis_data.token_document_url.to_vec(),
        token_document_hash: genesis_data.token_document_hash,
        decimals: genesis_data.decimals as u8,
    };
    db_serialize(&ser_genesis_data)
}

pub fn deser_genesis_data(data: &[u8]) -> Result<slp::GenesisInfo> {
    let genesis_data = db_deserialize::<SerGenesisInfo>(data)?;
    Ok(slp::GenesisInfo {
        token_ticker: Bytes::from(genesis_data.token_ticker),
        token_name: Bytes::from(genesis_data.token_name),
        token_document_url: Bytes::from(genesis_data.token_document_url),
        token_document_hash: genesis_data.token_document_hash,
        decimals: genesis_data.decimals as u32,
    })
}

pub fn ser_token_meta(token_meta: &slp::TokenMeta) -> Result<Vec<u8>> {
    let ser_meta = SerTokenMeta {
        token_id: token_meta.token_id.to_bytes(),
        token_type: to_ser_token_type(token_meta.token_type),
    };
    db_serialize(&ser_meta)
}

pub fn deser_token_meta(data: &[u8]) -> Result<slp::TokenMeta> {
    let ser_meta = db_deserialize::<SerTokenMeta>(data)?;
    Ok(slp::TokenMeta {
        token_id: slp::TokenId::from_be_bytes(ser_meta.token_id),
        token_type: from_ser_token_type(ser_meta.token_type),
    })
}

fn to_ser_token_type(token_type: slp::TokenType) -> SerTokenType {
    match token_type {
        slp::TokenType::Fungible => SerTokenType::Fungible,
        slp::TokenType::Nft1Group => SerTokenType::Nft1Group,
        slp::TokenType::Nft1Child => SerTokenType::Nft1Child,
        slp::TokenType::Unknown => SerTokenType::Unknown,
    }
}

fn from_ser_token_type(token_type: SerTokenType) -> slp::TokenType {
    match token_type {
        SerTokenType::Fungible => slp::TokenType::Fungible,
        SerTokenType::Nft1Group => slp::TokenType::Nft1Group,
        SerTokenType::Nft1Child => slp::TokenType::Nft1Child,
        SerTokenType::Unknown => slp::TokenType::Unknown,
    }
}

fn to_ser_token_output(token_output: &slp::Token) -> SerToken {
    SerToken {
        amount: token_output.amount,
        is_mint_baton: token_output.is_mint_baton,
    }
}

fn from_ser_token_output(token_output: &SerToken) -> slp::Token {
    slp::Token {
        amount: token_output.amount,
        is_mint_baton: token_output.is_mint_baton,
    }
}
