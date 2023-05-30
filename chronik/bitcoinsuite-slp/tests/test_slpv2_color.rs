use std::borrow::Cow;

use bitcoinsuite_core::{
    script::Script,
    tx::{Tx, TxId, TxMut, TxOutput},
};
use bitcoinsuite_slp::{
    empp,
    slpv2::{
        burn_section, genesis_section, mint_section, sections_opreturn,
        send_section, Amount, ColorError, ColorSectionError, ColoredTx,
        GenesisInfo, IntentionalBurn, MintData, SectionData, SectionType,
        Token, TokenData, TokenId, TokenMeta, TokenType, TokenVariant,
    },
};
use bytes::Bytes;
use pretty_assertions::assert_eq;

const TXID: TxId = TxId::new([4; 32]);
const TOKEN_ID: TokenId = TokenId::new(TXID);
const TXID2: TxId = TxId::new([5; 32]);
const TOKEN_ID2: TokenId = TokenId::new(TXID2);
const TXID3: TxId = TxId::new([6; 32]);
const TOKEN_ID3: TokenId = TokenId::new(TXID3);
const TXID4: TxId = TxId::new([7; 32]);
const TOKEN_ID4: TokenId = TokenId::new(TXID4);

const _0: Amount = Amount::ZERO;
const _1: Amount = Amount::new(1);
const _2: Amount = Amount::new(2);
const _3: Amount = Amount::new(3);
const _7: Amount = Amount::new(7);
const MAX: Amount = Amount::new(0xffff_ffff_ffff);

fn make_tx(script: Script, num_extra_outputs: usize) -> Tx {
    Tx::with_txid(
        TXID,
        TxMut {
            outputs: std::iter::once(TxOutput { value: 0, script })
                .chain(
                    std::iter::repeat(TxOutput::default())
                        .take(num_extra_outputs),
                )
                .collect(),
            ..Default::default()
        },
    )
}

fn make_genesis<const N: usize>(
    amounts: [Amount; N],
    num_batons: usize,
) -> Bytes {
    genesis_section(
        TokenType::Standard,
        &GenesisInfo::default(),
        &MintData {
            amounts: amounts.into_iter().collect(),
            num_batons,
        },
    )
}

fn make_mint<const N: usize>(
    token_id: &TokenId,
    amounts: [Amount; N],
    num_batons: usize,
) -> Bytes {
    mint_section(
        token_id,
        TokenType::Standard,
        &MintData {
            amounts: amounts.into_iter().collect(),
            num_batons,
        },
    )
}

fn make_send<const N: usize>(
    token_id: &TokenId,
    amounts: [Amount; N],
) -> Bytes {
    send_section(token_id, TokenType::Standard, amounts.into_iter())
}

fn make_burn(token_id: &TokenId, amount: Amount) -> Bytes {
    burn_section(token_id, TokenType::Standard, amount)
}

