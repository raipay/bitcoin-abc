use serde::{Deserialize, Serialize};

use crate::{
    slp,
    slpv2::{
        Amount, BurnError, ColorError, ColoredTx, GenesisInfo, Int,
        SectionType, TokenMeta,
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxData {
    pub sections: Vec<SectionData>,
    pub burns: Vec<TokenBurn>,
    pub outputs: Vec<Option<TokenData>>,
    pub color_errors: Vec<ColorError>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub meta: TokenMeta,
    pub variant: TokenVariant,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenVariant {
    Amount(Amount),
    MintBaton,
    Unknown(u8),
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
    pub burn_mint_batons: bool,
    pub is_total: bool,
    pub error: Option<BurnError>,
}

impl TokenVariant {
    pub fn amount(&self) -> Amount {
        match self {
            &TokenVariant::Amount(amount) => amount,
            TokenVariant::MintBaton => Amount::ZERO,
            TokenVariant::Unknown(_) => Amount::ZERO,
        }
    }

    pub fn is_mint_baton(&self) -> bool {
        *self == TokenVariant::MintBaton
    }
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

    pub fn unknown(section_idx: usize, token_type: u8) -> TokenData {
        TokenData {
            section_idx,
            variant: TokenVariant::Unknown(token_type),
        }
    }
}

impl TxData {
    pub fn new_burns(
        tx: ColoredTx,
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
        let outputs = tx
            .outputs
            .into_iter()
            .map(|entry| {
                entry.and_then(|mut data| {
                    data.section_idx = replace_token_idx[data.section_idx]?;
                    Some(data)
                })
            })
            .collect::<Vec<_>>();
        TxData {
            sections: remaining_sections,
            burns,
            outputs,
            color_errors: tx.errors,
        }
    }

    pub fn token_data(&self, token: &Token) -> TokenData {
        get_token_data(&self.sections, &self.burns, token)
    }

    pub fn outputs(&self) -> impl ExactSizeIterator<Item = Option<Token>> + '_ {
        self.outputs
            .iter()
            .map(|output| output.as_ref().map(|output| self.token(output)))
    }

    pub fn token(&self, token_output: &TokenData) -> Token {
        let meta = if token_output.section_idx < self.sections.len() {
            self.sections[token_output.section_idx].meta
        } else {
            self.burns[token_output.section_idx - self.sections.len()].meta
        };
        Token {
            meta,
            variant: token_output.variant,
        }
    }
}

fn get_token_data(
    sections: &[SectionData],
    burns: &[TokenBurn],
    token: &Token,
) -> TokenData {
    TokenData {
        section_idx: sections
            .iter()
            .position(|section| section.meta == token.meta)
            .or_else(|| {
                burns
                    .iter()
                    .position(|burn| burn.meta == token.meta)
                    .map(|idx| idx + sections.len())
            })
            .unwrap(),
        variant: token.variant,
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.variant {
            TokenVariant::Amount(amount) => {
                write!(
                    f,
                    "{} tokens for token ID {}",
                    amount, self.meta.token_id
                )
            }
            TokenVariant::MintBaton => {
                write!(f, "Mint baton for token ID {}", self.meta.token_id)
            }
            TokenVariant::Unknown(token_type) => {
                write!(f, "Unknown token type {}", token_type)
            }
        }
    }
}

impl TokenVariant {
    pub fn to_slpv1(self) -> Option<slp::Token> {
        Some(match self {
            TokenVariant::Amount(amount) => {
                slp::Token::Amount(amount.int().try_into().ok()?)
            }
            TokenVariant::MintBaton => slp::Token::MintBaton,
            TokenVariant::Unknown(token_type) => {
                slp::Token::Unknown(token_type.into())
            }
        })
    }

    pub fn from_slpv1(slp: slp::Token) -> Option<Self> {
        Some(match slp {
            slp::Token::Amount(amount) => {
                TokenVariant::Amount(Amount::new(amount.try_into().ok()?))
            }
            slp::Token::MintBaton => TokenVariant::MintBaton,
            slp::Token::Unknown(token_type) => {
                TokenVariant::Unknown(token_type.try_into().ok()?)
            }
        })
    }
}
