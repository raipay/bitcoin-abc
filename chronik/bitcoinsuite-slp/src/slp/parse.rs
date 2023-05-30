use bitcoinsuite_core::{
    error::DataError,
    script::{opcode::*, Op, Script},
    tx::{Tx, TxId},
};
use bytes::Bytes;
use thiserror::Error;

use crate::{
    common::{BURN, GENESIS, MINT, SEND},
    slp::{
        consts::{
            OUTPUT_QUANTITY_FIELD_NAMES, SLP_LOKAD_ID, TOKEN_TYPE_V1,
            TOKEN_TYPE_V1_NFT1_CHILD, TOKEN_TYPE_V1_NFT1_GROUP,
        },
        Amount, GenesisInfo, Token, TokenId, TokenType, TxType,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseData {
    pub output_tokens: Vec<Token>,
    pub token_type: TokenType,
    pub tx_type: TxType,
    /// 0000...000000 if token_id is incomplete
    pub token_id: TokenId,
}

/// Errors forwhen parsing a SLPv2 tx.
#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Failed parsing pushdata: {0}")]
    DataError(#[from] DataError),

    #[error("First must be OP_RETURN (0x6a), but got {opcode}")]
    MissingOpReturn { opcode: Opcode },
    #[error("First must be OP_RETURN (0x6a), but got no opcodes")]
    NoOpcodes,
    #[error("Tx has no outputs")]
    NoOutputs,
    #[error("Non-push op: {opcode} at op {op_idx}")]
    NonPushOp { opcode: Opcode, op_idx: usize },
    #[error("Disallowed push: {opcode} at op {op_idx}")]
    DisallowedPush { opcode: Opcode, op_idx: usize },
    #[error(
        "Field has invalid length: expected one of {expected:?} but got {actual} for field \
        {field_name}"
    )]
    InvalidFieldSize {
        field_name: &'static str,
        expected: &'static [usize],
        actual: usize,
    },
    #[error("Too many decimals, only max. 9 allowed, but got {actual}")]
    InvalidDecimals { actual: usize },
    #[error("Mint baton at invalid output index, must be between 2 and 255, but got {actual}")]
    InvalidMintBatonIdx { actual: usize },
    #[error("NFT1 Child Genesis cannot have mint baton")]
    Nft1ChildCannotHaveMintBaton,
    #[error("Invalid NFT1 Child Genesis initial quantity, expected 1 but got {actual}")]
    Nft1ChildInvalidInitialQuantity { actual: Amount },
    #[error(
        "Invalid NFT1 Child Genesis decimals, expected 0 but got {actual}"
    )]
    Nft1ChildInvalidDecimals { actual: u32 },
    #[error(
        "Too few pushes, expected at least {expected} but only got {actual}"
    )]
    TooFewPushes { expected: usize, actual: usize },
    #[error(
        "Too few pushes, expected exactly {expected} but only got {actual}"
    )]
    TooFewPushesExact { expected: usize, actual: usize },
    #[error("Pushed superfluous data: expected at most {expected} pushes, but got {actual}")]
    SuperfluousPushes { expected: usize, actual: usize },
    #[error("Invalid LOKAD ID: {:?}", .0)]
    InvalidLokadId(Bytes),
    #[error("Token type has invalid length (1,2 != {}): {:?}", .0.len(), .0)]
    InvalidTokenType(Bytes),
    #[error("Invalid tx type: {:?}", .0)]
    InvalidTxType(Bytes),
    #[error("Invalid SEND: Output amounts ({output_sum}) exceed input amounts ({input_sum})")]
    OutputSumExceedInputSum {
        output_sum: Amount,
        input_sum: Amount,
    },
    #[error("Invalid NFT1 Child GENESIS: No group token")]
    HasNoNft1Group,
    #[error("Invalid MINT: No baton")]
    HasNoMintBaton,
    #[error("Invalid BURN: Burning the wrong token_id")]
    WrongBurnTokenId,
    #[error("Invalid BURN: Burning MINT baton")]
    WrongBurnMintBaton,
    #[error(
        "Invalid BURN: Burning invalid amount, expected {expected} but got {actual} base tokens"
    )]
    WrongBurnInvalidAmount { expected: Amount, actual: Amount },
    #[error("Found orphan txs")]
    FoundOrphanTx,
}

use self::ParseError::*;

impl ParseError {
    pub fn should_ignore(&self) -> bool {
        matches!(
            self,
            NoOutputs
                | NoOpcodes
                | MissingOpReturn { .. }
                | InvalidLokadId { .. }
                | ParseError::DataError { .. }
        )
    }
}

