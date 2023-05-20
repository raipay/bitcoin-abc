use std::ops::Range;

use bytes::Bytes;

use crate::slpv2::{TokenId, DEFAULT_TOKEN_TYPE, SectionType, ParseError};

pub type Amount = i64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TokenType {
    Standard = DEFAULT_TOKEN_TYPE,
}

#[derive(Clone, Debug)]
pub struct TokenMeta {
    pub token_id: TokenId,
    pub token_type: TokenType,
}

#[derive(Debug)]
pub struct Section {
    pub meta: TokenMeta,
    pub variant: SectionVariant,
}

#[derive(Debug)]
pub enum SectionVariant {
    Genesis(Genesis),
    Mint(MintData),
    Send(Send),
}

#[derive(Clone, Debug)]
pub struct GenesisData {
    pub token_ticker: Bytes,
    pub token_name: Bytes,
    pub url: Bytes,
    pub data: Bytes,
    pub auth_pubkey: Bytes,
    pub decimals: u8,
}

#[derive(Clone, Debug)]
pub struct Genesis {
    pub data: GenesisData,
    pub mint_data: MintData,
}

#[derive(Clone, Debug)]
pub struct MintData {
    pub amounts: Vec<Amount>,
    pub num_batons: usize,
}

#[derive(Clone, Debug)]
pub struct Send(pub Vec<Amount>);

#[derive(Debug)]
pub enum ParsedPushdata {
    Section(Section),
    IntentionalBurn(IntentionalBurn),
    Error(ParseError),
}

#[derive(Clone, Debug, Default)]
pub struct IntentionalBurn {
    pub token_id: TokenId,
    pub amount: Amount,
}

pub struct TokenAmount<'a> {
    pub token_id: &'a TokenId,
    pub amount: Amount,
}

impl SectionVariant {
    pub fn section_type(&self) -> SectionType {
        match self {
            SectionVariant::Genesis(_) => SectionType::GENESIS,
            SectionVariant::Mint(_) => SectionType::MINT,
            SectionVariant::Send(_) => SectionType::SEND,
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
