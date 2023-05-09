use bitcoinsuite_core::{
    error::DataError,
    script::{opcode::*, Op, Script},
    tx::{Tx, TxId},
};
use bytes::Bytes;
use thiserror::Error;

use crate::slp::{
    consts::{
        LOKAD_ID, OUTPUT_QUANTITY_FIELD_NAMES, TOKEN_TYPE_V1,
        TOKEN_TYPE_V1_NFT1_CHILD, TOKEN_TYPE_V1_NFT1_GROUP,
    },
    Amount, GenesisInfo, Token, TokenId, TokenType, TxType,
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
            ParseError::NoOpcodes
                | ParseError::MissingOpReturn { .. }
                | ParseError::InvalidLokadId { .. }
                | ParseError::DataError { .. }
        )
    }
}

pub fn parse_tx(tx: &Tx) -> Result<ParseData, ParseError> {
    if tx.outputs.is_empty() {
        return Err(ParseError::NoOutputs);
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
        return Err(ParseError::TooFewPushes {
            actual: opreturn_data.len(),
            expected: 3,
        });
    }
    if opreturn_data[1].is_empty() || opreturn_data[1].len() > 2 {
        return Err(ParseError::InvalidTokenType(opreturn_data[1].clone()));
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
        b"GENESIS" => parse_genesis_data(opreturn_data, token_type)?,
        b"MINT" => parse_mint_data(opreturn_data)?,
        b"SEND" => parse_send_data(opreturn_data)?,
        b"BURN" => parse_burn_data(opreturn_data)?,
        _ => return Err(ParseError::InvalidTxType(opreturn_data[2].clone())),
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
                return Err(ParseError::MissingOpReturn { opcode })
            }
            _ => {}
        }
        match op {
            Op::Code(opcode @ OP_0)
            | Op::Code(opcode @ Opcode(OP_1NEGATE::N..=OP_16::N)) => {
                return Err(ParseError::DisallowedPush { op_idx, opcode });
            }
            Op::Code(opcode) => {
                return Err(ParseError::NonPushOp { op_idx, opcode });
            }
            Op::Push(opcode, push) => {
                if opcode == OP_0 || opcode.number() > OP_PUSHDATA4::N {
                    return Err(ParseError::DisallowedPush { op_idx, opcode });
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
                return Err(ParseError::MissingOpReturn { opcode });
            }
        },
        None => return Err(ParseError::NoOpcodes),
    }
    match ops.get(1) {
        Some(op) => match op {
            &Op::Code(opcode) => {
                return Err(ParseError::InvalidLokadId(
                    vec![opcode.number()].into(),
                ))
            }
            Op::Push(_, bytes) => {
                if bytes.as_ref() != LOKAD_ID {
                    return Err(ParseError::InvalidLokadId(bytes.clone()));
                }
            }
        },
        None => return Err(ParseError::InvalidLokadId(vec![].into())),
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

#[cfg(test)]
mod tests {
    use bitcoinsuite_core::{
        error::DataError,
        script::{opcode::*, Script},
        tx::{Tx, TxId},
    };
    use pretty_assertions::assert_eq;

    use crate::slp::{
        consts::OUTPUT_QUANTITY_FIELD_NAMES, parse, parse_tx, Amount,
        GenesisInfo, ParseData, ParseError, Token, TokenId, TokenType, TxType,
    };

    #[test]
    fn test_parse_slp() -> Result<(), ParseError> {
        fn check_script(script: &[u8], expected_err: ParseError) {
            assert_eq!(
                parse(
                    &TxId::default(),
                    &Script::new(script.to_vec().into()),
                    0,
                ),
                Err(expected_err),
            );
        }
        // No outputs
        assert_eq!(parse_tx(&Tx::default()), Err(ParseError::NoOutputs),);
        // Invalid OP_RETURN script
        check_script(
            &[0x01],
            ParseError::DataError(DataError::InvalidLength {
                expected: 1,
                actual: 0,
            }),
        );
        // Missing OP_RETURN opcode
        check_script(&[], ParseError::NoOpcodes);
        check_script(
            &[0xac],
            ParseError::MissingOpReturn {
                opcode: OP_CHECKSIG,
            },
        );
        // Disallowed push
        let mut scripts: Vec<(&[_], Opcode, usize)> = vec![
            (&[0x00], OP_0, 2),
            (&[0x4f], OP_1NEGATE, 2),
            (&[0x4c, 0x00, 0x51], OP_1, 3),
            (&[0x4d, 0x00, 0x00, 0x52], OP_2, 3),
            (&[0x4e, 0x00, 0x00, 0x00, 0x00, 0x53], OP_3, 3),
            (&[0x01, 0x00, 0x54], OP_4, 3),
            (&[0x02, 0x00, 0x00, 0x55], OP_5, 3),
            (&[0x56], OP_6, 2),
            (&[0x57], OP_7, 2),
            (&[0x58], OP_8, 2),
            (&[0x59], OP_9, 2),
            (&[0x5a], OP_10, 2),
            (&[0x5b], OP_11, 2),
            (&[0x5c], OP_12, 2),
            (&[0x5d], OP_13, 2),
            (&[0x5e], OP_14, 2),
            (&[0x5f], OP_15, 2),
            (&[0x60], OP_16, 2),
        ];
        let script = [[0x4b].as_ref(), &[0x00; 0x4b], &[0x00]].concat();
        scripts.push((&script, OP_0, 3));
        for (script, opcode, op_idx) in scripts {
            let script = [[0x6a, 0x04].as_ref(), b"SLP\0", script].concat();
            check_script(
                &script,
                ParseError::DisallowedPush { opcode, op_idx },
            );
        }
        // Non-pushop
        for opcode in 0x61..=0xff {
            check_script(
                &[[0x6a, 0x04].as_ref(), b"SLP\0", &[opcode]].concat(),
                ParseError::NonPushOp {
                    opcode: Opcode(opcode),
                    op_idx: 2,
                },
            );
        }
        // Too few pushes
        let scripts = [
            &[[0x6a, 0x04].as_ref(), b"SLP\0"].concat(),
            &[[0x6a, 0x04].as_ref(), b"SLP\0", &[0x01, 0x00]].concat(),
        ];
        for (num_pushes, script) in scripts.into_iter().enumerate() {
            check_script(
                script,
                ParseError::TooFewPushes {
                    expected: 3,
                    actual: num_pushes + 1,
                },
            );
        }
        // Invalid LOKAD ID
        check_script(&[0x6a], ParseError::InvalidLokadId(vec![].into()));
        check_script(
            &[0x6a, 0x01, 0x00],
            ParseError::InvalidLokadId(vec![0x00].into()),
        );
        check_script(
            &[0x6a, 0x01, 0x00, 0x01, 0x00],
            ParseError::InvalidLokadId(vec![0x00].into()),
        );
        check_script(
            &[0x6a, 0x03, b'S', b'L', b'P'],
            ParseError::InvalidLokadId(b"SLP".as_ref().into()),
        );
        check_script(
            &[0x6a, 0x04, b'S', b'L', b'P', 0x99],
            ParseError::InvalidLokadId(b"SLP\x99".as_ref().into()),
        );
        // Valid Lokad ID (using OP_PUSHDATA1, OP_PUSHDATA2 and OP_PUSHDATA4)
        check_script(
            &[
                0x6a, 0x4c, 0x04, b'S', b'L', b'P', 0x00, 0x4c, 0x00, 0x01,
                0x00,
            ],
            ParseError::InvalidTokenType(vec![].into()),
        );
        check_script(
            &[
                0x6a, 0x4d, 0x04, 0x00, b'S', b'L', b'P', 0x00, 0x4c, 0x00,
                0x01, 0x00,
            ],
            ParseError::InvalidTokenType(vec![].into()),
        );
        check_script(
            &[
                0x6a, 0x4e, 0x04, 0x00, 0x00, 0x00, b'S', b'L', b'P', 0x00,
                0x4c, 0x00, 0x01, 0x00,
            ],
            ParseError::InvalidTokenType(vec![].into()),
        );
        // Invalid token type
        check_script(
            &[0x6a, 0x04, b'S', b'L', b'P', 0x00, 0x4c, 0x00, 0x01, 0x00],
            ParseError::InvalidTokenType(vec![].into()),
        );
        check_script(
            &[
                0x6a, 0x04, b'S', b'L', b'P', 0x00, 0x03, 0x99, 0x99, 0x99,
                0x01, 0x00,
            ],
            ParseError::InvalidTokenType(vec![0x99, 0x99, 0x99].into()),
        );
        // Unknown token type (no error, but results in "Unknown" fields)
        assert_eq!(
            parse(
                &TxId::default(),
                &Script::new(
                    vec![
                        0x6a, 0x04, b'S', b'L', b'P', 0x00, 0x02, 0x99, 0x99,
                        0x01, 0x00
                    ]
                    .into()
                ),
                1,
            ),
            Ok(ParseData {
                output_tokens: vec![Token::EMPTY],
                token_type: TokenType::Unknown,
                tx_type: TxType::Unknown,
                token_id: TokenId::from_be_bytes([0; 32]),
            }),
        );
        // Invalid tx type
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"INVALID",
            ]
            .concat(),
            ParseError::InvalidTxType(b"INVALID".as_ref().into()),
        );
        // Invalid GENESIS
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
            ]
            .concat(),
            ParseError::TooFewPushesExact {
                expected: 10,
                actual: 3,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::SuperfluousPushes {
                expected: 10,
                actual: 11,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "token_document_hash",
                actual: 1,
                expected: &[0, 32],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x4c, 0x00],
                &[0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "decimals",
                actual: 0,
                expected: &[1],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x02, 0x00, 0x00],
                &[0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "decimals",
                actual: 2,
                expected: &[1],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x01, 0x00],
                &[0x02, 0x00, 0x00],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "mint_baton_out_idx",
                actual: 2,
                expected: &[0, 1],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x01, 0x00],
                &[0x01, 0x00],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "initial_quantity",
                actual: 1,
                expected: &[8],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x01, 10],
                &[0x01, 0x00],
                &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ]
            .concat(),
            ParseError::InvalidDecimals { actual: 10 },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x4c, 0x00],
                &[0x01, 0x09],
                &[0x01, 0x01],
                &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ]
            .concat(),
            ParseError::InvalidMintBatonIdx { actual: 0x01 },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 0x41],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x44, 0x01, 0x55, 0x01, 0x66, 0x4c, 0x00],
                &[0x01, 0x09],
                &[0x01, 0x02],
                &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ]
            .concat(),
            ParseError::Nft1ChildCannotHaveMintBaton,
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 0x41],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x44, 0x01, 0x55, 0x01, 0x66, 0x4c, 0x00],
                &[0x01, 0x09],
                &[0x4c, 0x00],
                &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 123],
            ]
            .concat(),
            ParseError::Nft1ChildInvalidInitialQuantity { actual: 123 },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 0x41],
                &[0x07],
                b"GENESIS",
                &[0x01, 0x44, 0x01, 0x55, 0x01, 0x66, 0x4c, 0x00],
                &[0x01, 0x09],
                &[0x4c, 0x00],
                &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
            ]
            .concat(),
            ParseError::Nft1ChildInvalidDecimals { actual: 9 },
        );
        // Valid GENESIS
        assert_eq!(
            parse(
                &TxId::from([
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
                ]),
                &Script::new(
                    [
                        [0x6a, 0x04].as_ref(),
                        b"SLP\0",
                        &[0x01, 1],
                        &[0x07],
                        b"GENESIS",
                        &[0x01, 0x44, 0x01, 0x55, 0x01, 0x66, 0x4c, 0x00],
                        &[0x01, 0x09],
                        &[0x01, 0x02],
                        &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 123],
                    ]
                    .concat()
                    .into()
                ),
                3
            ),
            Ok(ParseData {
                output_tokens: vec![
                    Token::EMPTY,
                    Token::amount(123),
                    Token::MINT_BATON
                ],
                token_type: TokenType::Fungible,
                tx_type: TxType::Genesis(Box::new(GenesisInfo {
                    token_ticker: vec![0x44].into(),
                    token_name: vec![0x55].into(),
                    token_document_url: vec![0x66].into(),
                    token_document_hash: None,
                    decimals: 9
                })),
                token_id: TokenId::from_be_bytes([
                    100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ]),
            }),
        );
        for (type_byte, token_type) in [
            (1, TokenType::Fungible),
            (0x41, TokenType::Nft1Child),
            (0x81, TokenType::Nft1Group),
        ] {
            let qty = match token_type {
                TokenType::Nft1Child => 1,
                _ => 123,
            };
            assert_eq!(
                parse(
                    &TxId::from([
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
                    ]),
                    &Script::new(
                        [
                            [0x6a, 0x04].as_ref(),
                            b"SLP\0",
                            &[0x01, type_byte],
                            &[0x07],
                            b"GENESIS",
                            &[0x01, 0x44, 0x01, 0x55, 0x01, 0x66, 0x4c, 0x00],
                            match token_type {
                                TokenType::Nft1Child => &[0x01, 0x00],
                                _ => &[0x01, 0x09],
                            },
                            match token_type {
                                TokenType::Nft1Child => &[0x4c, 0x00],
                                _ => &[0x01, 0x02],
                            },
                            &[
                                0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                qty
                            ],
                        ]
                        .concat()
                        .into()
                    ),
                    3
                ),
                Ok(ParseData {
                    output_tokens: vec![
                        Token::EMPTY,
                        Token::amount(qty as Amount),
                        match token_type {
                            TokenType::Nft1Child => Token::EMPTY,
                            _ => Token::MINT_BATON,
                        },
                    ],
                    token_type,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo {
                        token_ticker: vec![0x44].into(),
                        token_name: vec![0x55].into(),
                        token_document_url: vec![0x66].into(),
                        token_document_hash: None,
                        decimals: match token_type {
                            TokenType::Nft1Child => 0,
                            _ => 9,
                        },
                    })),
                    token_id: TokenId::from_be_bytes([
                        100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                    ]),
                }),
            );
        }
        // Invalid MINT
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
            ]
            .concat(),
            ParseError::TooFewPushesExact {
                expected: 6,
                actual: 3,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::SuperfluousPushes {
                expected: 6,
                actual: 7,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "token_id",
                actual: 1,
                expected: &[32],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
                &[0x20],
                &[0x44; 32],
                &[0x02, 0x00, 0x00],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "mint_baton_out_idx",
                actual: 2,
                expected: &[0, 1],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
                &[0x20],
                &[0x44; 32],
                &[0x01, 0x01],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidMintBatonIdx { actual: 1 },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"MINT",
                &[0x20],
                &[0x44; 32],
                &[0x01, 0x02],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "additional_quantity",
                actual: 1,
                expected: &[8],
            },
        );
        // Valid MINT
        for (type_byte, token_type) in [
            (1, TokenType::Fungible),
            (0x41, TokenType::Nft1Child),
            (0x81, TokenType::Nft1Group),
        ] {
            assert_eq!(
                parse(
                    &TxId::from([
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
                    ]),
                    &Script::new(
                        vec![
                            [0x6a, 0x04].as_ref(),
                            b"SLP\0",
                            &[0x01, type_byte],
                            &[0x04],
                            b"MINT",
                            &[0x20],
                            &[
                                44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                0
                            ],
                            &[0x01, 0x02],
                            &[
                                0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                231
                            ],
                        ]
                        .concat()
                        .into()
                    ),
                    3
                ),
                Ok(ParseData {
                    output_tokens: vec![
                        Token::EMPTY,
                        Token::amount(231),
                        Token::MINT_BATON,
                    ],
                    token_type,
                    tx_type: TxType::Mint,
                    token_id: TokenId::from_be_bytes([
                        44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                    ]),
                }),
            );
        }
        // Invalid SEND
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"SEND",
            ]
            .concat(),
            ParseError::TooFewPushes {
                expected: 5,
                actual: 3,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"SEND",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::SuperfluousPushes {
                expected: 23,
                actual: 24,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"SEND",
                &[0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "token_id",
                expected: &[32],
                actual: 1,
            },
        );
        // Test all possible SEND outputs with one amount having 2 bytes
        for num_outputs in 1..=19 {
            let script_intro = [
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"SEND",
                &[0x20],
                &[0x22; 32],
            ]
            .concat();
            for (invalid_idx, field_name) in OUTPUT_QUANTITY_FIELD_NAMES
                .iter()
                .enumerate()
                .take(num_outputs)
            {
                let mut script = script_intro.clone();
                for idx in 0..num_outputs {
                    if invalid_idx == idx {
                        script.extend([0x02, 0x00, 0x00]);
                    } else {
                        script.extend([
                            0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00,
                        ]);
                    }
                }
                check_script(
                    &script,
                    ParseError::InvalidFieldSize {
                        field_name,
                        expected: &[8],
                        actual: 2,
                    },
                );
            }
        }
        // Valid SEND
        for (type_byte, token_type) in [
            (1, TokenType::Fungible),
            (0x41, TokenType::Nft1Child),
            (0x81, TokenType::Nft1Group),
        ] {
            let script_intro = [
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, type_byte],
                &[0x04],
                b"SEND",
                &[0x20],
                &[0x22; 32],
            ]
            .concat();
            for num_amounts in 1..=19 {
                let mut script = script_intro.clone();
                let mut amounts = vec![Token::EMPTY];
                for idx in 1..=num_amounts {
                    script.extend([
                        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        idx as u8,
                    ]);
                    amounts.push(Token::amount(idx));
                }
                // output_tokens is independent of tx.outputs
                for num_tx_outputs in 1..=20 {
                    assert_eq!(
                        parse(
                            &TxId::from([3; 32]),
                            &Script::new(script.clone().into()),
                            num_tx_outputs,
                        ),
                        Ok(ParseData {
                            output_tokens: amounts.clone(),
                            token_type,
                            tx_type: TxType::Send,
                            token_id: TokenId::from_be_bytes([0x22; 32]),
                        }),
                    );
                }
            }
        }
        // Invalid burn
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"BURN",
            ]
            .concat(),
            ParseError::TooFewPushesExact {
                expected: 5,
                actual: 3,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"BURN",
                &[0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::SuperfluousPushes {
                expected: 5,
                actual: 6,
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"BURN",
                &[0x01, 0x00, 0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "token_id",
                actual: 1,
                expected: &[32],
            },
        );
        check_script(
            &[
                [0x6a, 0x04].as_ref(),
                b"SLP\0",
                &[0x01, 1],
                &[0x04],
                b"BURN",
                &[0x20],
                &[0x44; 32],
                &[0x01, 0x00],
            ]
            .concat(),
            ParseError::InvalidFieldSize {
                field_name: "token_burn_quantity",
                actual: 1,
                expected: &[8],
            },
        );
        // Valid burn
        for (type_byte, token_type) in [
            (1, TokenType::Fungible),
            (0x41, TokenType::Nft1Child),
            (0x81, TokenType::Nft1Group),
        ] {
            assert_eq!(
                parse(
                    &TxId::from([3; 32]),
                    &Script::new(
                        [
                            [0x6a, 0x04].as_ref(),
                            b"SLP\0",
                            &[0x01, type_byte],
                            &[0x04],
                            b"BURN",
                            &[0x20],
                            &[0x44; 32],
                            &[
                                0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                231
                            ],
                        ]
                        .concat()
                        .into()
                    ),
                    3,
                ),
                Ok(ParseData {
                    output_tokens: vec![Token::EMPTY; 3],
                    token_type,
                    tx_type: TxType::Burn(231),
                    token_id: TokenId::from_be_bytes([0x44; 32]),
                }),
            );
        }
        Ok(())
    }
}
