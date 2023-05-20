use std::ops::Range;

use bytes::Bytes;

use crate::slpv2::{self, TokenId, DEFAULT_TOKEN_TYPE};

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

#[derive(Debug, Default)]
pub struct Parsed {
    pub sections: Vec<Section>,
}

#[derive(Debug)]
pub struct ParseData {
    pub parsed: Parsed,
    pub first_err: Option<slpv2::ParseError>,
}

pub struct TokenAmount<'a> {
    pub token_id: &'a TokenId,
    pub amount: Amount,
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
