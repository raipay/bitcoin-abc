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
pub enum VerifyError {
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

pub fn verify(
    parse_data: &ParseData,
    spent_outputs: &[Option<SlpSpentOutput>],
) -> Result<TxData, VerifyError> {
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
                    .ok_or(VerifyError::HasNoNft1Group)?;
                if spent_output.token_type != TokenType::Nft1Group
                    || spent_output.token.amount == 0
                {
                    return Err(VerifyError::HasNoNft1Group);
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
                return Err(VerifyError::HasNoMintBaton);
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
                return Err(VerifyError::OutputSumExceedInputSum {
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
                    return Err(VerifyError::WrongBurnTokenId);
                }
                if burn.token.is_mint_baton {
                    return Err(VerifyError::WrongBurnMintBaton);
                }
                actual += burn.token.amount;
                input_tokens.push(burn.token);
            }
            if expected != actual {
                return Err(VerifyError::WrongBurnInvalidAmount {
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
