use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet},
};

use bitcoinsuite_core::tx::Tx;
use thiserror::Error;

use crate::slpv2::{
    Amount, Genesis, MintData, ParseError, Parsed, Section, SectionVariant,
    Send, TokenId, TokenMeta, TokenType,
};

#[derive(Clone, Debug, Default)]
pub struct TxSpec {
    pub sections: Vec<SectionData>,
    pub burn_token_ids: Vec<TokenId>,
    pub outputs: Vec<Option<TokenOutputData>>,
}

#[derive(Clone, Debug, Default)]
pub struct TxData {
    pub sections: Vec<SectionData>,
    pub burn_token_ids: Vec<TokenId>,
    pub inputs: Vec<Option<TokenOutputData>>,
    pub outputs: Vec<Option<TokenOutputData>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SectionType {
    GENESIS,
    MINT,
    SEND,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token<'a> {
    pub token_id: Cow<'a, TokenId>,
    pub amount: Amount,
    pub is_mint_baton: bool,
}

#[derive(Clone, Debug)]
pub struct SectionData {
    pub meta: TokenMeta,
    pub section_type: SectionType,
    pub expected_input_sum: Amount,
    pub intentional_burn_amount: Amount,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenOutputData {
    pub section_idx: usize,
    pub amount: Amount,
    pub is_mint_baton: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum ProcessError {
    #[error("Too few outputs, expected {expected} but got {actual}")]
    TooFewOutputs { expected: usize, actual: usize },

    #[error(
        "GENESIS must be the first section, but found GENESIS at index {0}"
    )]
    GenesisMustBeFirst(usize),

    #[error("Duplicate token_id {token_id}, found in section {prev_section_idx} and {section_idx}")]
    DuplicateTokenId {
        prev_section_idx: usize,
        section_idx: usize,
        token_id: TokenId,
    },

    #[error("Overlapping amount")]
    OverlappingAmount {
        prev_token: Token<'static>,
        section_idx: usize,
        amount_idx: usize,
        amount: Amount,
    },

    #[error("Overlapping mint baton")]
    OverlappingMintBaton {
        prev_token: Token<'static>,
        section_idx: usize,
        baton_idx: usize,
    },
}

use self::ProcessError::*;

impl TxSpec {
    pub fn process_parsed(
        parsed: &Parsed,
        tx: &Tx,
    ) -> (TxSpec, Option<ProcessError>) {
        let mut tx_data = TxSpec {
            sections: vec![],
            burn_token_ids: vec![],
            outputs: vec![None; tx.outputs.len()],
        };
        for section in &parsed.sections {
            if let Err(err) = tx_data.process_section(section) {
                return (tx_data, Some(err));
            }
        }
        (tx_data, None)
    }

    fn process_section(
        &mut self,
        section: &Section,
    ) -> Result<(), ProcessError> {
        let meta = section.meta.clone();
        for (prev_section_idx, prev_section) in self.sections.iter().enumerate()
        {
            if prev_section.meta.token_id == meta.token_id {
                return Err(DuplicateTokenId {
                    prev_section_idx,
                    section_idx: self.sections.len(),
                    token_id: meta.token_id,
                });
            }
        }
        match &section.variant {
            SectionVariant::Genesis(genesis) => {
                self.process_genesis(meta, genesis)
            }
            SectionVariant::Mint(mint) => self.process_mint(meta, mint),
            SectionVariant::Send(send) => self.process_send(meta, send),
        }
    }

