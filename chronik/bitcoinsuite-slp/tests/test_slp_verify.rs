use pretty_assertions::assert_eq;

use bitcoinsuite_slp::slp::{
    verify, Burn, ParseData, SlpSpentOutput, Token, TokenId, TokenType, TxData,
    TxType, TxTypeVariant, VerifyError,
};

#[test]
fn test_verify_genesis_failure() -> Result<(), VerifyError> {
    // Missing NFT1 Group token
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[None],
        ),
        Err(VerifyError::HasNoNft1Group),
    );
    // Invalid NFT1 Group token amount and token type
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([3; 32]),
                token_type: TokenType::Fungible,
                token: Token::EMPTY,
                group_token_id: None,
            })],
        ),
        Err(VerifyError::HasNoNft1Group),
    );
    // Invalid NFT1 Group token amount
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([3; 32]),
                token_type: TokenType::Nft1Group,
                token: Token::EMPTY,
                group_token_id: None,
            })],
        ),
        Err(VerifyError::HasNoNft1Group),
    );
    // Invalid NFT1 Group token type
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([3; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(1),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::HasNoNft1Group),
    );
    // Invalid NFT1 Group token input index (must be at 0)
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                None,
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(1),
                    group_token_id: None,
                })
            ],
        ),
        Err(VerifyError::HasNoNft1Group),
    );
    Ok(())
}

#[test]
fn test_verify_genesis_success() -> Result<(), VerifyError> {
    // Fungible token genesis
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[None],
        ),
        Ok(TxData {
            input_tokens: vec![Token::EMPTY],
            output_tokens: vec![],
            slp_burns: vec![None],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Genesis,
            token_id: TokenId::from_be_bytes([1; 32]),
            group_token_id: None,
        }),
    );
    // Fungible genesis burning another token
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([2; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(1),
                group_token_id: None,
            })],
        ),
        Ok(TxData {
            input_tokens: vec![Token::EMPTY],
            output_tokens: vec![],
            slp_burns: vec![Some(Box::new(Burn {
                token: Token::amount(1),
                token_id: TokenId::from_be_bytes([1; 32]),
            }))],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Genesis,
            token_id: TokenId::from_be_bytes([2; 32]),
            group_token_id: None,
        }),
    );
    // NFT1 Child genesis consuming NFT1 Group
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([3; 32]),
                token_type: TokenType::Nft1Group,
                token: Token::amount(4),
                group_token_id: None,
            })],
        ),
        Ok(TxData {
            input_tokens: vec![Token::amount(4)],
            output_tokens: vec![],
            slp_burns: vec![None],
            token_type: TokenType::Nft1Child,
            tx_type: TxTypeVariant::Genesis,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: Some(Box::new(TokenId::from_be_bytes([3; 32]))),
        }),
    );
    // NFT1 Child genesis consuming one NFT1 Group and burning another
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Genesis(Default::default()),
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(4),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([2; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(1),
                    group_token_id: None,
                }),
            ],
        ),
        Ok(TxData {
            input_tokens: vec![Token::amount(4), Token::EMPTY],
            output_tokens: vec![],
            slp_burns: vec![
                None,
                Some(Box::new(Burn {
                    token: Token::amount(1),
                    token_id: TokenId::from_be_bytes([2; 32]),
                })),
            ],
            token_type: TokenType::Nft1Child,
            tx_type: TxTypeVariant::Genesis,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: Some(Box::new(TokenId::from_be_bytes([3; 32]))),
        }),
    );
    Ok(())
}