#[test]
fn test_color_slpv2_no_outputs() {
    assert_eq!(
        ColoredTx::parse_tx(&Tx::default()),
        ColoredTx {
            errors: vec![ColorError::NoOutputs],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_empty_script() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(Script::default(), 0)),
        ColoredTx {
            outputs: vec![None],
            errors: vec![ColorError::EmppError(empp::ParseError::EmptyScript)],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_genesis_must_be_first() {
    let token_type = TokenType::Standard;
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_send(&TokenId::default(), []),
                make_genesis([], 0),
            ]),
            0,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta {
                    token_id: TokenId::default(),
                    token_type,
                },
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            outputs: vec![None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::GenesisMustBeFirst,
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_genesis_too_few_outputs() {
    for mint_data in [
        MintData {
            amounts: vec![_0],
            num_batons: 0,
        },
        MintData {
            amounts: vec![],
            num_batons: 1,
        },
    ] {
        assert_eq!(
            ColoredTx::parse_tx(&make_tx(
                sections_opreturn(vec![genesis_section(
                    TokenType::Standard,
                    &GenesisInfo::default(),
                    &mint_data,
                )]),
                0,
            )),
            ColoredTx {
                outputs: vec![None],
                errors: vec![ColorError::SectionError {
                    pushdata_idx: 0,
                    error: ColorSectionError::TooFewOutputs {
                        expected: 2,
                        actual: 1,
                    },
                }],
                ..Default::default()
            },
        );
    }
}

#[test]
fn test_color_slpv2_genesis_success_simple() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![genesis_section(
                TokenType::Standard,
                &GenesisInfo::default(),
                &MintData::default(),
            )]),
            0,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID),
                section_type: SectionType::GENESIS,
                genesis_info: Some(GenesisInfo::default()),
            }],
            outputs: vec![None],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_genesis_success_big() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![genesis_section(
                TokenType::Standard,
                &GenesisInfo::default(),
                &MintData {
                    amounts: vec![_0, _7, MAX, _0],
                    num_batons: 3,
                },
            )]),
            9,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID),
                section_type: SectionType::GENESIS,
                genesis_info: Some(GenesisInfo::default()),
            }],
            outputs: vec![
                None,
                None,
                Some(TokenData::amount(0, _7)),
                Some(TokenData::amount(0, MAX)),
                None,
                Some(TokenData::mint_baton(0)),
                Some(TokenData::mint_baton(0)),
                Some(TokenData::mint_baton(0)),
                None,
                None,
            ],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_mint_duplicate_token_id() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_mint(&TOKEN_ID2, [], 0),
                make_mint(&TOKEN_ID2, [], 0),
            ]),
            0,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::MINT,
                genesis_info: None,
            }],
            outputs: vec![None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::DuplicateTokenId {
                    prev_section_idx: 0,
                    token_id: TOKEN_ID2,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_mint_unknown_token_type() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_mint(&TOKEN_ID2, [_2], 0),
                b"SLP2\x77".as_ref().into(),
                make_mint(&TOKEN_ID3, [], 0),
                b"SLP2\x89".as_ref().into(),
            ]),
            3,
        )),
        ColoredTx {
            sections: vec![
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::MINT,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta {
                        token_id: TokenId::from([0; 32]),
                        token_type: TokenType::Unknown(0x77),
                    },
                    section_type: SectionType::UNKNOWN,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta {
                        token_id: TokenId::from([0; 32]),
                        token_type: TokenType::Unknown(0x89),
                    },
                    section_type: SectionType::UNKNOWN,
                    genesis_info: None,
                },
            ],
            outputs: vec![
                None,
                Some(TokenData::amount(0, _2)),
                Some(TokenData::unknown(1)),
                Some(TokenData::unknown(1)),
            ],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 2,
                error: ColorSectionError::DescendingTokenType {
                    before: 0x77,
                    after: 0,
                },
            },],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_mint_too_few_outputs() {
    for section in
        [make_mint(&TOKEN_ID2, [_0], 0), make_mint(&TOKEN_ID2, [], 1)]
    {
        assert_eq!(
            ColoredTx::parse_tx(&make_tx(sections_opreturn(vec![section]), 0,)),
            ColoredTx {
                outputs: vec![None],
                errors: vec![ColorError::SectionError {
                    pushdata_idx: 0,
                    error: ColorSectionError::TooFewOutputs {
                        expected: 2,
                        actual: 1,
                    },
                }],
                ..Default::default()
            },
        );
    }
}

