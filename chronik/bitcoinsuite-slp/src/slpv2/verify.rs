use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::slpv2::{
    consts::MAX_TX_INPUTS, Amount, ColoredTx, SectionType, Token, TokenBurn,
    TokenVariant, TxData,
};

#[derive(Clone, Debug, Deserialize, Eq, Error, Hash, PartialEq, Serialize)]
pub enum BurnError {
    #[error("Too many tx inputs, got {0} but only {} allowed", MAX_TX_INPUTS)]
    TooManyTxInputs(usize),

    #[error("Missing MINT baton")]
    MissingMintBaton,

    #[error("Insufficient token input output sum")]
    InsufficientInputSum(Amount),
}

use self::BurnError::*;

pub fn verify(tx: ColoredTx, actual_inputs: &[Option<Token>]) -> TxData {
    let mut burns = Vec::new();
    for (section_idx, section) in tx.sections.iter().enumerate() {
        let mut has_mint_baton = false;
        for input in actual_inputs.iter().flatten() {
            if input.meta == section.meta && input.variant.is_mint_baton() {
                has_mint_baton = true;
            }
        }
        if actual_inputs.len() > MAX_TX_INPUTS {
            burns.push(TokenBurn {
                meta: section.meta,
                intentional_burn: None,
                actual_burn: Amount::ZERO,
                burn_mint_batons: has_mint_baton,
                is_total: true,
                error: Some(TooManyTxInputs(actual_inputs.len())),
            });
            continue;
        }
        let mut input_sum = Amount::ZERO;
        for input in actual_inputs.iter().flatten() {
            if input.meta == section.meta {
                if let TokenVariant::Amount(amount) = input.variant {
                    input_sum += amount;
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
            .find(|burn| burn.meta == section.meta)
            .map(|burn| burn.amount);
        let burn = TokenBurn {
            meta: section.meta,
            intentional_burn,
            actual_burn: input_sum,
            burn_mint_batons: false,
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
                    error: Some(InsufficientInputSum(required_input_sum)),
                    burn_mint_batons: has_mint_baton,
                    ..burn
                });
            }
            SectionType::SEND if input_sum > required_input_sum => {
                burns.push(TokenBurn {
                    actual_burn: input_sum - required_input_sum,
                    burn_mint_batons: has_mint_baton,
                    ..burn
                });
            }
            _ if intentional_burn.is_some() => burns.push(TokenBurn {
                actual_burn: Amount::ZERO,
                ..burn
            }),
            _ => {}
        }
    }
    let mut bare_burns = HashMap::new();
    for input in actual_inputs.iter().flatten() {
        if !tx.sections.iter().any(|section| section.meta == input.meta) {
            let (burn_amount, any_mint_batons) = bare_burns
                .entry(&input.meta)
                .or_insert((Amount::ZERO, false));
            match input.variant {
                TokenVariant::Amount(amount) => *burn_amount += amount,
                TokenVariant::MintBaton => *any_mint_batons = true,
                TokenVariant::Unknown(_) => {}
            }
        }
    }
    for (burn_meta, (burn_amount, burn_mint_batons)) in bare_burns {
        let intentional_burn = tx
            .intentional_burns
            .iter()
            .find(|burn| &burn.meta == burn_meta)
            .map(|burn| burn.amount);
        burns.push(TokenBurn {
            meta: *burn_meta,
            intentional_burn,
            actual_burn: burn_amount,
            burn_mint_batons,
            is_total: false,
            error: None,
        });
    }
    for intentional_burn in &tx.intentional_burns {
        if !burns
            .iter()
            .any(|burn| burn.meta.token_id == intentional_burn.meta.token_id)
        {
            burns.push(TokenBurn {
                meta: intentional_burn.meta,
                intentional_burn: Some(intentional_burn.amount),
                actual_burn: Amount::ZERO,
                burn_mint_batons: false,
                is_total: false,
                error: None,
            });
        }
    }
    TxData::new_burns(tx, burns)
}