pub fn parse_tx(tx: &Tx) -> Result<ParseData, ParseError> {
    if tx.outputs.is_empty() {
        return Err(NoOutputs);
    }
    parse(tx.txid_ref(), &tx.outputs[0].script, tx.outputs.len())
}

pub fn parse(
    txid: &TxId,
    script: &Script,
    num_outputs: usize,
) -> Result<ParseData, ParseError> {
    let ops = script.iter_ops().collect::<Result<Vec<_>, _>>()?;
    parse_lokad_id(&ops)?;
    let opreturn_data = parse_opreturn_ops(ops.into_iter())?;
    if opreturn_data.len() < 3 {
        return Err(TooFewPushes {
            actual: opreturn_data.len(),
            expected: 3,
        });
    }
    if opreturn_data[1].is_empty() || opreturn_data[1].len() > 2 {
        return Err(InvalidTokenType(opreturn_data[1].clone()));
    }
    // Short circuit for unknown/unsupported token types
    let token_type = match parse_token_type(&opreturn_data[1]) {
        Some(token_type) => token_type,
        None => {
            let token = Token::EMPTY;
            return Ok(ParseData {
                output_tokens: (0..num_outputs).map(|_| token).collect(),
                token_type: TokenType::Unknown,
                tx_type: TxType::Unknown,
                token_id: TokenId::from_be_bytes([0; 32]),
            });
        }
    };

    let parsed_opreturn = match opreturn_data[2].as_ref() {
        GENESIS => parse_genesis_data(opreturn_data, token_type)?,
        MINT => parse_mint_data(opreturn_data)?,
        SEND => parse_send_data(opreturn_data)?,
        BURN => parse_burn_data(opreturn_data)?,
        _ => return Err(InvalidTxType(opreturn_data[2].clone())),
    };
    let token_id = match (&parsed_opreturn.tx_type, parsed_opreturn.token_id) {
        (TxType::Genesis(_), None) => TokenId::from_txid(*txid),
        (
            TxType::Mint | TxType::Send | TxType::Burn(_),
            Some(expected_token_id),
        ) => expected_token_id,
        _ => unreachable!(),
    };
    let mut output_tokens =
        (0..num_outputs).map(|_| Token::EMPTY).collect::<Vec<_>>();
    match parsed_opreturn.outputs {
        ParsedOutputs::MintTokens {
            mint_quantity,
            baton_out_idx,
        } => {
            if let Some(baton_out_idx) = baton_out_idx {
                if let Some(output_token) = output_tokens.get_mut(baton_out_idx)
                {
                    output_token.is_mint_baton = true;
                }
            }
            if let Some(output_token) = output_tokens.get_mut(1) {
                output_token.amount = mint_quantity;
            }
        }
        ParsedOutputs::Send(amounts) => {
            output_tokens.resize(amounts.len() + 1, Token::EMPTY);
            for (output_token, amount) in
                output_tokens.iter_mut().skip(1).zip(amounts)
            {
                output_token.amount = amount;
            }
        }
        ParsedOutputs::Burn => {}
    }
    Ok(ParseData {
        output_tokens,
        token_type,
        tx_type: parsed_opreturn.tx_type,
        token_id,
    })
}

fn parse_opreturn_ops(
    ops: impl Iterator<Item = Op>,
) -> Result<Vec<Bytes>, ParseError> {
    let mut pushes = Vec::new();
    for (op_idx, op) in ops.into_iter().enumerate() {
        // first opcode must be OP_RETURN
        match (op_idx, &op) {
            (0, Op::Code(OP_RETURN)) => continue,
            (0, &Op::Code(opcode)) | (0, &Op::Push(opcode, _)) => {
                return Err(MissingOpReturn { opcode })
            }
            _ => {}
        }
        match op {
            Op::Code(opcode @ OP_0)
            | Op::Code(opcode @ Opcode(OP_1NEGATE::N..=OP_16::N)) => {
                return Err(DisallowedPush { op_idx, opcode });
            }
            Op::Code(opcode) => {
                return Err(NonPushOp { op_idx, opcode });
            }
            Op::Push(opcode, push) => {
                if opcode == OP_0 || opcode.number() > OP_PUSHDATA4::N {
                    return Err(DisallowedPush { op_idx, opcode });
                }
                pushes.push(push);
            }
        }
    }
    Ok(pushes)
}

fn parse_lokad_id(ops: &[Op]) -> Result<(), ParseError> {
    match ops.get(0) {
        Some(op) => match op {
            &Op::Code(OP_RETURN) => {}
            &Op::Code(opcode) | &Op::Push(opcode, _) => {
                return Err(MissingOpReturn { opcode });
            }
        },
        None => return Err(NoOpcodes),
    }
    match ops.get(1) {
        Some(op) => match op {
            &Op::Code(opcode) => {
                return Err(InvalidLokadId(
                    vec![opcode.number()].into(),
                ))
            }
            Op::Push(_, bytes) => {
                if bytes.as_ref() != SLP_LOKAD_ID {
                    return Err(InvalidLokadId(bytes.clone()));
                }
            }
        },
        None => return Err(InvalidLokadId(vec![].into())),
    }
    Ok(())
}