#[test]
fn test_color_slpv2_mint_overlapping_amount() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_mint(&TOKEN_ID2, [_0], 1),
                make_mint(&TOKEN_ID3, [_0, _1], 0),
            ]),
            2,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::MINT,
                genesis_info: None,
            }],
            outputs: vec![None, None, Some(TokenData::mint_baton(0))],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::OverlappingAmount {
                    prev_token: Token {
                        token_id: Cow::Owned(TOKEN_ID2),
                        token_type: TokenType::Standard,
                        variant: TokenVariant::MintBaton,
                    },
                    amount_idx: 1,
                    amount: _1,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_mint_overlapping_mint_baton() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_mint(&TOKEN_ID2, [_0, _0, _2], 0),
                make_mint(&TOKEN_ID3, [_0], 2),
            ]),
            3,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::MINT,
                genesis_info: None,
            }],
            outputs: vec![None, None, None, Some(TokenData::amount(0, _2))],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::OverlappingMintBaton {
                    prev_token: Token {
                        token_id: Cow::Owned(TOKEN_ID2),
                        token_type: TokenType::Standard,
                        variant: TokenVariant::Amount(_2),
                    },
                    baton_idx: 1,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_send_too_few_outputs() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![make_send(&TOKEN_ID2, [_0, _0, _0])]),
            2,
        )),
        ColoredTx {
            outputs: vec![None, None, None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 0,
                error: ColorSectionError::TooFewOutputs {
                    expected: 4,
                    actual: 3,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_send_duplicate_token_id() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_send(&TOKEN_ID2, []),
                make_send(&TOKEN_ID2, []),
            ]),
            0,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            outputs: vec![None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::DuplicateTokenId {
                    prev_section_idx: 0,
                    token_id: TOKEN_ID2,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_send_overlapping_amount() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_send(&TOKEN_ID2, [_0, _2, _0]),
                make_send(&TOKEN_ID3, [_3, _7, MAX]),
            ]),
            3,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            outputs: vec![None, None, Some(TokenData::amount(0, _2)), None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::OverlappingAmount {
                    prev_token: Token {
                        token_id: Cow::Owned(TOKEN_ID2),
                        token_type: TokenType::Standard,
                        variant: TokenVariant::Amount(_2),
                    },
                    amount_idx: 1,
                    amount: _7,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_send_success_simple() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![make_send(&TOKEN_ID2, [_0, _2, _0, MAX]),]),
            4,
        )),
        ColoredTx {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            outputs: vec![
                None,
                None,
                Some(TokenData::amount(0, _2)),
                None,
                Some(TokenData::amount(0, MAX)),
            ],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_send_success_complex() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_send(&TOKEN_ID2, [_0, _2, _0, _0, _3]),
                make_send(&TOKEN_ID3, [_0, _0, MAX]),
                make_send(&TOKEN_ID4, [_1, _0, _0, _0, _0, _7]),
            ]),
            7,
        )),
        ColoredTx {
            sections: vec![
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::SEND,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    section_type: SectionType::SEND,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID4),
                    section_type: SectionType::SEND,
                    genesis_info: None,
                },
            ],
            outputs: vec![
                None,
                Some(TokenData::amount(2, _1)),
                Some(TokenData::amount(0, _2)),
                Some(TokenData::amount(1, MAX)),
                None,
                Some(TokenData::amount(0, _3)),
                Some(TokenData::amount(2, _7)),
                None,
            ],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_burn_duplicate() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_burn(&TOKEN_ID2, _3),
                make_burn(&TOKEN_ID2, _2),
            ]),
            0,
        )),
        ColoredTx {
            intentional_burns: vec![IntentionalBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                amount: _3,
            }],
            outputs: vec![None],
            errors: vec![ColorError::SectionError {
                pushdata_idx: 1,
                error: ColorSectionError::DuplicateIntentionalBurnTokenId {
                    prev_burn_idx: 0,
                    burn_idx: 1,
                    token_id: TOKEN_ID2,
                },
            }],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_burn_success_simple() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![make_burn(&TOKEN_ID2, _3)]),
            0,
        )),
        ColoredTx {
            intentional_burns: vec![IntentionalBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                amount: _3,
            }],
            outputs: vec![None],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_burn_success_complex() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                make_burn(&TOKEN_ID2, _1),
                make_burn(&TOKEN_ID3, MAX),
            ]),
            0,
        )),
        ColoredTx {
            intentional_burns: vec![
                IntentionalBurn {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    amount: _1,
                },
                IntentionalBurn {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    amount: MAX,
                },
            ],
            outputs: vec![None],
            ..Default::default()
        },
    );
}

