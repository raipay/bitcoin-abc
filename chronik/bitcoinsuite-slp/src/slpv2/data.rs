use std::borrow::Cow;

use crate::slpv2::{
    Amount, BurnError, GenesisInfo, SectionType, TokenId, TokenMeta, TokenType, ColoredTx,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxData {
    pub sections: Vec<SectionData>,
    pub burns: Vec<TokenBurn>,
    pub inputs: Vec<Option<TokenData>>,
    pub outputs: Vec<Option<TokenData>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token<'a> {
    pub token_id: Cow<'a, TokenId>,
    pub token_type: TokenType,
    pub variant: TokenVariant,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenVariant {
    Amount(Amount),
    MintBaton,
    Unknown,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SectionData {
    pub meta: TokenMeta,
    pub section_type: SectionType,
    pub genesis_info: Option<GenesisInfo>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenData {
    pub section_idx: usize,
    pub variant: TokenVariant,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TokenBurn {
    pub meta: TokenMeta,
    pub intentional_burn: Option<Amount>,
    pub actual_burn: Amount,
    pub is_total: bool,
    pub error: Option<BurnError>,
}

impl TokenData {
    pub fn amount(section_idx: usize, amount: Amount) -> TokenData {
        TokenData {
            section_idx,
            variant: TokenVariant::Amount(amount),
        }
    }

    pub fn mint_baton(section_idx: usize) -> TokenData {
        TokenData {
            section_idx,
            variant: TokenVariant::MintBaton,
        }
    }

    pub fn unknown(section_idx: usize) -> TokenData {
        TokenData {
            section_idx,
            variant: TokenVariant::Unknown,
        }
    }
}

impl TxData {
    pub fn new_burns(
        tx: ColoredTx,
        inputs: &[Option<Token<'_>>],
        burns: Vec<TokenBurn>,
    ) -> Self {
        let mut remaining_sections = Vec::new();
        let mut replace_token_idx = vec![None; tx.sections.len()];
        for (idx, section) in tx.sections.into_iter().enumerate() {
            let is_total = burns
                .iter()
                .filter(|burn| burn.is_total)
                .any(|burn| burn.meta.token_id == section.meta.token_id);
            if !is_total {
                replace_token_idx[idx] = Some(remaining_sections.len());
                remaining_sections.push(section);
            }
        }
        let outputs = tx.outputs
            .into_iter()
            .map(|entry| {
                entry.and_then(|mut data| {
                    data.section_idx = replace_token_idx[data.section_idx]?;
                    Some(data)
                })
            })
            .collect::<Vec<_>>();
        let inputs = inputs
            .iter()
            .map(|input| {
                input.as_ref().map(|input| {
                    get_token_data(&remaining_sections, &burns, input)
                })
            })
            .collect::<Vec<_>>();
        TxData {
            sections: remaining_sections,
            burns,
            inputs,
            outputs,
        }
    }

    pub fn token_data(&self, token: &Token<'_>) -> TokenData {
        get_token_data(&self.sections, &self.burns, token)
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

    pub fn token(&self, token_output: &TokenData) -> Token<'_> {
        let meta = if token_output.section_idx < self.sections.len() {
            &self.sections[token_output.section_idx].meta
        } else {
            &self.burns[token_output.section_idx - self.sections.len()].meta
        };
        Token {
            token_id: Cow::Borrowed(&meta.token_id),
            token_type: meta.token_type,
            variant: token_output.variant,
        }
    }
}

impl Token<'_> {
    pub fn to_static(&self) -> Token<'static> {
        Token {
            token_id: Cow::Owned(self.token_id.clone().into_owned()),
            token_type: self.token_type,
            variant: self.variant,
        }
    }
}

fn get_token_data(
    sections: &[SectionData],
    burns: &[TokenBurn],
    token: &Token<'_>,
) -> TokenData {
    TokenData {
        section_idx: sections
            .iter()
            .position(|section| section.meta.token_id == *token.token_id)
            .or_else(|| {
                burns
                    .iter()
                    .position(|burn| {
                        &burn.meta.token_id == token.token_id.as_ref()
                    })
                    .map(|idx| idx + sections.len())
            })
            .unwrap(),
        variant: token.variant,
    }
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.variant {
            TokenVariant::Amount(amount) => {
                write!(f, "{} tokens for token ID {}", amount, self.token_id)
            }
            TokenVariant::MintBaton => {
                write!(f, "Mint baton for token ID {}", self.token_id)
            }
            TokenVariant::Unknown => {
                write!(f, "Unknown token type {}", self.token_type.to_u8())
            }
        }
    }
}
