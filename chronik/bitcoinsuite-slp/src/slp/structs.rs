use bytes::Bytes;

use crate::slp::TokenId;

pub type Amount = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    Fungible,
    Nft1Group,
    Nft1Child,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TxType {
    Genesis(Box<GenesisInfo>),
    Send,
    Mint,
    Burn(u64),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TxTypeVariant {
    Genesis,
    Send,
    Mint,
    Burn,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct GenesisInfo {
    pub token_ticker: Bytes,
    pub token_name: Bytes,
    pub token_document_url: Bytes,
    pub token_document_hash: Option<[u8; 32]>,
    pub decimals: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenMeta {
    pub token_id: TokenId,
    pub token_type: TokenType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Burn {
    pub token: Token,
    pub token_id: TokenId,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token {
    pub amount: Amount,
    pub is_mint_baton: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxData {
    pub input_tokens: Vec<Token>,
    pub output_tokens: Vec<Token>,
    pub slp_burns: Vec<Option<Box<Burn>>>,
    pub token_type: TokenType,
    pub tx_type: TxTypeVariant,
    /// 0000...000000 if token_id is incomplete
    pub token_id: TokenId,
    pub group_token_id: Option<Box<TokenId>>,
}

impl TxType {
    pub fn type_str(&self) -> &'static str {
        match self {
            TxType::Genesis(_) => "GENESIS",
            TxType::Mint => "MINT",
            TxType::Send => "SEND",
            TxType::Burn(_) => "BURN",
            TxType::Unknown => "UNKNOWN",
        }
    }
}

impl Token {
    pub const MINT_BATON: Token = Token {
        amount: 0,
        is_mint_baton: true,
    };

    pub const EMPTY: Token = Token {
        amount: 0,
        is_mint_baton: false,
    };

    pub const fn amount(amount: Amount) -> Self {
        Token {
            amount,
            is_mint_baton: false,
        }
    }
}

impl TxType {
    pub fn tx_type_variant(&self) -> TxTypeVariant {
        match &self {
            TxType::Genesis(_) => TxTypeVariant::Genesis,
            TxType::Send => TxTypeVariant::Send,
            TxType::Mint => TxTypeVariant::Mint,
            TxType::Burn(_) => TxTypeVariant::Burn,
            TxType::Unknown => TxTypeVariant::Unknown,
        }
    }
}
