use bitcoinsuite_core::hash::Sha256d;
use bitcoinsuite_slp::slp::{
    verify, ParseData, SlpSpentOutput, Token, TokenBurn, TokenId, TokenMeta,
    TxData, TxType, TxTypeVariant, VerifyError,
};
use pretty_assertions::assert_eq;

const TOKEN_ID1: TokenId = TokenId::from_le_hash(Sha256d([1; 32]));
const TOKEN_ID2: TokenId = TokenId::from_le_hash(Sha256d([2; 32]));
const TOKEN_ID3: TokenId = TokenId::from_le_hash(Sha256d([3; 32]));
const TOKEN_ID4: TokenId = TokenId::from_le_hash(Sha256d([4; 32]));
const TOKEN_ID5: TokenId = TokenId::from_le_hash(Sha256d([5; 32]));

#[test]
fn test_verify_genesis_failure() -> Result<(), VerifyError> {
    // Missing NFT1 Group token
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: Some(VerifyError::HasNoNft1Group),
            genesis_info: None,
        },
    );
    // Invalid NFT1 Group token amount and token type
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID3),
                token: Token::Amount(0),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: Some(VerifyError::HasNoNft1Group),
            genesis_info: None,
        },
    );
    // Invalid NFT1 Group token amount
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                token: Token::Amount(0),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: Some(VerifyError::HasNoNft1Group),
            genesis_info: None,
        },
    );
    // Invalid NFT1 Group token type
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID4),
                token: Token::Amount(1),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID4),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::HasNoNft1Group),
            genesis_info: None,
        },
    );
    // Invalid NFT1 Group token input index (must be at 0)
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[
                None,
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(1),
                    group_token_id: None,
                })
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::HasNoNft1Group),
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_genesis_success() -> Result<(), VerifyError> {
    // Fungible token genesis
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: Some(Default::default()),
        },
    );
    // Fungible genesis burning another token
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID2),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::Amount(1),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID2),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID1),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: None,
            genesis_info: Some(Default::default()),
        },
    );
    // NFT1 Child genesis consuming NFT1 Group
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::nft1_group(TOKEN_ID3),
                token: Token::Amount(4),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: Some(TOKEN_ID3),
            burns: vec![],
            error: None,
            genesis_info: Some(Default::default()),
        },
    );
    // NFT1 Child genesis consuming one NFT1 Group and burning another
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Genesis(Default::default()),
                output_tokens: vec![],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID3),
                    token: Token::Amount(4),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID2),
                    token: Token::Amount(1),
                    group_token_id: None,
                }),
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Genesis,
            output_tokens: vec![],
            group_token_id: Some(TOKEN_ID3),
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_group(TOKEN_ID2),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: None,
            genesis_info: Some(Default::default()),
        },
    );
    Ok(())
}