#[test]
fn test_verify_mint_failure() -> Result<(), VerifyError> {
    // No SLP inputs
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[None],
        ),
        Err(VerifyError::HasNoMintBaton),
    );
    // No MINT input
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(4),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::HasNoMintBaton),
    );
    // Wrong MINT input token ID
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([2; 32]),
                token_type: TokenType::Fungible,
                token: Token::MINT_BATON,
                group_token_id: None,
            })],
        ),
        Err(VerifyError::HasNoMintBaton),
    );
    // Big Fungible example with lots of wrong MINT batons
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([2; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Group)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::MINT_BATON,
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
            ],
        ),
        Err(VerifyError::HasNoMintBaton),
    );
    // Big NFT1 Group example with lots of wrong batons
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([2; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (Fungible)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::MINT_BATON,
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
            ],
        ),
        Err(VerifyError::HasNoMintBaton),
    );
    Ok(())
}

#[test]
fn test_verify_mint_success() -> Result<(), VerifyError> {
    // Fungible MINT
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::MINT_BATON,
                group_token_id: None,
            })],
        ),
        Ok(TxData {
            input_tokens: vec![Token::MINT_BATON],
            output_tokens: vec![],
            slp_burns: vec![None],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Mint,
            token_id: TokenId::from_be_bytes([1; 32]),
            group_token_id: None,
        }),
    );
    // Fungible MINT with lots of wrong batons and one correct one
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([2; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Group)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Correct MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::MINT_BATON,
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
            ],
        ),
        Ok(TxData {
            input_tokens: vec![
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
                Token::MINT_BATON,
                Token::EMPTY,
                Token::EMPTY,
            ],
            output_tokens: vec![],
            slp_burns: vec![
                None,
                Some(Box::new(Burn {
                    token: Token::amount(4),
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                None,
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([2; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                None, // Correct MINT baton not burned
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                None,
            ],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Mint,
            token_id: TokenId::from_be_bytes([1; 32]),
            group_token_id: None,
        }),
    );
    // NFT Group MINT with lots of invalid batons and one correct one
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Mint,
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[
                None,
                // Not a MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(4),
                    group_token_id: None,
                }),
                None,
                // Wrong token ID
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([2; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Correct MINT baton
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (Fungible)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                // Wrong token type (NFT1 Child)
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([1; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::MINT_BATON,
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
            ],
        ),
        Ok(TxData {
            input_tokens: vec![
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
                Token::MINT_BATON,
                Token::EMPTY,
                Token::EMPTY,
                Token::EMPTY,
            ],
            output_tokens: vec![],
            slp_burns: vec![
                None,
                Some(Box::new(Burn {
                    token: Token::amount(4),
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                None,
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([2; 32]),
                })),
                None, // Correct MINT baton not burned
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([1; 32]),
                })),
                None,
            ],
            token_type: TokenType::Nft1Group,
            tx_type: TxTypeVariant::Mint,
            token_id: TokenId::from_be_bytes([1; 32]),
            group_token_id: None,
        }),
    );
    Ok(())
}

#[test]
fn test_verify_send_failure() -> Result<(), VerifyError> {
    // No input tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[None],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0,
            output_sum: 4,
        }),
    );
    // Fungible inputs not enough (3 < 4)
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([4; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(3),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 3,
            output_sum: 4,
        }),
    );
    // Wrong input token type (expected Fungible, got NFT1 Child)
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([4; 32]),
                token_type: TokenType::Nft1Child,
                token: Token::amount(1),
                group_token_id: Some(Box::new(TokenId::from_be_bytes(
                    [10; 32]
                ))),
            })],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0,
            output_sum: 4,
        }),
    );
    // NFT1 Group inputs not enough (3 < 4)
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([4; 32]),
                token_type: TokenType::Nft1Group,
                token: Token::amount(3),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 3,
            output_sum: 4,
        }),
    );
    // Wrong input token type (expected NFT1 Group, got NFT1 Child)
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([4; 32]),
                token_type: TokenType::Nft1Child,
                token: Token::amount(1),
                group_token_id: Some(Box::new(TokenId::from_be_bytes(
                    [10; 32]
                ))),
            })],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0,
            output_sum: 4,
        }),
    );
    // Wrong input token ID
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(4)],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([3; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(5),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0,
            output_sum: 4,
        }),
    );
    // Big Fungible with off-by-one too little input tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![
                    Token::amount(1),
                    Token::amount(0xffff_ffff_ffff_0000),
                    Token::amount(0xffff_ffff_ffff_0001),
                    Token::amount(2),
                ],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                }),
                None,
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(1),
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(0xffff_ffff_ffff_0003),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(100),
                    group_token_id: None,
                })
            ],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0x1fffffffffffe0003,
            output_sum: 0x1fffffffffffe0004,
        }),
    );
    // Big NFT1 Group with off-by-one too little input tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![
                    Token::amount(1),
                    Token::amount(0xffff_ffff_ffff_0000),
                    Token::amount(0xffff_ffff_ffff_0001),
                    Token::amount(2),
                ],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                None,
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(1),
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                None,
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0xffff_ffff_ffff_0003),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(100),
                    group_token_id: None,
                })
            ],
        ),
        Err(VerifyError::OutputSumExceedInputSum {
            input_sum: 0x1fffffffffffe0003,
            output_sum: 0x1fffffffffffe0004,
        }),
    );
    Ok(())
}

