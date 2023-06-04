use std::ops::Range;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::slpv2::{consts::STANDARD_TOKEN_TYPE, Amount, TokenId};

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub enum TokenType {
    Standard,
    Unknown(u8),
}

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub struct TokenMeta {
    pub token_id: TokenId,
    pub token_type: TokenType,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Section {
    pub meta: TokenMeta,
    pub variant: SectionVariant,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum SectionType {
    GENESIS,
    MINT,
    SEND,
    BURN,
    UNKNOWN,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SectionVariant {
    Genesis(Genesis),
    Mint(MintData),
    Send(Send),
    Burn(Amount),
    Unknown,
}

#[derive(
    Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize,
)]
pub struct GenesisInfo {
    pub token_ticker: Bytes,
    pub token_name: Bytes,
    pub url: Bytes,
    pub data: Bytes,
    pub auth_pubkey: Bytes,
    pub decimals: u8,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Genesis {
    pub data: GenesisInfo,
    pub mint_data: MintData,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MintData {
    pub amounts: Vec<Amount>,
    pub num_batons: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Send(pub Vec<Amount>);

impl SectionVariant {
    pub fn section_type(&self) -> SectionType {
        match self {
            SectionVariant::Genesis(_) => SectionType::GENESIS,
            SectionVariant::Mint(_) => SectionType::MINT,
            SectionVariant::Send(_) => SectionType::SEND,
            SectionVariant::Burn(_) => SectionType::BURN,
            SectionVariant::Unknown => SectionType::UNKNOWN,
        }
    }
}

impl TokenType {
    pub fn to_u8(self) -> u8 {
        match self {
            TokenType::Standard => STANDARD_TOKEN_TYPE,
            TokenType::Unknown(token_type) => token_type,
        }
    }
}

impl TokenMeta {
    pub const fn standard(token_id: TokenId) -> Self {
        TokenMeta {
            token_id,
            token_type: TokenType::Standard,
        }
    }
}

impl MintData {
    pub fn amounts_range(&self) -> Range<usize> {
        1..1 + self.amounts.len()
    }

    pub fn batons_range(&self) -> Range<usize> {
        let start = 1 + self.amounts.len();
        start..start + self.num_batons
    }
}