#[test]
fn test_verify_mint_failure() -> Result<(), VerifyError> {
    // No SLP inputs
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: Some(VerifyError::HasNoMintBaton),
            genesis_info: None,
        },
    );
    // No MINT input
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::Amount(4),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID1),
                amount: 4,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::HasNoMintBaton),
            genesis_info: None,
        },
    );
    // Wrong MINT input token ID
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID2),
                token: Token::MintBaton,
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID2),
                amount: 0,
                burn_mint_batons: true,
            }],
            error: Some(VerifyError::HasNoMintBaton),
            genesis_info: None,
        },
    );
    // Big Fungible example with lots of wrong MINT batons
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::Amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Group)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
            ],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    amount: 4,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
            ],
            error: Some(VerifyError::HasNoMintBaton),
            genesis_info: None,
        },
    );
    // Big NFT1 Group example with lots of wrong batons
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    token: Token::Amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID2),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (Fungible)
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    amount: 4,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID2),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
            ],
            error: Some(VerifyError::HasNoMintBaton),
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_mint_success() -> Result<(), VerifyError> {
    // Fungible MINT
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::MintBaton,
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: None,
        },
    );
    // Fungible MINT with lots of wrong batons and one correct one
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::Amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Group)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Correct MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
            ],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    amount: 4,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
            ],
            error: None,
            genesis_info: None,
        },
    );
    // NFT Group MINT with lots of invalid batons and one correct one
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID1),
                tx_type: TxType::Mint,
                output_tokens: vec![],
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::Amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Correct MINT baton
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (Fungible)
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    token: Token::MintBaton,
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID1),
            tx_type: TxTypeVariant::Mint,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID1),
                    amount: 4,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID2),
                    amount: 0,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID1),
                    amount: 0,
                    burn_mint_batons: true,
                },
            ],
            error: None,
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_send_failure() -> Result<(), VerifyError> {
    // No input tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // Fungible inputs not enough (3 < 4)
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID4),
                token: Token::Amount(3),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID4),
                amount: 3,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 3,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // Wrong input token type (expected Fungible, got NFT1 Child)
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                token: Token::Amount(1),
                group_token_id: Some(TokenId::from_be_bytes([10; 32])),
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // NFT1 Group inputs not enough (3 < 4)
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                token: Token::Amount(3),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                amount: 3,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 3,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // Wrong input token type (expected NFT1 Group, got NFT1 Child)
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                token: Token::Amount(1),
                group_token_id: Some(TokenId::from_be_bytes([10; 32])),
            })],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                amount: 1,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // Wrong input token ID
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(4))],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID3),
                token: Token::Amount(5),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID3),
                amount: 5,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
            genesis_info: None,
        },
    );
    // Big Fungible with off-by-one too little input tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![
                    Some(Token::Amount(1)),
                    Some(Token::Amount(0xffff_ffff_ffff_0000)),
                    Some(Token::Amount(0xffff_ffff_ffff_0001)),
                    Some(Token::Amount(2)),
                ],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::MintBaton,
                    group_token_id: None,
                }),
                None,
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(1),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0003),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID3),
                    token: Token::Amount(100),
                    group_token_id: None,
                })
            ],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None; 4],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    amount: 0x1_ffff_ffff_fffe_0003,
                    burn_mint_batons: true,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    amount: 1,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID3),
                    amount: 100,
                    burn_mint_batons: false,
                },
            ],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0x1fffffffffffe0003,
                output_sum: 0x1fffffffffffe0004,
            }),
            genesis_info: None,
        },
    );
    // Big NFT1 Group with off-by-one too little input tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![
                    Some(Token::Amount(1)),
                    Some(Token::Amount(0xffff_ffff_ffff_0000)),
                    Some(Token::Amount(0xffff_ffff_ffff_0001)),
                    Some(Token::Amount(2)),
                ],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                None,
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(1),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                None,
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0003),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID3),
                    token: Token::Amount(100),
                    group_token_id: None,
                })
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None; 4],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    amount: 0x1_ffff_ffff_fffe_0003,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    amount: 1,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID3),
                    amount: 100,
                    burn_mint_batons: false,
                },
            ],
            error: Some(VerifyError::OutputSumExceedInputSum {
                input_sum: 0x1fffffffffffe0003,
                output_sum: 0x1fffffffffffe0004,
            }),
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_send_success() -> Result<(), VerifyError> {
    // Valid Fungible SEND with 0 inputs and outputs
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![None],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: None,
        },
    );
    // Valid NFT1 Group SEND with 0 inputs and outputs
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![None],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: None,
        },
    );
    // Valid NFT1 Child SEND with 0 inputs and outputs
    // This leaves group_token_id at None
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![None],
            },
            &[None],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: None,
        },
    );
    // Fungible SEND sending 10 tokens and burning a MINT baton
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![Some(Token::Amount(10))],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::MintBaton,
                    group_token_id: None,
                })
            ],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![Some(Token::Amount(10))],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID4),
                amount: 0,
                burn_mint_batons: true,
            }],
            error: None,
            genesis_info: None,
        },
    );
    // Big Fungible SEND with 64-bit overflow and partially burning tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![
                    Some(Token::Amount(0xffff_ffff_ffff_0000)),
                    Some(Token::Amount(0xffff_ffff_ffff_0002)),
                    Some(Token::Amount(1)),
                ],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(0x2fff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    token: Token::Amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID5),
                    token: Token::Amount(10),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
            ],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![
                Some(Token::Amount(0xffff_ffff_ffff_0000)),
                Some(Token::Amount(0xffff_ffff_ffff_0002)),
                Some(Token::Amount(1)),
            ],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID5),
                    amount: 10,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::fungible(TOKEN_ID4),
                    amount: 0x1fff_ffff_fffe_fffd + 10,
                    burn_mint_batons: false,
                },
            ],
            error: None,
            genesis_info: None,
        },
    );
    // Big NFT1 Group SEND with 64-bit overflow and partially burning tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![
                    Some(Token::Amount(0xffff_ffff_ffff_0000)),
                    Some(Token::Amount(0xffff_ffff_ffff_0002)),
                    Some(Token::Amount(1)),
                ],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0x2fff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID5),
                    token: Token::Amount(10),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_group(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![
                Some(Token::Amount(0xffff_ffff_ffff_0000)),
                Some(Token::Amount(0xffff_ffff_ffff_0002)),
                Some(Token::Amount(1)),
            ],
            group_token_id: None,
            burns: vec![
                TokenBurn {
                    meta: TokenMeta::nft1_child(TOKEN_ID5),
                    amount: 10,
                    burn_mint_batons: false,
                },
                TokenBurn {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    amount: 0x1fff_ffff_fffe_fffd + 10,
                    burn_mint_batons: false,
                },
            ],
            error: None,
            genesis_info: None,
        },
    );
    // Big NFT1 Child SEND with two 0 value NFT1 Child inputs
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::nft1_child(TOKEN_ID4),
                tx_type: TxType::Send,
                output_tokens: vec![None, Some(Token::Amount(1)), None,],
            },
            &[
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(0),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_group(TOKEN_ID4),
                    token: Token::Amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(1),
                    group_token_id: Some(TokenId::from_be_bytes([10; 32])),
                }),
                Some(SlpSpentOutput {
                    meta: TokenMeta::nft1_child(TOKEN_ID4),
                    token: Token::Amount(0),
                    group_token_id: None,
                }),
            ],
        ),
        TxData {
            meta: TokenMeta::nft1_child(TOKEN_ID4),
            tx_type: TxTypeVariant::Send,
            output_tokens: vec![None, Some(Token::Amount(1)), None],
            group_token_id: Some(TokenId::from_be_bytes([10; 32])),
            burns: vec![TokenBurn {
                meta: TokenMeta::nft1_group(TOKEN_ID4),
                amount: 0xefff_ffff_ffff_0000,
                burn_mint_batons: false,
            }],
            error: None,
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_burn_failure() -> Result<(), VerifyError> {
    // Invalid BURN: wrong token ID
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Burn(10),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID2),
                token: Token::Amount(10),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Burn,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID2),
                amount: 10,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::WrongBurnTokenId),
            genesis_info: None,
        },
    );
    // Invalid BURN: can't use to burn MINT baton
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Burn(10),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::MintBaton,
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Burn,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID1),
                amount: 0,
                burn_mint_batons: true,
            }],
            error: Some(VerifyError::WrongBurnMintBaton),
            genesis_info: None,
        },
    );
    // Invalid BURN: burning less tokens than claimed
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Burn(10),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::Amount(9),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Burn,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![TokenBurn {
                meta: TokenMeta::fungible(TOKEN_ID1),
                amount: 9,
                burn_mint_batons: false,
            }],
            error: Some(VerifyError::WrongBurnInvalidAmount {
                expected: 10,
                actual: 9,
            }),
            genesis_info: None,
        },
    );
    Ok(())
}

#[test]
fn test_verify_burn_success() -> Result<(), VerifyError> {
    // Valid BURN: burning 10 tokens
    assert_eq!(
        verify(
            &ParseData {
                meta: TokenMeta::fungible(TOKEN_ID1),
                tx_type: TxType::Burn(10),
                output_tokens: vec![],
            },
            &[Some(SlpSpentOutput {
                meta: TokenMeta::fungible(TOKEN_ID1),
                token: Token::Amount(10),
                group_token_id: None,
            })],
        ),
        TxData {
            meta: TokenMeta::fungible(TOKEN_ID1),
            tx_type: TxTypeVariant::Burn,
            output_tokens: vec![],
            group_token_id: None,
            burns: vec![],
            error: None,
            genesis_info: None,
        },
    );
    Ok(())
}
