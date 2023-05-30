use bitcoinsuite_core::{
    bytes::{read_array, read_bytes},
    error::DataError,
    tx::TxId,
};
use bytes::Bytes;
use thiserror::Error;

use crate::{slpv2::{
    consts::{SLPV2_LOKAD_ID, STANDARD_TOKEN_TYPE},
    Amount, Genesis, GenesisInfo, MintData, Section, SectionVariant, Send,
    TokenId, TokenMeta, TokenType,
}, common::{LokadId, GENESIS, MINT, SEND, BURN}};

/// Errors when parsing a SLPv2 tx.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Failed parsing pushdata: {0}")]
    DataError(#[from] DataError),

    #[error("Leftover bytes: {0:?}")]
    LeftoverBytes(Bytes),

    #[error("Invalid LOKAD_ID: {0:?}")]
    InvalidLokadId(LokadId),

    #[error("Unknown tx type: {0:?}")]
    UnknownTxType(Bytes),

    #[error("Size out of range: {0}, must be 0-127")]
    SizeOutOfRange(u8),

    #[error("Decimals out of range: {0}, must be 0-9")]
    DecimalsOutOfRange(u8),
}

use self::ParseError::*;

pub fn parse_section(
    txid: &TxId,
    mut pushdata: Bytes,
) -> Result<Section, ParseError> {
    let lokad_id: LokadId = read_array(&mut pushdata)?;
    if lokad_id != SLPV2_LOKAD_ID {
        return Err(InvalidLokadId(lokad_id));
    }
    let token_type = parse_token_type(&mut pushdata)?;
    if let TokenType::Unknown(_) = token_type {
        return Ok(Section {
            meta: TokenMeta {
                token_id: TokenId::from([0; 32]),
                token_type,
            },
            variant: SectionVariant::Unknown,
        });
    }
    let tx_type = read_var_bytes(&mut pushdata)?;
    let parsed = match tx_type.as_ref() {
        GENESIS => parse_genesis(txid, token_type, &mut pushdata)?,
        MINT => parse_mint(token_type, &mut pushdata)?,
        SEND => parse_send(token_type, &mut pushdata)?,
        BURN => parse_burn(token_type, &mut pushdata)?,
        _ => return Err(UnknownTxType(tx_type)),
    };
    if !pushdata.is_empty() {
        return Err(LeftoverBytes(pushdata.split_off(0)));
    }
    Ok(parsed)
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
    let mint_data = parse_mint_data(pushdata)?;
    if decimals > 9 {
        return Err(DecimalsOutOfRange(decimals));
    }
    Ok(Section {
        meta: TokenMeta {
            token_id: TokenId::from(*txid),
            token_type,
        },
        variant: SectionVariant::Genesis(Genesis {
            data: GenesisInfo {
                token_ticker,
                token_name,
                url,
                data,
                auth_pubkey,
                decimals,
            },
            mint_data,
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

fn parse_burn(
    token_type: TokenType,
    pushdata: &mut Bytes,
) -> Result<Section, ParseError> {
    let token_id: [u8; 32] = read_array(pushdata)?;
    let amount = read_amount(pushdata)?;
    Ok(Section {
        meta: TokenMeta {
            token_id: TokenId::from(token_id),
            token_type,
        },
        variant: SectionVariant::Burn(amount),
    })
}

fn parse_token_type(pushdata: &mut Bytes) -> Result<TokenType, ParseError> {
    let token_type = read_array::<1>(pushdata)?[0];
    Ok(match token_type {
        STANDARD_TOKEN_TYPE => TokenType::Standard,
        _ => TokenType::Unknown(token_type),
    })
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

fn read_amount(pushdata: &mut Bytes) -> Result<Amount, ParseError> {
    Ok(Amount::from_bytes(read_array(pushdata)?))
}

fn read_amounts(pushdata: &mut Bytes) -> Result<Vec<Amount>, ParseError> {
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
