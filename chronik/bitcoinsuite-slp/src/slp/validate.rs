use thiserror::Error;

use crate::slp::{
    Amount, Burn, ParseData, Token, TokenId, TokenType, TxData, TxType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlpSpentOutput {
    pub token_id: TokenId,
    pub token_type: TokenType,
    pub token: Token,
    pub group_token_id: Option<Box<TokenId>>,
}

/// Errors forwhen parsing a SLPv2 tx.
#[derive(Debug, Error, PartialEq)]
pub enum ValidateError {
    #[error("Invalid SEND: Output amounts ({output_sum}) exceed input amounts ({input_sum})")]
    OutputSumExceedInputSum { output_sum: u128, input_sum: u128 },
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

pub fn validate(
    parse_data: &ParseData,
    spent_outputs: &[Option<SlpSpentOutput>],
) -> Result<TxData, ValidateError> {
    let mut input_tokens = Vec::with_capacity(spent_outputs.len());
    let mut slp_burns = Vec::with_capacity(spent_outputs.len());
    let mut group_token_id = None;
    match &parse_data.tx_type {
        TxType::Genesis(_) => {
            for spent_output in spent_outputs {
                input_tokens.push(Token::EMPTY);
                match spent_output {
                    Some(spent_output) => {
                        slp_burns.push(Some(Box::new(Burn {
                            token: spent_output.token,
                            token_id: spent_output.token_id.clone(),
                        })));
                    }
                    None => slp_burns.push(None),
                }
            }
            if parse_data.token_type == TokenType::Nft1Child {
                let spent_output = spent_outputs
                    .get(0)
                    .and_then(|x| x.as_ref())
                    .ok_or(ValidateError::HasNoNft1Group)?;
                if spent_output.token_type != TokenType::Nft1Group
                    || spent_output.token.amount == 0
                {
                    return Err(ValidateError::HasNoNft1Group);
                }
                input_tokens[0] = spent_output.token;
                slp_burns[0] = None;
                group_token_id = Some(Box::new(spent_output.token_id.clone()));
            }
        }
        TxType::Mint => {
            let mut has_mint_baton = false;
            for spent_output in spent_outputs {
                match spent_output {
                    Some(spent_output) => {
                        if parse_data.token_id == spent_output.token_id
                            && parse_data.token_type == spent_output.token_type
                            && spent_output.token.is_mint_baton
                        {
                            // Found mint baton
                            has_mint_baton = true;
                            slp_burns.push(None);
                            input_tokens.push(spent_output.token);
                        } else {
                            // Invalid SLP input, burn it
                            slp_burns.push(Some(Box::new(Burn {
                                token: spent_output.token,
                                token_id: spent_output.token_id.clone(),
                            })));
                            input_tokens.push(Token::EMPTY);
                        }
                    }
                    None => {
                        slp_burns.push(None);
                        input_tokens.push(Token::EMPTY);
                    }
                }
            }
            if !has_mint_baton {
                return Err(ValidateError::HasNoMintBaton);
            }
        }
        TxType::Send => {
            let output_sum = parse_data
                .output_tokens
                .iter()
                .map(|token| u128::from(token.amount))
                .sum::<u128>();
            let mut input_sum: u128 = 0;
            for spent_output in spent_outputs {
                match spent_output {
                    Some(spent_output) => {
                        if parse_data.token_id == spent_output.token_id
                            && parse_data.token_type == spent_output.token_type
                            && !spent_output.token.is_mint_baton
                        {
                            // Valid input which is not a mint_baton
                            input_tokens.push(spent_output.token);
                            input_sum += u128::from(spent_output.token.amount);
                            if group_token_id.is_none() {
                                group_token_id =
                                    spent_output.group_token_id.clone();
                            }
                            if input_sum > output_sum {
                                let total_burned = input_sum - output_sum;
                                let spent_amount =
                                    u128::from(spent_output.token.amount);
                                let burned_amount =
                                    if total_burned < spent_amount {
                                        total_burned
                                    } else {
                                        spent_amount
                                    };
                                slp_burns.push(Some(Box::new(Burn {
                                    token: Token {
                                        amount: burned_amount as u64,
                                        is_mint_baton: false,
                                    },
                                    token_id: spent_output.token_id,
                                })));
                            } else {
                                slp_burns.push(None);
                            }
                        } else {
                            // Invalid SLP input, burn it
                            slp_burns.push(Some(Box::new(Burn {
                                token: spent_output.token,
                                token_id: spent_output.token_id,
                            })));
                            input_tokens.push(Token::EMPTY);
                        }
                    }
                    None => {
                        slp_burns.push(None);
                        input_tokens.push(Token::EMPTY);
                    }
                }
            }
            if output_sum > input_sum {
                return Err(ValidateError::OutputSumExceedInputSum {
                    output_sum,
                    input_sum,
                });
            }
        }
        TxType::Unknown => {
            for spent_output in spent_outputs {
                input_tokens.push(Token::EMPTY);
                match spent_output {
                    Some(spent_output) => {
                        slp_burns.push(Some(Box::new(Burn {
                            token: spent_output.token,
                            token_id: spent_output.token_id,
                        })));
                    }
                    None => slp_burns.push(None),
                }
            }
        }
        &TxType::Burn(expected) => {
            let mut actual = 0;
            for burn in spent_outputs.iter() {
                slp_burns.push(None);
                let burn = match burn {
                    Some(burn) => burn,
                    None => continue,
                };
                if burn.token == Token::EMPTY {
                    continue;
                }
                if burn.token_id != parse_data.token_id {
                    return Err(ValidateError::WrongBurnTokenId);
                }
                if burn.token.is_mint_baton {
                    return Err(ValidateError::WrongBurnMintBaton);
                }
                actual += burn.token.amount;
                input_tokens.push(burn.token);
            }
            if expected != actual {
                return Err(ValidateError::WrongBurnInvalidAmount {
                    expected,
                    actual,
                });
            }
        }
    }
    Ok(TxData {
        input_tokens,
        output_tokens: parse_data.output_tokens.clone(),
        slp_burns,
        token_type: parse_data.token_type,
        tx_type: parse_data.tx_type.tx_type_variant(),
        token_id: parse_data.token_id,
        group_token_id,
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::slp::{
        validate, Burn, GenesisInfo, ParseData, SlpSpentOutput, Token, TokenId,
        TokenType, TxData, TxType, TxTypeVariant, ValidateError,
    };

    #[test]
    fn test_validate_genesis_failure() -> Result<(), ValidateError> {
        // Missing NFT1 Group token
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Nft1Child,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo::default())),
                    token_id: TokenId::from_be_bytes([4; 32]),
                },
                &[None],
            ),
            Err(ValidateError::HasNoNft1Group),
        );
        // Invalid NFT1 Group token amount and token type
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Nft1Child,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo::default())),
                    token_id: TokenId::from_be_bytes([4; 32]),
                },
                &[Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::EMPTY,
                    group_token_id: None,
                })],
            ),
            Err(ValidateError::HasNoNft1Group),
        );
        // Invalid NFT1 Group token amount
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Nft1Child,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo::default())),
                    token_id: TokenId::from_be_bytes([4; 32]),
                },
                &[Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Nft1Group,
                    token: Token::EMPTY,
                    group_token_id: None,
                })],
            ),
            Err(ValidateError::HasNoNft1Group),
        );
        // Invalid NFT1 Group token type
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Nft1Child,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo::default())),
                    token_id: TokenId::from_be_bytes([4; 32]),
                },
                &[Some(SlpSpentOutput {
                    token_id: TokenId::from_be_bytes([3; 32]),
                    token_type: TokenType::Fungible,
                    token: Token::amount(1),
                    group_token_id: None,
                })],
            ),
            Err(ValidateError::HasNoNft1Group),
        );
        // Invalid NFT1 Group token input index (must be at 0)
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Nft1Child,
                    tx_type: TxType::Genesis(Box::new(GenesisInfo::default())),
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
            Err(ValidateError::HasNoNft1Group),
        );
        Ok(())
    }

    #[test]
    fn test_validate_genesis_success() -> Result<(), ValidateError> {
        // Fungible token genesis
        assert_eq!(
            validate(
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
            validate(
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
            validate(
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
            validate(
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
    fn test_validate_mint_failure() -> Result<(), ValidateError> {
        // No SLP inputs
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![],
                    token_type: TokenType::Fungible,
                    tx_type: TxType::Mint,
                    token_id: TokenId::from_be_bytes([1; 32]),
                },
                &[None],
            ),
            Err(ValidateError::HasNoMintBaton),
        );
        // No MINT input
        assert_eq!(
            validate(
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
            Err(ValidateError::HasNoMintBaton),
        );
        // Wrong MINT input token ID
        assert_eq!(
            validate(
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
            Err(ValidateError::HasNoMintBaton),
        );
        // Big Fungible example with lots of wrong MINT batons
        assert_eq!(
            validate(
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
            Err(ValidateError::HasNoMintBaton),
        );
        // Big NFT1 Group example with lots of wrong batons
        assert_eq!(
            validate(
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
            Err(ValidateError::HasNoMintBaton),
        );
        Ok(())
    }

    #[test]
    fn test_validate_mint_success() -> Result<(), ValidateError> {
        // Fungible MINT
        assert_eq!(
            validate(
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
            validate(
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
            validate(
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
    fn test_validate_send_failure() -> Result<(), ValidateError> {
        // No input tokens
        assert_eq!(
            validate(
                &ParseData {
                    output_tokens: vec![Token::amount(4)],
                    token_type: TokenType::Fungible,
                    tx_type: TxType::Send,
                    token_id: TokenId::from_be_bytes([4; 32]),
                },
                &[None],
            ),
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
        );
        // Fungible inputs not enough (3 < 4)
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 3,
                output_sum: 4,
            }),
        );
        // Wrong input token type (expected Fungible, got NFT1 Child)
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
        );
        // NFT1 Group inputs not enough (3 < 4)
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 3,
                output_sum: 4,
            }),
        );
        // Wrong input token type (expected NFT1 Group, got NFT1 Child)
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
        );
        // Wrong input token ID
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0,
                output_sum: 4,
            }),
        );
        // Big Fungible with off-by-one too little input tokens
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0x1fffffffffffe0003,
                output_sum: 0x1fffffffffffe0004,
            }),
        );
        // Big NFT1 Group with off-by-one too little input tokens
        assert_eq!(
            validate(
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
            Err(ValidateError::OutputSumExceedInputSum {
                input_sum: 0x1fffffffffffe0003,
                output_sum: 0x1fffffffffffe0004,
            }),
        );
        Ok(())
    }

    #[test]
    fn test_validate_send_success() -> Result<(), ValidateError> {
        // Valid Fungible SEND with 0 inputs and outputs
        assert_eq!(
            validate(
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
            validate(
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
            validate(
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
            validate(
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
            validate(
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
            validate(
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
            validate(
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
                output_tokens: vec![
                    Token::EMPTY,
                    Token::amount(1),
                    Token::EMPTY
                ],
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
                group_token_id: Some(Box::new(TokenId::from_be_bytes(
                    [10; 32]
                ))),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_validate_burn_failure() -> Result<(), ValidateError> {
        // Invalid BURN: wrong token ID
        assert_eq!(
            validate(
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
            Err(ValidateError::WrongBurnTokenId),
        );
        // Invalid BURN: can't use to burn MINT baton
        assert_eq!(
            validate(
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
            Err(ValidateError::WrongBurnMintBaton),
        );
        // Invalid BURN: selling less tokens than claimed
        assert_eq!(
            validate(
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
            Err(ValidateError::WrongBurnInvalidAmount {
                expected: 10,
                actual: 9,
            }),
        );
        Ok(())
    }

    #[test]
    fn test_validate_burn_success() -> Result<(), ValidateError> {
        // Valid BURN: burning 10 tokens
        assert_eq!(
            validate(
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
}
