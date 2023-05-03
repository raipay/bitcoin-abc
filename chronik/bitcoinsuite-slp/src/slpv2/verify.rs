use std::collections::{HashSet, BTreeSet};

use itertools::Itertools;
use thiserror::Error;

use crate::slpv2::{Amount, TokenId, Token, TxData};

#[derive(Debug, Error, PartialEq)]
pub enum MismatchError {
    #[error("Unspecified input at index {input_idx}: Specified no input, but input actually has {actual_input}")]
    UnspecifiedInput {
        input_idx: usize,
        actual_input: Token<'static>,
    },

    #[error("Mismatch input at index {input_idx}: Specified {expected_input}, but input actually has no token output")]
    MismatchEmpty {
        input_idx: usize,
        expected_input: Token<'static>,
    },

    #[error("Mismatch input at index {input_idx}: Specified {expected_input}, but input actually has {actual_input}")]
    Mismatch {
        input_idx: usize,
        expected_input: Token<'static>,
        actual_input: Token<'static>,
    },
}

use self::MismatchError::*;

pub fn verify(
    data: &mut TxData,
    actual_inputs: &[Option<Token<'_>>],
) -> Vec<MismatchError> {
    let mut burn_token_ids = BTreeSet::new();
    let mut burns = Vec::new();
    for (input_idx, actual_input) in actual_inputs.iter().enumerate()
    {
        let expected_input = data.inputs().nth(input_idx).flatten();
        match (&expected_input, actual_input) {
            (None, None) => {}
            (None, Some(input)) => {
                burns.push(UnspecifiedInput {
                    input_idx,
                    actual_input: input.to_static(),
                });
                burn_token_ids.insert(*input.token_id);
            }
            (Some(input), None) => {
                burns.push(MismatchEmpty {
                    input_idx,
                    expected_input: input.to_static(),
                });
                burn_token_ids.insert(*input.token_id);
            }
            (Some(expected_input), Some(actual_input)) => {
                if expected_input != actual_input {
                    burns.push(Mismatch {
                        input_idx,
                        expected_input: expected_input.to_static(),
                        actual_input: actual_input.to_static(),
                    });
                    burn_token_ids.insert(*expected_input.token_id);
                    burn_token_ids.insert(*actual_input.token_id);
                }
            }
        }
    }
    data.burn_token_ids(&burn_token_ids, actual_inputs);
    burns
}
