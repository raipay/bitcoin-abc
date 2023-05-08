use std::collections::{BTreeSet, HashSet};

use itertools::Itertools;
use thiserror::Error;

use crate::slpv2::{Amount, Token, TokenId, TxSpec, SectionType};

#[derive(Debug, Error, PartialEq)]
pub enum MismatchError {
    #[error(
        "Mismatched token input and output sum for {token_id} at section index {section_idx}: Expected input sum {expected_sum} != actual input sum {actual_sum} (with {burn_amount} intentional burn)"
    )]
    MismatchedInputSum {
        section_idx: usize,
        token_id: TokenId,
        expected_sum: Amount,
        actual_sum: Amount,
        burn_amount: Amount,
    },

    #[error(
        "Missing MINT baton for {token_id} at section index {section_idx}"
    )]
    MissingMintBaton {
        section_idx: usize,
        token_id: TokenId,
    },
}

use self::MismatchError::*;

pub fn verify(
    data: &mut TxSpec,
    actual_inputs: &[Option<Token<'_>>],
) -> Vec<MismatchError> {
    let mut burn_token_ids = BTreeSet::new();
    let mut burns = Vec::new();
    for (section_idx, section) in data.sections.iter().enumerate() {
        let mut input_sum = 0;
        let mut has_mint_baton = false;
        for input in actual_inputs.iter().flatten() {
            if input.token_id.as_ref() == &section.meta.token_id {
                input_sum += input.amount;
                if input.is_mint_baton {
                    has_mint_baton = true;
                }
            }
        }
        if input_sum != section.expected_input_sum {
            burns.push(MismatchedInputSum {
                section_idx,
                token_id: section.meta.token_id,
                expected_sum: section.expected_input_sum,
                actual_sum: input_sum,
                burn_amount: section.intentional_burn_amount,
            });
            burn_token_ids.insert(section.meta.token_id);
        }
        if section.section_type == SectionType::MINT && !has_mint_baton {
            burns.push(MissingMintBaton {
                section_idx,
                token_id: section.meta.token_id,
            });
            burn_token_ids.insert(section.meta.token_id);
        }
    }
    data.burn_token_ids(&burn_token_ids);
    burns
}