#[test]
fn test_verify_send_success() -> Result<(), VerifyError> {
    // Valid Fungible SEND with 0 inputs and outputs
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::EMPTY],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[None],
        ),
        Ok(TxData {
            input_tokens: vec![Token::EMPTY],
            output_tokens: vec![Token::EMPTY],
            slp_burns: vec![None],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Valid NFT1 Group SEND with 0 inputs and outputs
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::EMPTY],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[None],
        ),
        Ok(TxData {
            input_tokens: vec![Token::EMPTY],
            output_tokens: vec![Token::EMPTY],
            slp_burns: vec![None],
            token_type: TokenType::Nft1Group,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Valid NFT1 Child SEND with 0 inputs and outputs
    // This leaves group_token_id at None
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::EMPTY],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[None],
        ),
        Ok(TxData {
            input_tokens: vec![Token::EMPTY],
            output_tokens: vec![Token::EMPTY],
            slp_burns: vec![None],
            token_type: TokenType::Nft1Child,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Fungible SEND sending 10 tokens and burning a MINT baton
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![Token::amount(10)],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::MINT_BATON,
                    group_token_id: None,
                })
            ],
        ),
        Ok(TxData {
            input_tokens: vec![Token::amount(10), Token::EMPTY],
            output_tokens: vec![Token::amount(10)],
            slp_burns: vec![
                None,
                Some(Box::new(Burn {
                    token: Token::MINT_BATON,
                    token_id: TokenId::from_be_bytes([4; 32]),
                }))
            ],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Big Fungible SEND with 64-bit overflow and partially burning tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![
                    Token::amount(0xffff_ffff_ffff_0000),
                    Token::amount(0xffff_ffff_ffff_0002),
                    Token::amount(1),
                ],
                token_type: TokenType::Fungible,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(0x2fff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(10),
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
            ],
        ),
        Ok(TxData {
            input_tokens: vec![
                Token::amount(0xffff_ffff_ffff_0000),
                Token::amount(0xefff_ffff_ffff_0000),
                Token::amount(0x2fff_ffff_ffff_0000),
                Token::amount(10),
                Token::EMPTY,
            ],
            output_tokens: vec![
                Token::amount(0xffff_ffff_ffff_0000),
                Token::amount(0xffff_ffff_ffff_0002),
                Token::amount(1),
            ],
            slp_burns: vec![
                None,
                None,
                Some(Box::new(Burn {
                    token: Token::amount(0x1fff_ffff_fffe_fffd),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::amount(10),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::amount(10),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
            ],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Big NFT1 Group SEND with 64-bit overflow and partially burning tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![
                    Token::amount(0xffff_ffff_ffff_0000),
                    Token::amount(0xffff_ffff_ffff_0002),
                    Token::amount(1),
                ],
                token_type: TokenType::Nft1Group,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0xffff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0x2fff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(10),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(10),
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
            ],
        ),
        Ok(TxData {
            input_tokens: vec![
                Token::amount(0xffff_ffff_ffff_0000),
                Token::amount(0xefff_ffff_ffff_0000),
                Token::amount(0x2fff_ffff_ffff_0000),
                Token::amount(10),
                Token::EMPTY,
            ],
            output_tokens: vec![
                Token::amount(0xffff_ffff_ffff_0000),
                Token::amount(0xffff_ffff_ffff_0002),
                Token::amount(1),
            ],
            slp_burns: vec![
                None,
                None,
                Some(Box::new(Burn {
                    token: Token::amount(0x1fff_ffff_fffe_fffd),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::amount(10),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
                Some(Box::new(Burn {
                    token: Token::amount(10),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
            ],
            token_type: TokenType::Nft1Group,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: None,
        }),
    );
    // Big NFT1 Child SEND with two 0 value NFT1 Child inputs
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![
                    Token::EMPTY,
                    Token::amount(1),
                    Token::EMPTY
                ],
                token_type: TokenType::Nft1Child,
                tx_type: TxType::Send,
                token_id: TokenId::from_be_bytes([4; 32]),
            },
            &[
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::EMPTY,
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::amount(0xefff_ffff_ffff_0000),
                    group_token_id: None,
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::amount(1),
                    group_token_id: Some(Box::new(TokenId::from_be_bytes(
                        [10; 32]
                    ))),
                }),
                Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([4; 32]),
                    token_type: TokenType::Nft1Child,
                    token: Token::EMPTY,
                    group_token_id: None,
                }),
            ],
        ),
        Ok(TxData {
            input_tokens: vec![
                Token::EMPTY,
                Token::EMPTY,
                Token::amount(1),
                Token::EMPTY,
            ],
            output_tokens: vec![Token::EMPTY, Token::amount(1), Token::EMPTY],
            slp_burns: vec![
                None,
                Some(Box::new(Burn {
                    token: Token::amount(0xefff_ffff_ffff_0000),
                    token_id: TokenId::from_be_bytes([4; 32]),
                })),
                None,
                None,
            ],
            token_type: TokenType::Nft1Child,
            tx_type: TxTypeVariant::Send,
            token_id: TokenId::from_be_bytes([4; 32]),
            group_token_id: Some(Box::new(TokenId::from_be_bytes([10; 32]))),
        }),
    );
    Ok(())
}

#[test]
fn test_verify_burn_failure() -> Result<(), VerifyError> {
    // Invalid BURN: wrong token ID
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Burn(10),
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([2; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(10),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::WrongBurnTokenId),
    );
    // Invalid BURN: can't use to burn MINT baton
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Burn(10),
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::MINT_BATON,
                group_token_id: None,
            })],
        ),
        Err(VerifyError::WrongBurnMintBaton),
    );
    // Invalid BURN: selling less tokens than claimed
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Burn(10),
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(9),
                group_token_id: None,
            })],
        ),
        Err(VerifyError::WrongBurnInvalidAmount {
            expected: 10,
            actual: 9,
        }),
    );
    Ok(())
}

#[test]
fn test_verify_burn_success() -> Result<(), VerifyError> {
    // Valid BURN: burning 10 tokens
    assert_eq!(
        verify(
            &ParseData {
                output_tokens: vec![],
                token_type: TokenType::Fungible,
                tx_type: TxType::Burn(10),
                token_id: TokenId::from_be_bytes([1; 32]),
            },
            &[Some(SlpSpentOutput {
                token_id: TokenId::from_be_bytes([1; 32]),
                token_type: TokenType::Fungible,
                token: Token::amount(10),
                group_token_id: None,
            })],
        ),
        Ok(TxData {
            input_tokens: vec![Token::amount(10)],
            output_tokens: vec![],
            slp_burns: vec![None],
            token_type: TokenType::Fungible,
            tx_type: TxTypeVariant::Burn,
            token_id: TokenId::from_be_bytes([1; 32]),
            group_token_id: None,
        }),
    );
    Ok(())
}
