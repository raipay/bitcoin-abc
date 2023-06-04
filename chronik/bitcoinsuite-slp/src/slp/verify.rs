use thiserror::Error;

use crate::slp::{
    Amount, ParseData, Token, TokenBurn, TokenId, TokenMeta, TokenType, TxData,
    TxType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlpSpentOutput {
    pub meta: TokenMeta,
    pub token: Token,
    pub group_token_id: Option<TokenId>,
}

/// Errors forwhen parsing a SLPv2 tx.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum VerifyError {
    #[error(
        "Invalid SEND: Output amounts ({output_sum}) exceed input amounts \
         ({input_sum})"
    )]
    OutputSumExceedInputSum { output_sum: u128, input_sum: u128 },
    #[error("Invalid NFT1 Child GENESIS: No group token")]
    HasNoNft1Group,
    #[error("Invalid MINT: No baton")]
    HasNoMintBaton,
    #[error("Invalid BURN: Burning the wrong token_id")]
    WrongBurnTokenId,
    #[error("Invalid BURN: Burning the wrong token_type")]
    WrongBurnTokenType,
    #[error("Invalid BURN: Burning MINT baton")]
    WrongBurnMintBaton,
    #[error(
        "Invalid BURN: Burning invalid amount, expected {expected} but got \
         {actual} base tokens"
    )]
    WrongBurnInvalidAmount { expected: Amount, actual: u128 },
    #[error("Found orphan txs")]
    FoundOrphanTx,
}

use self::VerifyError::*;

fn add_burn_token(
    burns: &mut Vec<TokenBurn>,
    meta: TokenMeta,
    amount: u128,
    is_mint_baton: bool,
) {
    if amount == 0 && !is_mint_baton {
        return;
    }
    let burn = burns.iter_mut().find(|burn| burn.meta == meta);
    match burn {
        Some(burn) => {
            burn.amount += amount;
            burn.burn_mint_batons = burn.burn_mint_batons || is_mint_baton;
        }
        None => burns.push(TokenBurn {
            meta,
            amount,
            burn_mint_batons: is_mint_baton,
        }),
    }
}

pub fn verify(
    parse_data: &ParseData,
    spent_outputs: &[Option<SlpSpentOutput>],
) -> TxData {
    let make_total_burn =
        |error: VerifyError, group_token_id: Option<TokenId>| {
            let mut burns = Vec::new();
            for output in spent_outputs.iter().flatten() {
                let amount = output.token.amount();
                add_burn_token(
                    &mut burns,
                    output.meta,
                    amount.into(),
                    output.token == Token::MintBaton,
                );
            }
            TxData {
                output_tokens: parse_data
                    .output_tokens
                    .iter()
                    .map(|_| None)
                    .collect(),
                tx_type: parse_data.tx_type.tx_type_variant(),
                meta: parse_data.meta,
                group_token_id,
                burns,
                error: Some(error),
                genesis_info: None,
            }
        };
    let mut burns = Vec::<TokenBurn>::new();
    let mut group_token_id = None;
    let mut genesis_info = None;
    let mut burn_token =
        |meta: TokenMeta, amount: u128, is_mint_baton: bool| {
            add_burn_token(&mut burns, meta, amount, is_mint_baton)
        };
    match &parse_data.tx_type {
        TxType::Genesis(info) => {
            if parse_data.meta.token_type == TokenType::Nft1Child {
                match spent_outputs.get(0) {
                    Some(Some(output))
                        if output.meta.token_type == TokenType::Nft1Group
                            && matches!(output.token, Token::Amount(1..)) =>
                    {
                        group_token_id = Some(output.meta.token_id);
                    }
                    _ => {
                        return make_total_burn(HasNoNft1Group, None);
                    }
                }
            }
            genesis_info = Some(info.clone());
            for (input_idx, output) in spent_outputs.iter().enumerate() {
                if group_token_id.is_some() && input_idx == 0 {
                    continue;
                }
                if let Some(output) = output {
                    burn_token(
                        output.meta,
                        output.token.amount().into(),
                        output.token.is_mint_baton(),
                    );
                }
            }
        }
        TxType::Mint => {
            let mut has_mint_baton = false;
            for output in spent_outputs.iter().flatten() {
                if parse_data.meta == output.meta
                    && output.token == Token::MintBaton
                {
                    // Found mint baton
                    has_mint_baton = true;
                } else {
                    // Invalid SLP input, burn it
                    burn_token(
                        output.meta,
                        output.token.amount().into(),
                        output.token.is_mint_baton(),
                    );
                }
            }
            if !has_mint_baton {
                return make_total_burn(HasNoMintBaton, None);
            }
        }
        TxType::Send => {
            let output_sum = parse_data
                .output_tokens
                .iter()
                .flatten()
                .map(|token| u128::from(token.amount()))
                .sum::<u128>();
            let mut input_sum: u128 = 0;
            for output in spent_outputs.iter().flatten() {
                if parse_data.meta == output.meta
                    && matches!(output.token, Token::Amount(_))
                {
                    // Valid input which is not a mint_baton
                    input_sum += u128::from(output.token.amount());
                    if group_token_id.is_none() {
                        group_token_id = output.group_token_id;
                    }
                } else {
                    // Invalid SLP input, burn it
                    burn_token(
                        output.meta,
                        output.token.amount().into(),
                        output.token.is_mint_baton(),
                    );
                }
            }
            if output_sum > input_sum {
                return make_total_burn(
                    OutputSumExceedInputSum {
                        output_sum,
                        input_sum,
                    },
                    group_token_id,
                );
            }
            if input_sum > output_sum {
                burn_token(parse_data.meta, input_sum - output_sum, false);
            }
        }
        TxType::Unknown => {
            for output in spent_outputs.iter().flatten() {
                burn_token(
                    output.meta,
                    output.token.amount().into(),
                    output.token.is_mint_baton(),
                );
            }
        }
        &TxType::Burn(expected) => {
            let mut actual: u128 = 0;
            for burn in spent_outputs.iter() {
                let burn = match burn {
                    Some(burn) => burn,
                    None => continue,
                };
                if burn.token == Token::Amount(0) {
                    continue;
                }
                if burn.meta.token_id != parse_data.meta.token_id {
                    return make_total_burn(WrongBurnTokenId, None);
                }
                if burn.meta.token_type != parse_data.meta.token_type {
                    return make_total_burn(WrongBurnTokenType, None);
                }
                if burn.token == Token::MintBaton {
                    return make_total_burn(WrongBurnMintBaton, None);
                }
                actual += u128::from(burn.token.amount());
            }
            if u128::from(expected) != actual {
                return make_total_burn(
                    VerifyError::WrongBurnInvalidAmount { expected, actual },
                    None,
                );
            }
        }
    }
    TxData {
        output_tokens: parse_data.output_tokens.clone(),
        tx_type: parse_data.tx_type.tx_type_variant(),
        meta: parse_data.meta,
        group_token_id,
        burns,
        error: None,
        genesis_info,
    }
}
