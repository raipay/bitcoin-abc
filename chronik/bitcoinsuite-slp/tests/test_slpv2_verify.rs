use bitcoinsuite_core::tx::TxId;
use bitcoinsuite_slp::slpv2::{
    consts::MAX_TX_INPUTS, verify, Amount, BurnError, ColoredTx, GenesisInfo,
    Int, IntentionalBurn, SectionData, SectionType, Token, TokenBurn,
    TokenData, TokenId, TokenMeta, TokenType, TokenVariant, TxData,
};
use pretty_assertions::assert_eq;

const TOKEN_ID: TokenId = TokenId::new(TxId::new([4; 32]));
const TOKEN_ID2: TokenId = TokenId::new(TxId::new([5; 32]));
const TOKEN_ID3: TokenId = TokenId::new(TxId::new([6; 32]));
const TOKEN_ID4: TokenId = TokenId::new(TxId::new([7; 32]));

const _0: Amount = Amount::ZERO;
const _1: Amount = Amount::new(1);
const _2: Amount = Amount::new(2);
const _3: Amount = Amount::new(3);
const _7: Amount = Amount::new(7);
const MAX: Amount = Amount::new(0xffff_ffff_ffff);

#[test]
fn test_verify_slpv2_too_many_inputs() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TOKEN_ID2),
            section_type: SectionType::SEND,
            genesis_info: None,
        }],
        ..Default::default()
    };
    // using MAX_TX_INPUTS is fine
    assert_eq!(
        verify(colored_tx.clone(), &vec![None; MAX_TX_INPUTS]),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            burns: vec![],
            outputs: vec![],
            color_errors: vec![],
        },
    );
    // using MAX_TX_INPUTS + 1 burns all tokens
    assert_eq!(
        verify(colored_tx, &vec![None; MAX_TX_INPUTS + 1]),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: _0,
                burn_mint_batons: false,
                is_total: true,
                error: Some(BurnError::TooManyTxInputs(MAX_TX_INPUTS + 1)),
            }],
            outputs: vec![],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_no_overflow() {
    assert_eq!(
        verify(
            ColoredTx {
                sections: vec![SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::SEND,
                    genesis_info: None,
                }],
                outputs: vec![Some(TokenData {
                    section_idx: 0,
                    variant: TokenVariant::Amount(MAX),
                })],
                ..Default::default()
            },
            &vec![
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::Amount(MAX),
                });
                MAX_TX_INPUTS
            ],
        ),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: (MAX_TX_INPUTS as Int - 1) * MAX,
                burn_mint_batons: false,
                is_total: false,
                error: None,
            }],
            outputs: vec![Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(MAX),
            }),],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_missing_mint_baton() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TOKEN_ID2),
            section_type: SectionType::MINT,
            genesis_info: None,
        }],
        outputs: vec![
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(100)),
            }),
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(20)),
            }),
        ],
        ..Default::default()
    };
    assert_eq!(
        verify(colored_tx.clone(), &[None]),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: _0,
                burn_mint_batons: false,
                is_total: true,
                error: Some(BurnError::MissingMintBaton),
            }],
            outputs: vec![None, None],
            color_errors: vec![],
        },
    );
    assert_eq!(
        verify(
            colored_tx,
            &[Some(Token {
                meta: TokenMeta::standard(TOKEN_ID3),
                variant: TokenVariant::MintBaton,
            })],
        ),
        TxData {
            sections: vec![],
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    intentional_burn: None,
                    actual_burn: _0,
                    burn_mint_batons: false,
                    is_total: true,
                    error: Some(BurnError::MissingMintBaton),
                },
                TokenBurn {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    intentional_burn: None,
                    actual_burn: _0,
                    burn_mint_batons: true,
                    is_total: false,
                    error: None,
                },
            ],
            outputs: vec![None, None],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_mint_burn_tokens() {
    assert_eq!(
        verify(
            ColoredTx {
                sections: vec![SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::MINT,
                    genesis_info: None,
                }],
                outputs: vec![Some(TokenData {
                    section_idx: 0,
                    variant: TokenVariant::Amount(Amount::new(100)),
                })],
                ..Default::default()
            },
            &[
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::MintBaton,
                }),
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::Amount(Amount::new(600)),
                }),
            ],
        ),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::MINT,
                genesis_info: None,
            }],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: Amount::new(600),
                burn_mint_batons: false,
                is_total: false,
                error: None,
            }],
            outputs: vec![Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(100)),
            })],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_mint_success() {
    assert_eq!(
        verify(
            ColoredTx {
                sections: vec![SectionData {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    section_type: SectionType::MINT,
                    genesis_info: None,
                }],
                outputs: vec![Some(TokenData {
                    section_idx: 0,
                    variant: TokenVariant::Amount(Amount::new(123)),
                })],
                ..Default::default()
            },
            &[Some(Token {
                meta: TokenMeta::standard(TOKEN_ID2),
                variant: TokenVariant::MintBaton,
            })],
        ),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::MINT,
                genesis_info: None,
            }],
            burns: vec![],
            outputs: vec![Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(123)),
            })],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_send_insufficient_inputs() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TOKEN_ID2),
            section_type: SectionType::SEND,
            genesis_info: None,
        }],
        outputs: vec![
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(80)),
            }),
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(20)),
            }),
        ],
        ..Default::default()
    };
    assert_eq!(
        verify(colored_tx.clone(), &[None]),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: _0,
                burn_mint_batons: false,
                is_total: true,
                error: Some(BurnError::InsufficientInputSum(Amount::new(100))),
            }],
            outputs: vec![None, None],
            color_errors: vec![],
        },
    );
    assert_eq!(
        verify(
            colored_tx,
            &[Some(Token {
                meta: TokenMeta::standard(TOKEN_ID2),
                variant: TokenVariant::Amount(Amount::new(99)),
            })],
        ),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: Amount::new(99),
                burn_mint_batons: false,
                is_total: true,
                error: Some(BurnError::InsufficientInputSum(Amount::new(100))),
            }],
            outputs: vec![None, None],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_send_partial_burn() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TOKEN_ID2),
            section_type: SectionType::SEND,
            genesis_info: None,
        }],
        outputs: vec![
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(80)),
            }),
            Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(20)),
            }),
        ],
        ..Default::default()
    };
    assert_eq!(
        verify(
            colored_tx,
            &[
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::Amount(Amount::new(70)),
                }),
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::Amount(Amount::new(50)),
                }),
            ],
        ),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: Amount::new(20),
                burn_mint_batons: false,
                is_total: false,
                error: None,
            }],
            outputs: vec![
                Some(TokenData {
                    section_idx: 0,
                    variant: TokenVariant::Amount(Amount::new(80)),
                }),
                Some(TokenData {
                    section_idx: 0,
                    variant: TokenVariant::Amount(Amount::new(20)),
                })
            ],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_foreign_token_burn() {
    assert_eq!(
        verify(
            ColoredTx::default(),
            &[Some(Token {
                meta: TokenMeta::standard(TOKEN_ID2),
                variant: TokenVariant::Amount(Amount::new(100)),
            })],
        ),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID2),
                intentional_burn: None,
                actual_burn: Amount::new(100),
                burn_mint_batons: false,
                is_total: false,
                error: None,
            }],
            outputs: vec![],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_send_success() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TOKEN_ID2),
            section_type: SectionType::SEND,
            genesis_info: None,
        }],
        outputs: vec![Some(TokenData {
            section_idx: 0,
            variant: TokenVariant::Amount(Amount::new(100)),
        })],
        ..Default::default()
    };
    assert_eq!(
        verify(
            colored_tx,
            &[Some(Token {
                meta: TokenMeta::standard(TOKEN_ID2),
                variant: TokenVariant::Amount(Amount::new(100)),
            })],
        ),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TOKEN_ID2),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            burns: vec![],
            outputs: vec![Some(TokenData {
                section_idx: 0,
                variant: TokenVariant::Amount(Amount::new(100)),
            })],
            color_errors: vec![],
        },
    );
}

