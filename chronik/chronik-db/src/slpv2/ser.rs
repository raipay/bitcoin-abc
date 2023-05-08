use abc_rust_error::Result;
use bitcoinsuite_slp::slpv2::{self, GenesisData, TokenId, TokenMeta};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{
    ser::{db_deserialize, db_serialize},
    slpv2::{
        io::TokenNum,
        structs::{DbTxData, DbTxSection},
    },
};

#[derive(Deserialize, Serialize, Clone)]
struct SerTokenOutput {
    section_idx: u32,
    amount: i64,
}

#[derive(Deserialize, Serialize, Clone)]
enum SerSectionType {
    GENESIS = 0,
    MINT = 1,
    SEND = 2,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerTxData {
    sections: Vec<SerTxSection>,
    burn_token_nums: Vec<TokenNum>,
    input_tokens: Vec<SerTokenOutput>,
    output_tokens: Vec<SerTokenOutput>,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerTxSection {
    token_num: TokenNum,
    section_type: SerSectionType,
    expected_input_sum: i64,
    intentional_burn_amount: i64,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerGenesisData {
    token_ticker: Vec<u8>,
    token_name: Vec<u8>,
    url: Vec<u8>,
    data: Vec<u8>,
    auth_pubkey: Vec<u8>,
    decimals: u8,
}

#[derive(Deserialize, Serialize, Clone)]
enum SerTokenType {
    Standard,
}

#[derive(Deserialize, Serialize, Clone)]
struct SerTokenMeta {
    token_id: [u8; 32],
    token_type: SerTokenType,
}

pub fn ser_tx_data(tx_data: &DbTxData) -> Result<Vec<u8>> {
    let ser_tx_data = SerTxData {
        sections: tx_data
            .sections
            .iter()
            .map(|section| SerTxSection {
                token_num: section.token_num,
                section_type: match section.section_type {
                    slpv2::SectionType::GENESIS => SerSectionType::GENESIS,
                    slpv2::SectionType::MINT => SerSectionType::MINT,
                    slpv2::SectionType::SEND => SerSectionType::SEND,
                },
                expected_input_sum: section.expected_input_sum,
                intentional_burn_amount: section.intentional_burn_amount,
            })
            .collect(),
        burn_token_nums: tx_data.burn_token_nums.clone(),
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
    };
    db_serialize(&ser_tx_data)
}

pub fn deser_tx_data(data: &[u8]) -> Result<DbTxData> {
    let tx_data = db_deserialize::<SerTxData>(data)?;
    Ok(DbTxData {
        sections: tx_data
            .sections
            .iter()
            .map(|section| DbTxSection {
                token_num: section.token_num,
                section_type: match section.section_type {
                    SerSectionType::GENESIS => slpv2::SectionType::GENESIS,
                    SerSectionType::MINT => slpv2::SectionType::MINT,
                    SerSectionType::SEND => slpv2::SectionType::SEND,
                },
                expected_input_sum: section.expected_input_sum,
                intentional_burn_amount: section.intentional_burn_amount,
            })
            .collect(),
        burn_token_nums: tx_data.burn_token_nums,
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
    })
}

pub fn ser_genesis_data(genesis_data: &GenesisData) -> Result<Vec<u8>> {
    let ser_genesis_data = SerGenesisData {
        token_ticker: genesis_data.token_ticker.to_vec(),
        token_name: genesis_data.token_name.to_vec(),
        url: genesis_data.url.to_vec(),
        data: genesis_data.data.to_vec(),
        auth_pubkey: genesis_data.auth_pubkey.to_vec(),
        decimals: genesis_data.decimals,
    };
    db_serialize(&ser_genesis_data)
}

pub fn deser_genesis_data(data: &[u8]) -> Result<GenesisData> {
    let genesis_data = db_deserialize::<SerGenesisData>(data)?;
    Ok(GenesisData {
        token_ticker: Bytes::from(genesis_data.token_ticker),
        token_name: Bytes::from(genesis_data.token_name),
        url: Bytes::from(genesis_data.url),
        data: Bytes::from(genesis_data.data),
        auth_pubkey: Bytes::from(genesis_data.auth_pubkey),
        decimals: genesis_data.decimals,
    })
}

pub fn ser_token_meta(token_meta: &TokenMeta) -> Result<Vec<u8>> {
    let ser_meta = SerTokenMeta {
        token_id: token_meta.token_id.to_bytes(),
        token_type: match token_meta.token_type {
            slpv2::TokenType::Standard => SerTokenType::Standard,
        },
    };
    db_serialize(&ser_meta)
}

pub fn deser_token_meta(data: &[u8]) -> Result<TokenMeta> {
    let ser_meta = db_deserialize::<SerTokenMeta>(data)?;
    Ok(TokenMeta {
        token_id: TokenId::from(ser_meta.token_id),
        token_type: match ser_meta.token_type {
            SerTokenType::Standard => slpv2::TokenType::Standard,
        },
    })
}

fn to_ser_token_output(
    token_output: &Option<slpv2::TokenOutputData>,
) -> SerTokenOutput {
    match token_output {
        Some(token_output) => SerTokenOutput {
            section_idx: token_output.section_idx as u32,
            amount: if token_output.is_mint_baton {
                -1
            } else {
                token_output.amount
            },
        },
        None => SerTokenOutput {
            section_idx: 0,
            amount: 0,
        },
    }
}

fn from_ser_token_output(
    token_output: &SerTokenOutput,
) -> Option<slpv2::TokenOutputData> {
    if token_output.amount == 0 {
        return None;
    }
    let (amount, is_mint_baton) = if token_output.amount == -1 {
        (0, true)
    } else {
        (token_output.amount, false)
    };
    Some(slpv2::TokenOutputData {
        section_idx: token_output.section_idx as usize,
        amount,
        is_mint_baton,
    })
}
