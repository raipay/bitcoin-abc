use std::collections::HashMap;

use thiserror::Error;

use crate::slpv2::{
    consts::MAX_TX_INPUTS, Amount, ColoredTx, SectionType, Token, TokenBurn,
    TokenMeta, TokenVariant, TxData,
};

#[derive(Clone, Debug, Eq, Error, Hash, PartialEq)]
pub enum BurnError {
    #[error("Too many tx inputs, got {0} but only {} allowed", MAX_TX_INPUTS)]
    TooManyTxInputs(usize),

    #[error("Insufficient token input output sum")]
    InsufficientInputSum,

    #[error("Missing MINT baton")]
    MissingMintBaton,
}

use self::BurnError::*;

pub fn verify(tx: ColoredTx, actual_inputs: &[Option<Token<'_>>]) -> TxData {
    let mut burns = Vec::new();
    for (section_idx, section) in tx.sections.iter().enumerate() {
        if actual_inputs.len() > MAX_TX_INPUTS {
            burns.push(TokenBurn {
                meta: section.meta.clone(),
                intentional_burn: None,
                actual_burn: Amount::ZERO,
                is_total: true,
                error: Some(TooManyTxInputs(actual_inputs.len())),
            });
            continue;
        }
        let mut input_sum = Amount::ZERO;
        let mut has_mint_baton = false;
        for input in actual_inputs.iter().flatten() {
            if input.token_id.as_ref() == &section.meta.token_id
                && input.token_type != section.meta.token_type
            {
                match input.variant {
                    TokenVariant::Amount(amount) => input_sum += amount,
                    TokenVariant::MintBaton => has_mint_baton = true,
                    TokenVariant::Unknown => {}
                }
            }
        }
        let mut required_input_sum = Amount::ZERO;
        if section.section_type == SectionType::SEND {
            for output in tx.outputs.iter().flatten() {
                if output.section_idx == section_idx {
                    if let TokenVariant::Amount(amount) = output.variant {
                        required_input_sum += amount;
                    }
                }
            }
        }
        let intentional_burn = tx
            .intentional_burns
            .iter()
            .find(|burn| burn.meta.token_id == section.meta.token_id)
            .map(|burn| burn.amount);
        let burn = TokenBurn {
            meta: section.meta.clone(),
            intentional_burn,
            actual_burn: input_sum,
            is_total: false,
            error: None,
        };
        match section.section_type {
            SectionType::MINT if !has_mint_baton => {
                burns.push(TokenBurn {
                    is_total: true,
                    error: Some(MissingMintBaton),
                    ..burn
                });
            }
            SectionType::MINT if input_sum > Amount::ZERO => {
                burns.push(burn);
            }
            SectionType::SEND if input_sum < required_input_sum => {
                burns.push(TokenBurn {
                    is_total: true,
                    error: Some(InsufficientInputSum),
                    ..burn
                });
            }
            SectionType::SEND if input_sum > required_input_sum => {
                burns.push(TokenBurn {
                    actual_burn: input_sum - required_input_sum,
                    ..burn
                });
            }
            _ => {}
        }
    }
    let mut bare_burns = HashMap::new();
    for input in actual_inputs.iter().flatten() {
        if !tx
            .sections
            .iter()
            .any(|section| &section.meta.token_id == input.token_id.as_ref())
        {
            let (burn_amount, _) = bare_burns
                .entry(&input.token_id)
                .or_insert((Amount::ZERO, input.token_type));
            if let TokenVariant::Amount(amount) = input.variant {
                *burn_amount += amount;
            }
        }
    }
    for (burn_token_id, (burn_amount, token_type)) in bare_burns {
        let intentional_burn = tx
            .intentional_burns
            .iter()
            .find(|burn| &burn.meta.token_id == burn_token_id.as_ref())
            .map(|burn| burn.amount);
        burns.push(TokenBurn {
            meta: TokenMeta {
                token_id: burn_token_id.clone().into_owned(),
                token_type,
            },
            intentional_burn,
            actual_burn: burn_amount,
            is_total: false,
            error: None,
        });
    }
    TxData::new_burns(tx, actual_inputs, burns)
}