    fn process_genesis(
        &mut self,
        meta: TokenMeta,
        genesis: &Genesis,
    ) -> Result<(), ProcessError> {
        if !self.sections.is_empty() {
            return Err(GenesisMustBeFirst(self.sections.len()));
        }
        self.check_mint_outputs(&genesis.mint_data)?;
        self.process_mint_outputs(&genesis.mint_data);
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::GENESIS,
            expected_input_sum: 0,
            intentional_burn_amount: 0,
        });
        Ok(())
    }

    fn process_mint(
        &mut self,
        meta: TokenMeta,
        mint: &MintData,
    ) -> Result<(), ProcessError> {
        self.check_mint_outputs(mint)?;

        self.process_mint_outputs(mint);
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::MINT,
            expected_input_sum: 0,
            intentional_burn_amount: 0,
        });
        Ok(())
    }

    fn process_send(
        &mut self,
        meta: TokenMeta,
        send: &Send,
    ) -> Result<(), ProcessError> {
        let output_sum =
            self.check_send_amounts(&send.output_amounts, &self.outputs)?;

        let burn_amount = send.intentional_burn_amount.unwrap_or_default();

        Self::process_send_amounts(
            self.sections.len(),
            &send.output_amounts,
            &mut self.outputs[1..],
        );
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::SEND,
            expected_input_sum: output_sum + burn_amount,
            intentional_burn_amount: burn_amount,
        });
        Ok(())
    }

    fn check_mint_outputs(
        &mut self,
        mint_data: &MintData,
    ) -> Result<(), ProcessError> {
        if self.outputs.len() < mint_data.batons_range().end {
            return Err(TooFewOutputs {
                expected: mint_data.batons_range().end,
                actual: self.outputs.len(),
            });
        }
        let iter_mint_amounts = self.outputs[mint_data.amounts_range()]
            .iter()
            .zip(mint_data.amounts.iter().copied())
            .enumerate();
        for (amount_idx, (token, amount)) in iter_mint_amounts {
            if let Some(token) = token {
                if amount != 0 {
                    return Err(OverlappingAmount {
                        prev_token: self.token(token).to_static(),
                        section_idx: self.sections.len(),
                        amount_idx,
                        amount,
                    });
                }
            }
        }
        for (baton_idx, output) in
            self.outputs[mint_data.batons_range()].iter().enumerate()
        {
            if let Some(output) = output {
                return Err(OverlappingMintBaton {
                    prev_token: self.token(output).to_static(),
                    section_idx: self.sections.len(),
                    baton_idx,
                });
            }
        }
        Ok(())
    }

    fn process_mint_outputs(&mut self, mint_data: &MintData) {
        let section_idx = self.sections.len();
        for (output, amount) in self.outputs[mint_data.amounts_range()]
            .iter_mut()
            .zip(mint_data.amounts.iter().copied())
        {
            if amount > 0 {
                *output = Some(TokenOutputData {
                    section_idx,
                    amount,
                    is_mint_baton: false,
                });
            }
        }
        for output in self.outputs[mint_data.batons_range()].iter_mut() {
            *output = Some(TokenOutputData {
                section_idx,
                amount: 0,
                is_mint_baton: true,
            });
        }
    }

    fn check_send_amounts(
        &self,
        amounts: &[Amount],
        tokens: &[Option<TokenOutputData>],
    ) -> Result<Amount, ProcessError> {
        if tokens.len() < amounts.len() + 1 {
            return Err(TooFewOutputs {
                expected: amounts.len() + 1,
                actual: tokens.len(),
            });
        }
        let mut sum = 0;
        for (idx, &amount) in amounts.iter().enumerate() {
            sum += amount;
            if amount > 0 {
                self.check_non_overlapping(tokens, idx + 1, amount)?;
            }
        }
        Ok(sum)
    }

    fn process_send_amounts(
        section_idx: usize,
        amounts: &[Amount],
        tokens: &mut [Option<TokenOutputData>],
    ) {
        for (idx, &amount) in amounts.iter().enumerate() {
            if amount == 0 {
                continue;
            }
            tokens[idx] = Some(TokenOutputData {
                section_idx,
                amount,
                is_mint_baton: false,
            });
        }
    }

    fn check_non_overlapping(
        &self,
        tokens: &[Option<TokenOutputData>],
        idx: usize,
        amount: Amount,
    ) -> Result<(), ProcessError> {
        if let Some(token) = &tokens[idx] {
            let prev_token = self.token(token).to_static();
            return Err(OverlappingAmount {
                prev_token,
                section_idx: self.sections.len(),
                amount_idx: idx,
                amount,
            });
        }
        Ok(())
    }

    pub fn token(&self, token_output: &TokenOutputData) -> Token<'_> {
        Token {
            token_id: Cow::Borrowed(
                if token_output.section_idx < self.sections.len() {
                    &self.sections[token_output.section_idx].meta.token_id
                } else {
                    &self.burn_token_ids
                        [token_output.section_idx - self.sections.len()]
                },
            ),
            amount: token_output.amount,
            is_mint_baton: token_output.is_mint_baton,
        }
    }

    pub fn burn_token_ids(&mut self, token_ids: &BTreeSet<TokenId>) {
        let mut remaining_sections = Vec::with_capacity(
            self.sections.len().saturating_sub(token_ids.len()),
        );
        let mut replace_token_idx = vec![None; self.sections.len()];
        let sections = std::mem::take(&mut self.sections);
        for (idx, section) in sections.into_iter().enumerate() {
            if !token_ids.contains(&section.meta.token_id) {
                replace_token_idx[idx] = Some(remaining_sections.len());
                remaining_sections.push(section);
            }
        }
        for entry in self.outputs.iter_mut() {
            if let Some(data) = entry {
                match replace_token_idx[data.section_idx] {
                    Some(token_idx) => data.section_idx = token_idx,
                    None => *entry = None,
                }
            }
        }
        self.sections = remaining_sections;
        self.burn_token_ids.extend(token_ids.iter().cloned());
    }
}

