use bitcoinsuite_core::{
    error::DataError,
    script::{opcode::*, Script},
    tx::{Tx, TxId},
};
use pretty_assertions::assert_eq;

use bitcoinsuite_slp::slp::{
    consts::OUTPUT_QUANTITY_FIELD_NAMES, parse, parse_tx, Amount, GenesisInfo,
    ParseData, ParseError, Token, TokenId, TokenType, TxType,
};

#[test]
fn test_parse_slp() -> Result<(), ParseError> {
    fn check_script(script: &[u8], expected_err: ParseError) {
        assert_eq!(
            parse(&TxId::default(), &Script::new(script.to_vec().into()), 0,),
            Err(expected_err),
        );
    }
    // No outputs
    assert_eq!(parse_tx(&Tx::default()), Err(ParseError::NoOutputs));
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
        check_script(&script, ParseError::DisallowedPush { opcode, op_idx });
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
            0x6a, 0x4c, 0x04, b'S', b'L', b'P', 0x00, 0x4c, 0x00, 0x01, 0x00,
        ],
        ParseError::InvalidTokenType(vec![].into()),
    );
    check_script(
        &[
            0x6a, 0x4d, 0x04, 0x00, b'S', b'L', b'P', 0x00, 0x4c, 0x00, 0x01,
            0x00,
        ],
        ParseError::InvalidTokenType(vec![].into()),
    );
    check_script(
        &[
            0x6a, 0x4e, 0x04, 0x00, 0x00, 0x00, b'S', b'L', b'P', 0x00, 0x4c,
            0x00, 0x01, 0x00,
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
            0x6a, 0x04, b'S', b'L', b'P', 0x00, 0x03, 0x99, 0x99, 0x99, 0x01,
            0x00,
        ],
        ParseError::InvalidTokenType(vec![0x99, 0x99, 0x99].into()),
    );
    // Unknown token type (no error, but results in "Unknown" fields)
    assert_eq!(
        parse(
            &TxId::default(),
            &Script::new(
                vec![
                    0x6a, 0x04, b'S', b'L', b'P', 0x00, 0x02, 0x99, 0x99, 0x01,
                    0x00
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
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
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
                100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
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
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
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
                        &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, qty],
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
                    100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
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
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100
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
                            44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                        ],
                        &[0x01, 0x02],
                        &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 231],
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
                    44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
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
                        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
                    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, idx as u8,
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
                        &[0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 231],
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