#[test]
fn test_color_slpv2_all_the_things() {
    assert_eq!(
        ColoredTx::parse_tx(&make_tx(
            sections_opreturn(vec![
                // success GENESIS
                make_genesis([_0, _7, _0, _0, _1], 2),
                // fail GENESIS: must be first
                make_genesis([], 0),
                // fail MINT: Too few outputs
                make_mint(&TOKEN_ID2, [_0; 7], 99),
                // fail MINT: Overlapping amounts
                make_mint(&TOKEN_ID2, [_0, MAX], 0),
                // fail MINT: Overlapping batons
                make_mint(&TOKEN_ID2, [_0], 1),
                // success BURN: token ID 2
                make_burn(&TOKEN_ID2, _2),
                // success MINT: token ID 3
                make_mint(&TOKEN_ID3, [_3, _0], 1),
                // success MINT: token ID 2
                make_mint(&TOKEN_ID2, [_0, _0, _0, _2, _0, _0, _0], 1),
                // fail MINT: Duplicate token ID 2
                make_mint(&TOKEN_ID2, [], 0),
                // fail BURN: Duplicate burn token ID 2
                make_burn(&TOKEN_ID2, _7),
                // fail SEND: Too few outputs
                make_send(&TOKEN_ID4, [_0; 12]),
                // success SEND: token ID 4
                make_send(
                    &TOKEN_ID4,
                    [_0, _0, _0, _0, _0, _0, _0, _0, _0, MAX],
                ),
                // fail MINT: Duplicate token ID 4
                make_mint(&TOKEN_ID4, [], 0),
                // success UNKNOWN
                b"SLP2\x89".as_ref().into(),
                // fail: Descending token type
                make_burn(&TOKEN_ID3, _1),
                // success UNKNOWN
                b"SLP2\x9a".as_ref().into(),
            ]),
            10,
        )),
        ColoredTx {
            sections: vec![
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID),
                    section_type: SectionType::GENESIS,
                    genesis_info: Some(GenesisInfo::default()),
                },
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    section_type: SectionType::MINT,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::MINT,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID4),
                    section_type: SectionType::SEND,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta {
                        token_id: TokenId::from([0; 32]),
                        token_type: TokenType::Unknown(0x89),
                    },
                    section_type: SectionType::UNKNOWN,
                    genesis_info: None,
                },
                SectionData {
                    meta: TokenMeta {
                        token_id: TokenId::from([0; 32]),
                        token_type: TokenType::Unknown(0x9a),
                    },
                    section_type: SectionType::UNKNOWN,
                    genesis_info: None,
                },
            ],
            intentional_burns: vec![
                IntentionalBurn {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    amount: _2,
                },
            ],
            outputs: vec![
                None,
                // success MINT: token ID 3
                Some(TokenData::amount(1, _3)),
                // success GENESIS
                Some(TokenData::amount(0, _7)),
                // success MINT: token ID 3
                Some(TokenData::mint_baton(1)),
                // success MINT: token ID 2
                Some(TokenData::amount(2, _2)),
                // success GENESIS
                Some(TokenData::amount(0, _1)),
                // success GENESIS
                Some(TokenData::mint_baton(0)),
                // success GENESIS
                Some(TokenData::mint_baton(0)),
                // success MINT: token ID 2
                Some(TokenData::mint_baton(2)),
                // success UNKNOWN
                Some(TokenData::unknown(4)),
                // success SEND: token ID 4
                Some(TokenData::amount(3, MAX)),
            ],
            errors: vec![
                // fail GENESIS: must be first
                ColorError::SectionError {
                    pushdata_idx: 1,
                    error: ColorSectionError::GenesisMustBeFirst,
                },
                // fail MINT: Too few outputs
                ColorError::SectionError {
                    pushdata_idx: 2,
                    error: ColorSectionError::TooFewOutputs {
                        expected: 107,
                        actual: 11,
                    },
                },
                // fail MINT: Overlapping amounts
                ColorError::SectionError {
                    pushdata_idx: 3,
                    error: ColorSectionError::OverlappingAmount {
                        prev_token: Token {
                            token_id: Cow::Owned(TOKEN_ID),
                            token_type: TokenType::Standard,
                            variant: TokenVariant::Amount(_7),
                        },
                        amount_idx: 1,
                        amount: MAX,
                    },
                },
                // fail MINT: Overlapping batons
                ColorError::SectionError {
                    pushdata_idx: 4,
                    error: ColorSectionError::OverlappingMintBaton {
                        prev_token: Token {
                            token_id: Cow::Owned(TOKEN_ID),
                            token_type: TokenType::Standard,
                            variant: TokenVariant::Amount(_7),
                        },
                        baton_idx: 0,
                    },
                },
                // fail MINT: Duplicate token ID 2
                ColorError::SectionError {
                    pushdata_idx: 8,
                    error: ColorSectionError::DuplicateTokenId {
                        prev_section_idx: 2,
                        token_id: TOKEN_ID2,
                    },
                },
                // fail BURN: Duplicate burn token ID 2
                ColorError::SectionError {
                    pushdata_idx: 9,
                    error: ColorSectionError::DuplicateIntentionalBurnTokenId {
                        prev_burn_idx: 0,
                        burn_idx: 1,
                        token_id: TOKEN_ID2,
                    },
                },
                // fail SEND: Too few outputs
                ColorError::SectionError {
                    pushdata_idx: 10,
                    error: ColorSectionError::TooFewOutputs {
                        expected: 13,
                        actual: 11,
                    },
                },
                // fail MINT: Duplicate token ID 4
                ColorError::SectionError {
                    pushdata_idx: 12,
                    error: ColorSectionError::DuplicateTokenId {
                        prev_section_idx: 3,
                        token_id: TOKEN_ID4,
                    },
                },
                // fail: Descending token type
                ColorError::SectionError {
                    pushdata_idx: 14,
                    error: ColorSectionError::DescendingTokenType {
                        before: 137,
                        after: 0,
                    },
                },
            ],
        },
    );
}