impl TxData {
    pub fn from_spec_and_inputs(
        tx_spec: TxSpec,
        input_tokens: &[Option<Token<'_>>],
    ) -> Self {
        let mut tx_data = TxData {
            sections: tx_spec.sections,
            burn_token_ids: tx_spec.burn_token_ids,
            inputs: Vec::with_capacity(input_tokens.len()),
            outputs: tx_spec.outputs,
        };
        for input_token in input_tokens {
            match input_token {
                Some(token) => {
                    tx_data.inputs.push(Some(tx_data.token_output_data(token)));
                },
                None => tx_data.inputs.push(None),
            }
        }
        tx_data
    }

    pub fn token_output_data(&self, token: &Token<'_>) -> TokenOutputData {
        TokenOutputData {
            section_idx: self
                .sections
                .iter()
                .position(|section| section.meta.token_id == *token.token_id)
                .or_else(|| {
                    self.burn_token_ids
                        .iter()
                        .position(|token_id| {
                            token_id == token.token_id.as_ref()
                        })
                        .map(|idx| idx + self.sections.len())
                })
                .unwrap(),
            amount: token.amount,
            is_mint_baton: token.is_mint_baton,
        }
    }

    pub fn inputs(&self) -> impl ExactSizeIterator<Item = Option<Token<'_>>> {
        self.inputs
            .iter()
            .map(|input| input.as_ref().map(|input| self.token(input)))
    }

    pub fn outputs(&self) -> impl ExactSizeIterator<Item = Option<Token<'_>>> {
        self.outputs
            .iter()
            .map(|output| output.as_ref().map(|output| self.token(output)))
    }

    pub fn token(&self, token_output: &TokenOutputData) -> Token<'_> {
        Token {
            token_id: Cow::Borrowed(
                if token_output.section_idx < self.sections.len() {
                    &self.sections[token_output.section_idx].meta.token_id
                } else {
                    &self.burn_token_ids
                        [token_output.section_idx - self.sections.len()]
                },
            ),
            amount: token_output.amount,
            is_mint_baton: token_output.is_mint_baton,
        }
    }
}

impl Token<'_> {
    pub fn to_static(&self) -> Token<'static> {
        Token {
            token_id: Cow::Owned(self.token_id.clone().into_owned()),
            amount: self.amount,
            is_mint_baton: self.is_mint_baton,
        }
    }
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_mint_baton {
            write!(f, "Mint baton for token ID {}", self.token_id)
        } else {
            write!(f, "{} tokens for token ID {}", self.amount, self.token_id)
        }
    }
}