fn parse_token_type(bytes: &Bytes) -> Option<TokenType> {
    if bytes.as_ref() == TOKEN_TYPE_V1 {
        Some(TokenType::Fungible)
    } else if bytes.as_ref() == TOKEN_TYPE_V1_NFT1_GROUP {
        Some(TokenType::Nft1Group)
    } else if bytes.as_ref() == TOKEN_TYPE_V1_NFT1_CHILD {
        Some(TokenType::Nft1Child)
    } else {
        None
    }
}

struct ParsedOpReturn {
    tx_type: TxType,
    outputs: ParsedOutputs,
    token_id: Option<TokenId>,
}

enum ParsedOutputs {
    MintTokens {
        baton_out_idx: Option<usize>,
        mint_quantity: Amount,
    },
    Send(Vec<Amount>),
    Burn,
}

fn parse_genesis_data(
    opreturn_data: Vec<Bytes>,
    slp_token_type: TokenType,
) -> Result<ParsedOpReturn, ParseError> {
    if opreturn_data.len() < 10 {
        return Err(ParseError::TooFewPushesExact {
            expected: 10,
            actual: opreturn_data.len(),
        });
    }
    if opreturn_data.len() > 10 {
        return Err(ParseError::SuperfluousPushes {
            expected: 10,
            actual: opreturn_data.len(),
        });
    }
    let mut data_iter = opreturn_data.into_iter();
    let _lokad_id = data_iter.next().unwrap();
    let _token_type = data_iter.next().unwrap();
    let _tx_type = data_iter.next().unwrap();
    let token_ticker = data_iter.next().unwrap();
    let token_name = data_iter.next().unwrap();
    let token_document_url = data_iter.next().unwrap();
    let token_document_hash = data_iter.next().unwrap();
    let decimals = data_iter.next().unwrap();
    let mint_baton_out_idx = data_iter.next().unwrap();
    let initial_quantity = data_iter.next().unwrap();
    assert!(data_iter.next().is_none());
    if !token_document_hash.is_empty() && token_document_hash.len() != 32 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "token_document_hash",
            expected: &[0, 32],
            actual: token_document_hash.len(),
        });
    }
    if decimals.len() != 1 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "decimals",
            expected: &[1],
            actual: decimals.len(),
        });
    }
    if !mint_baton_out_idx.is_empty() && mint_baton_out_idx.len() != 1 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "mint_baton_out_idx",
            expected: &[0, 1],
            actual: mint_baton_out_idx.len(),
        });
    }
    let initial_quantity = parse_amount(&initial_quantity, "initial_quantity")?;
    if decimals[0] > 9 {
        return Err(ParseError::InvalidDecimals {
            actual: decimals[0] as usize,
        });
    }
    if mint_baton_out_idx.len() == 1 && mint_baton_out_idx[0] < 2 {
        return Err(ParseError::InvalidMintBatonIdx {
            actual: mint_baton_out_idx[0] as usize,
        });
    }
    let decimals = decimals[0] as u32;
    if slp_token_type == TokenType::Nft1Child {
        if !mint_baton_out_idx.is_empty() {
            return Err(ParseError::Nft1ChildCannotHaveMintBaton);
        }
        if initial_quantity != 1 {
            return Err(ParseError::Nft1ChildInvalidInitialQuantity {
                actual: initial_quantity,
            });
        }
        if decimals != 0 {
            return Err(ParseError::Nft1ChildInvalidDecimals {
                actual: decimals,
            });
        }
    }
    Ok(ParsedOpReturn {
        tx_type: TxType::Genesis(Box::new(GenesisInfo {
            token_ticker,
            token_name,
            token_document_url,
            token_document_hash: token_document_hash.as_ref().try_into().ok(),
            decimals,
        })),
        outputs: ParsedOutputs::MintTokens {
            baton_out_idx: mint_baton_out_idx
                .first()
                .map(|&mint_baton_out_idx| mint_baton_out_idx as usize),
            mint_quantity: initial_quantity,
        },
        token_id: None,
    })
}

