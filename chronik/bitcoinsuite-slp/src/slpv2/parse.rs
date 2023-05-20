use bitcoinsuite_core::{
    bytes::{read_array, read_bytes},
    error::DataError,
    script::Script,
    tx::{Tx, TxId},
};
use bytes::Bytes;
use thiserror::Error;

use crate::{
    empp,
    slpv2::{
        Genesis, GenesisData, MintData, ParseData, Parsed, Section,
        SectionVariant, Send, TokenId, TokenMeta, TokenType,
    },
};

pub type LokadId = [u8; 4];

pub const SLPV2_LOKAD_ID: LokadId = *b"SLP2";
pub const DEFAULT_TOKEN_TYPE: u8 = 200;

pub const GENESIS: &[u8] = b"GENESIS";
pub const MINT: &[u8] = b"MINT";
pub const SEND: &[u8] = b"SEND";
pub const BURN: &[u8] = b"BURN";

/// Errors forwhen parsing a SLPv2 tx.
#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Invalid EMPP output: {0}")]
    EmppError(#[from] empp::ParseError),

    #[error("Failed parsing pushdata: {0}")]
    DataError(#[from] DataError),

    #[error("Leftover bytes: {0:?}")]
    LeftoverBytes(Bytes),

    #[error("No outputs")]
    NoOutputs,

    #[error("Invalid LOKAD_ID: {0:?}")]
    InvalidLokadId(LokadId),

    #[error("Size out of range: {0}, must be 0-127")]
    SizeOutOfRange(u8),

    #[error("Unknown token type {0}")]
    UnknownTokenType(u8),

    #[error("Unknown tx type: {0:?}")]
    UnknownTxType(Bytes),

    #[error("Decimals out of range: {0}, must be 0-9")]
    DecimalsOutOfRange(u8),

    #[error("Invalid burn, expected {:?}, but got {0:?}", BURN)]
    InvalidBurn(Bytes),

    #[error("Burn amount must be non-zero")]
    ZeroBurnAmount,
}

use self::ParseError::*;

impl ParseError {
    pub fn should_ignore(&self) -> bool {
        match self {
            ParseError::EmppError(_) => true,
            ParseError::InvalidLokadId(_) => true,
            ParseError::NoOutputs => true,
            _ => false,
        }
    }
}

pub fn parse_tx(tx: &Tx) -> ParseData {
    match tx.outputs.get(0) {
        Some(output) => parse(tx.txid_ref(), &output.script),
        None => ParseData {
            parsed: Parsed::default(),
            first_err: Some(ParseError::NoOutputs),
        },
    }
}

pub fn parse(txid: &TxId, script: &Script) -> ParseData {
    let mut empp_data = match empp::parse(script) {
        Ok(empp_data) => empp_data,
        Err(err) => {
            return ParseData {
                parsed: Parsed::default(),
                first_err: Some(err.into()),
            }
        }
    };
    let mut sections = Vec::new();
    for mut pushdata in empp_data {
        match parse_section(txid, &mut pushdata) {
            Ok(section) => sections.push(section),
            Err(err) => {
                return ParseData {
                    parsed: Parsed { sections },
                    first_err: Some(err),
                };
            }
        }
    }
    ParseData {
        parsed: Parsed { sections },
        first_err: None,
    }
}

fn parse_section(
    txid: &TxId,
    pushdata: &mut Bytes,
) -> Result<Section, ParseError> {
    let lokad_id: LokadId = read_array(pushdata)?;
    if lokad_id != SLPV2_LOKAD_ID {
        return Err(InvalidLokadId(lokad_id));
    }
    let token_type = parse_token_type(pushdata)?;
    let tx_type = read_var_bytes(pushdata)?;
    let section = match tx_type.as_ref() {
        GENESIS => parse_genesis(txid, token_type, pushdata)?,
        MINT => parse_mint(token_type, pushdata)?,
        SEND => parse_send(token_type, pushdata)?,
        _ => return Err(UnknownTxType(tx_type)),
    };
    if !pushdata.is_empty() {
        return Err(LeftoverBytes(pushdata.split_off(0)));
    }
    Ok(section)
}

fn parse_genesis(
    txid: &TxId,
    token_type: TokenType,
    pushdata: &mut Bytes,
) -> Result<Section, ParseError> {
    let token_ticker = read_var_bytes(pushdata)?;
    let token_name = read_var_bytes(pushdata)?;
    let url = read_var_bytes(pushdata)?;
    let data = read_var_bytes(pushdata)?;
    let auth_pubkey = read_var_bytes(pushdata)?;
    let decimals = read_byte(pushdata)?;
    if decimals > 9 {
        return Err(DecimalsOutOfRange(decimals));
    }
    Ok(Section {
        meta: TokenMeta {
            token_id: TokenId::from(*txid),
            token_type,
        },
        variant: SectionVariant::Genesis(Genesis {
            data: GenesisData {
                token_ticker,
                token_name,
                url,
                data,
                auth_pubkey,
                decimals,
            },
            mint_data: parse_mint_data(pushdata)?,
        }),
    })
}

fn parse_mint(
    token_type: TokenType,
    pushdata: &mut Bytes,
) -> Result<Section, ParseError> {
    let token_id: [u8; 32] = read_array(pushdata)?;
    let mint_data = parse_mint_data(pushdata)?;
    Ok(Section {
        meta: TokenMeta {
            token_id: TokenId::from(token_id),
            token_type,
        },
        variant: SectionVariant::Mint(mint_data),
    })
}

fn parse_send(
    token_type: TokenType,
    pushdata: &mut Bytes,
) -> Result<Section, ParseError> {
    let token_id: [u8; 32] = read_array(pushdata)?;
    let output_amounts = read_amounts(pushdata)?;
    Ok(Section {
        meta: TokenMeta {
            token_id: TokenId::from(token_id),
            token_type,
        },
        variant: SectionVariant::Send(Send(output_amounts)),
    })
}

fn parse_token_type(pushdata: &mut Bytes) -> Result<TokenType, ParseError> {
    let token_type = read_array::<1>(pushdata)?[0];
    if token_type != DEFAULT_TOKEN_TYPE {
        return Err(UnknownTokenType(token_type));
    }
    Ok(TokenType::Standard)
}

fn parse_mint_data(pushdata: &mut Bytes) -> Result<MintData, ParseError> {
    let amounts = read_amounts(pushdata)?;
    let num_batons = read_size(pushdata)?;
    Ok(MintData {
        amounts,
        num_batons,
    })
}

fn read_byte(pushdata: &mut Bytes) -> Result<u8, ParseError> {
    Ok(read_array::<1>(pushdata)?[0])
}

fn read_size(pushdata: &mut Bytes) -> Result<usize, ParseError> {
    let size = read_byte(pushdata)?;
    if size > 127 {
        return Err(SizeOutOfRange(size));
    }
    Ok(size.into())
}

fn read_amount(pushdata: &mut Bytes) -> Result<i64, ParseError> {
    let amount6: [u8; 6] = read_array(pushdata)?;
    let mut amount8 = [0u8; 8];
    amount8[..6].copy_from_slice(&amount6);
    let amount = u64::from_le_bytes(amount8);
    Ok(amount.try_into().unwrap())
}

fn read_amounts(pushdata: &mut Bytes) -> Result<Vec<i64>, ParseError> {
    let size = read_size(pushdata)?;
    let mut amounts = Vec::with_capacity(size);
    for _ in 0..size {
        amounts.push(read_amount(pushdata)?);
    }
    Ok(amounts)
}

fn read_var_bytes(pushdata: &mut Bytes) -> Result<Bytes, ParseError> {
    let size = read_size(pushdata)?;
    Ok(read_bytes(pushdata, size)?)
}