#[test]
fn test_color_slpv2_all_the_things() {
    let colored_tx = ColoredTx {
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
        ],
        intentional_burns: vec![IntentionalBurn {
            meta: TokenMeta::standard(TOKEN_ID4),
            amount: _2,
        }],
        outputs: vec![
            None,
            Some(TokenData::amount(1, _3)),
            Some(TokenData::amount(0, _7)),
            Some(TokenData::mint_baton(1)),
            Some(TokenData::amount(2, _2)),
            Some(TokenData::amount(0, _1)),
            Some(TokenData::mint_baton(0)),
            Some(TokenData::mint_baton(0)),
            Some(TokenData::mint_baton(2)),
            Some(TokenData::unknown(4, 0x89)),
            Some(TokenData::amount(3, MAX)),
        ],
        errors: vec![],
    };
    assert_eq!(
        verify(colored_tx.clone(), &[None]),
        TxData {
            sections: vec![
                SectionData {
                    meta: TokenMeta::standard(TOKEN_ID),
                    section_type: SectionType::GENESIS,
                    genesis_info: Some(GenesisInfo::default()),
                },
                SectionData {
                    meta: TokenMeta {
                        token_id: TokenId::from([0; 32]),
                        token_type: TokenType::Unknown(0x89),
                    },
                    section_type: SectionType::UNKNOWN,
                    genesis_info: None,
                }
            ],
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    intentional_burn: None,
                    actual_burn: _0,
                    burn_mint_batons: false,
                    is_total: true,
                    error: Some(BurnError::MissingMintBaton),
                },
                TokenBurn {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    intentional_burn: None,
                    actual_burn: _0,
                    burn_mint_batons: false,
                    is_total: true,
                    error: Some(BurnError::MissingMintBaton),
                },
                TokenBurn {
                    meta: TokenMeta::standard(TOKEN_ID4),
                    intentional_burn: Some(_2),
                    actual_burn: _0,
                    burn_mint_batons: false,
                    is_total: true,
                    error: Some(BurnError::InsufficientInputSum(MAX)),
                },
            ],
            outputs: vec![
                None,
                None,
                Some(TokenData::amount(0, _7)),
                None,
                None,
                Some(TokenData::amount(0, _1)),
                Some(TokenData::mint_baton(0)),
                Some(TokenData::mint_baton(0)),
                None,
                Some(TokenData::unknown(1, 0x89)),
                None,
            ],
            color_errors: vec![],
        },
    );
    assert_eq!(
        verify(
            colored_tx,
            &[
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID3),
                    variant: TokenVariant::MintBaton,
                }),
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID2),
                    variant: TokenVariant::MintBaton,
                }),
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID4),
                    variant: TokenVariant::Amount(MAX - _2),
                }),
                Some(Token {
                    meta: TokenMeta::standard(TOKEN_ID4),
                    variant: TokenVariant::Amount(_7),
                }),
            ],
        ),
        TxData {
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
            ],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TOKEN_ID4),
                intentional_burn: Some(_2),
                actual_burn: Amount::new(5),
                burn_mint_batons: false,
                is_total: false,
                error: None,
            }],
            outputs: vec![
                None,
                Some(TokenData::amount(1, _3)),
                Some(TokenData::amount(0, _7)),
                Some(TokenData::mint_baton(1)),
                Some(TokenData::amount(2, _2)),
                Some(TokenData::amount(0, _1)),
                Some(TokenData::mint_baton(0)),
                Some(TokenData::mint_baton(0)),
                Some(TokenData::mint_baton(2)),
                Some(TokenData::unknown(4, 0x89)),
                Some(TokenData::amount(3, MAX)),
            ],
            color_errors: vec![],
        },
    );
}