fn parse_mint_data(
    opreturn_data: Vec<Bytes>,
) -> Result<ParsedOpReturn, ParseError> {
    if opreturn_data.len() < 6 {
        return Err(ParseError::TooFewPushesExact {
            expected: 6,
            actual: opreturn_data.len(),
        });
    }
    if opreturn_data.len() > 6 {
        return Err(ParseError::SuperfluousPushes {
            expected: 6,
            actual: opreturn_data.len(),
        });
    }
    let mut data_iter = opreturn_data.into_iter();
    let _lokad_id = data_iter.next().unwrap();
    let _token_type = data_iter.next().unwrap();
    let _tx_type = data_iter.next().unwrap();
    let token_id = data_iter.next().unwrap();
    let mint_baton_out_idx = data_iter.next().unwrap();
    let additional_quantity = data_iter.next().unwrap();
    assert!(data_iter.next().is_none());
    if token_id.len() != 32 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "token_id",
            expected: &[32],
            actual: token_id.len(),
        });
    }
    if !(0..=1).contains(&mint_baton_out_idx.len()) {
        return Err(ParseError::InvalidFieldSize {
            field_name: "mint_baton_out_idx",
            expected: &[0, 1],
            actual: mint_baton_out_idx.len(),
        });
    }
    if mint_baton_out_idx.len() == 1 && mint_baton_out_idx[0] < 2 {
        return Err(ParseError::InvalidMintBatonIdx {
            actual: mint_baton_out_idx[0] as usize,
        });
    }
    let additional_quantity =
        parse_amount(&additional_quantity, "additional_quantity")?;
    Ok(ParsedOpReturn {
        tx_type: TxType::Mint,
        outputs: ParsedOutputs::MintTokens {
            baton_out_idx: mint_baton_out_idx
                .first()
                .map(|&mint_baton_out_idx| mint_baton_out_idx as usize),
            mint_quantity: additional_quantity,
        },
        token_id: Some(TokenId::from_be_bytes(
            token_id.as_ref().try_into().unwrap(),
        )),
    })
}

fn parse_send_data(
    opreturn_data: Vec<Bytes>,
) -> Result<ParsedOpReturn, ParseError> {
    if opreturn_data.len() < 5 {
        return Err(ParseError::TooFewPushes {
            expected: 5,
            actual: opreturn_data.len(),
        });
    }
    if opreturn_data.len() > 23 {
        return Err(ParseError::SuperfluousPushes {
            expected: 23,
            actual: opreturn_data.len(),
        });
    }
    let mut data_iter = opreturn_data.into_iter();
    let _lokad_id = data_iter.next().unwrap();
    let _token_type = data_iter.next().unwrap();
    let _tx_type = data_iter.next().unwrap();
    let token_id = data_iter.next().unwrap();
    let output_quantities = data_iter;
    if token_id.len() != 32 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "token_id",
            expected: &[32],
            actual: token_id.len(),
        });
    }
    let output_quantities = output_quantities
        .enumerate()
        .map(|(idx, quantity)| {
            parse_amount(&quantity, OUTPUT_QUANTITY_FIELD_NAMES[idx])
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ParsedOpReturn {
        tx_type: TxType::Send,
        outputs: ParsedOutputs::Send(output_quantities),
        token_id: Some(TokenId::from_be_bytes(
            token_id.as_ref().try_into().unwrap(),
        )),
    })
}

fn parse_burn_data(
    opreturn_data: Vec<Bytes>,
) -> Result<ParsedOpReturn, ParseError> {
    if opreturn_data.len() < 5 {
        return Err(ParseError::TooFewPushesExact {
            expected: 5,
            actual: opreturn_data.len(),
        });
    }
    if opreturn_data.len() > 5 {
        return Err(ParseError::SuperfluousPushes {
            expected: 5,
            actual: opreturn_data.len(),
        });
    }
    let mut data_iter = opreturn_data.into_iter();
    let _lokad_id = data_iter.next().unwrap();
    let _token_type = data_iter.next().unwrap();
    let _tx_type = data_iter.next().unwrap();
    let token_id = data_iter.next().unwrap();
    let token_burn_quantity = data_iter.next().unwrap();
    if token_id.len() != 32 {
        return Err(ParseError::InvalidFieldSize {
            field_name: "token_id",
            expected: &[32],
            actual: token_id.len(),
        });
    }
    let token_burn_quantity =
        parse_amount(&token_burn_quantity, "token_burn_quantity")?;
    Ok(ParsedOpReturn {
        tx_type: TxType::Burn(token_burn_quantity),
        outputs: ParsedOutputs::Burn,
        token_id: Some(TokenId::from_be_bytes(
            token_id.as_ref().try_into().unwrap(),
        )),
    })
}

fn parse_amount(
    amount_bytes: &[u8],
    field_name: &'static str,
) -> Result<Amount, ParseError> {
    Ok(Amount::from_be_bytes(amount_bytes.try_into().map_err(
        |_| ParseError::InvalidFieldSize {
            field_name,
            expected: &[8],
            actual: amount_bytes.len(),
        },
    )?))
}
