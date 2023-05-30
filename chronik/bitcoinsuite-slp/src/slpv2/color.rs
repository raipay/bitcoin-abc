use std::borrow::Cow;

use bitcoinsuite_core::tx::{Tx, TxId};
use bytes::Bytes;
use thiserror::Error;

use crate::{
    empp,
    slpv2::{
        parse_section, Amount, Genesis, GenesisInfo, MintData, ParseError,
        SectionData, SectionType, SectionVariant, Send, Token, TokenData,
        TokenId, TokenMeta, TokenVariant,
    },
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColoredTx {
    pub sections: Vec<SectionData>,
    pub intentional_burns: Vec<IntentionalBurn>,
    pub outputs: Vec<Option<TokenData>>,
    pub errors: Vec<ColorError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IntentionalBurn {
    pub meta: TokenMeta,
    pub amount: Amount,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ColorError {
    #[error("No outputs")]
    NoOutputs,

    #[error("eMPP parse failed: {0}")]
    EmppError(#[from] empp::ParseError),

    #[error("Error at pushdata index {pushdata_idx}: {error}")]
    SectionError {
        pushdata_idx: usize,
        error: ColorSectionError,
    },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ColorSectionError {
    #[error("SLPv2 parse failed: {0}")]
    Slpv2ParseError(#[from] ParseError),

    #[error("Too few outputs, expected {expected} but got {actual}")]
    TooFewOutputs { expected: usize, actual: usize },

    #[error("GENESIS must be the first pushdata")]
    GenesisMustBeFirst,

    #[error("Descending token type: {before} > {after}, token types must be in ascending order")]
    DescendingTokenType { before: u8, after: u8 },

    #[error(
        "Duplicate token_id {token_id}, found in section {prev_section_idx}"
    )]
    DuplicateTokenId {
        prev_section_idx: usize,
        token_id: TokenId,
    },

    #[error("Duplicate intentional burn token_id {token_id}, found in burn #{prev_burn_idx} and #{burn_idx}")]
    DuplicateIntentionalBurnTokenId {
        prev_burn_idx: usize,
        burn_idx: usize,
        token_id: TokenId,
    },

    #[error("Overlapping amount")]
    OverlappingAmount {
        prev_token: Token<'static>,
        amount_idx: usize,
        amount: Amount,
    },

    #[error("Overlapping mint baton")]
    OverlappingMintBaton {
        prev_token: Token<'static>,
        baton_idx: usize,
    },
}

use self::ColorError::*;
use self::ColorSectionError::*;

impl ColoredTx {
    pub fn parse_tx(tx: &Tx) -> ColoredTx {
        let mut colored = ColoredTx {
            sections: vec![],
            intentional_burns: vec![],
            outputs: vec![None; tx.outputs.len()],
            errors: vec![],
        };
        let pushdata = match Self::parse_pushdata(tx) {
            Ok(pushdata) => pushdata,
            Err(err) => {
                colored.errors = vec![err];
                return colored;
            }
        };
        colored.color_pushdata(pushdata, tx.txid_ref());
        colored
    }

    fn parse_pushdata(tx: &Tx) -> Result<Vec<Bytes>, ColorError> {
        let first_output = tx.outputs.get(0).ok_or(NoOutputs)?;
        empp::parse(&first_output.script).map_err(EmppError)
    }

    fn color_pushdata(&mut self, pushdata: Vec<Bytes>, txid: &TxId) {
        let mut max_token_type = 0;
        for (pushdata_idx, pushdata) in pushdata.into_iter().enumerate() {
            if let Err(error) = self.color_section(
                pushdata_idx,
                pushdata,
                txid,
                &mut max_token_type,
            ) {
                self.errors.push(SectionError {
                    pushdata_idx,
                    error,
                });
            }
        }
    }

    fn color_section(
        &mut self,
        pushdata_idx: usize,
        pushdata: Bytes,
        txid: &TxId,
        max_token_type: &mut u8,
    ) -> Result<(), ColorSectionError> {
        let section = parse_section(txid, pushdata)?;
        let meta = section.meta.clone();
        if *max_token_type > meta.token_type.to_u8() {
            return Err(DescendingTokenType {
                before: *max_token_type,
                after: meta.token_type.to_u8(),
            });
        }
        *max_token_type = meta.token_type.to_u8();
        if matches!(
            section.variant.section_type(),
            SectionType::MINT | SectionType::SEND,
        ) {
            for (prev_section_idx, prev_section) in
                self.sections.iter().enumerate()
            {
                if prev_section.meta.token_id == meta.token_id {
                    return Err(DuplicateTokenId {
                        prev_section_idx,
                        token_id: meta.token_id,
                    });
                }
            }
        }
        match section.variant {
            SectionVariant::Genesis(genesis) => {
                self.color_genesis(pushdata_idx, meta, genesis)
            }
            SectionVariant::Mint(mint) => self.color_mint(meta, mint),
            SectionVariant::Send(send) => self.color_send(meta, send),
            SectionVariant::Burn(amount) => self.color_burn(meta, amount),
            SectionVariant::Unknown => self.color_unknown(meta),
        }
    }

    fn color_genesis(
        &mut self,
        pushdata_idx: usize,
        meta: TokenMeta,
        genesis: Genesis,
    ) -> Result<(), ColorSectionError> {
        if pushdata_idx != 0 {
            return Err(GenesisMustBeFirst);
        }
        self.color_mint_data(&genesis.mint_data)?;
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::GENESIS,
            genesis_info: Some(genesis.data),
        });
        Ok(())
    }

    fn color_mint(
        &mut self,
        meta: TokenMeta,
        mint: MintData,
    ) -> Result<(), ColorSectionError> {
        self.color_mint_data(&mint)?;
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::MINT,
            genesis_info: None,
        });
        Ok(())
    }

    fn color_mint_data(
        &mut self,
        mint_data: &MintData,
    ) -> Result<(), ColorSectionError> {
        let section_idx = self.sections.len();
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
                if amount != Amount::ZERO {
                    return Err(OverlappingAmount {
                        prev_token: self.token(token).to_static(),
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
                    baton_idx,
                });
            }
        }
        for (output, amount) in self.outputs[mint_data.amounts_range()]
            .iter_mut()
            .zip(mint_data.amounts.iter().copied())
        {
            if amount > Amount::ZERO {
                *output = Some(TokenData {
                    section_idx,
                    variant: TokenVariant::Amount(amount),
                });
            }
        }
        for output in self.outputs[mint_data.batons_range()].iter_mut() {
            *output = Some(TokenData {
                section_idx,
                variant: TokenVariant::MintBaton,
            });
        }
        Ok(())
    }

    fn color_send(
        &mut self,
        meta: TokenMeta,
        send: Send,
    ) -> Result<(), ColorSectionError> {
        let amounts = send.0;
        if self.outputs.len() < amounts.len() + 1 {
            return Err(TooFewOutputs {
                expected: amounts.len() + 1,
                actual: self.outputs.len(),
            });
        }
        for (idx, &amount) in amounts.iter().enumerate() {
            if amount > Amount::ZERO {
                if let Some(token) = &self.outputs[idx + 1] {
                    return Err(OverlappingAmount {
                        prev_token: self.token(token).to_static(),
                        amount_idx: idx,
                        amount,
                    });
                }
            }
        }
        for (idx, &amount) in amounts.iter().enumerate() {
            if amount == Amount::ZERO {
                continue;
            }
            self.outputs[idx + 1] = Some(TokenData {
                section_idx: self.sections.len(),
                variant: TokenVariant::Amount(amount),
            });
        }
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::SEND,
            genesis_info: None,
        });
        Ok(())
    }

    fn color_burn(
        &mut self,
        meta: TokenMeta,
        amount: Amount,
    ) -> Result<(), ColorSectionError> {
        for (prev_burn_idx, prev_burn) in
            self.intentional_burns.iter().enumerate()
        {
            if prev_burn.meta.token_id == meta.token_id {
                return Err(DuplicateIntentionalBurnTokenId {
                    prev_burn_idx,
                    burn_idx: self.intentional_burns.len(),
                    token_id: meta.token_id,
                });
            }
        }
        self.intentional_burns
            .push(IntentionalBurn { meta, amount });
        Ok(())
    }

    fn color_unknown(
        &mut self,
        meta: TokenMeta,
    ) -> Result<(), ColorSectionError> {
        for token_data in self.outputs.iter_mut().skip(1) {
            if token_data.is_none() {
                *token_data = Some(TokenData {
                    section_idx: self.sections.len(),
                    variant: TokenVariant::Unknown,
                });
            }
        }
        self.sections.push(SectionData {
            meta,
            section_type: SectionType::UNKNOWN,
            genesis_info: None,
        });
        Ok(())
    }

    pub fn token(&self, token_output: &TokenData) -> Token<'_> {
        let section = &self.sections[token_output.section_idx];
        Token {
            token_id: Cow::Borrowed(&section.meta.token_id),
            token_type: section.meta.token_type,
            variant: token_output.variant,
        }
    }
}
