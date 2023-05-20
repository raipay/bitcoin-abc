use std::collections::BTreeSet;

use thiserror::Error;

use crate::slpv2::{Amount, SectionType, Token, TokenId, TxSpec, TxData};

#[derive(Debug, Error, PartialEq)]
pub enum VerifyError {
    #[error(
        "Insufficient token input output sum for {token_id} at section index {section_idx}: input sum {input_sum} < output sum {output_sum}"
    )]
    InsufficientInputSum {
        section_idx: usize,
        token_id: TokenId,
        input_sum: Amount,
        output_sum: Amount,
    },

    #[error(
        "Missing MINT baton for {token_id} at section index {section_idx}"
    )]
    MissingMintBaton {
        section_idx: usize,
        token_id: TokenId,
    },
}

use self::VerifyError::*;

pub fn verify(
    data: TxSpec,
    actual_inputs: &[Option<Token<'_>>],
) -> (TxData, Vec<VerifyError>) {
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
        if input_sum < section.required_input_sum {
            burns.push(InsufficientInputSum {
                section_idx,
                token_id: section.meta.token_id,
                input_sum,
                output_sum: section.required_input_sum,
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
    let burn_token_ids = burn_token_ids.into_iter().collect::<Vec<_>>();
    let tx_data = data.into_tx_data(actual_inputs, burn_token_ids);
    (tx_data, burns)
}
