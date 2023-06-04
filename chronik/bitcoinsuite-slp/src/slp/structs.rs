use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::slp::{
    consts::{
        TOKEN_TYPE_V1, TOKEN_TYPE_V1_NFT1_CHILD, TOKEN_TYPE_V1_NFT1_GROUP,
    },
    TokenId, VerifyError,
};

pub type Amount = u64;

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
    Fungible,
    Nft1Group,
    Nft1Child,
    Unknown(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TxType {
    Genesis(GenesisInfo),
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

#[derive(
    Clone, Debug, Deserialize, PartialEq, Eq, Hash, Default, Serialize,
)]
pub struct GenesisInfo {
    pub token_ticker: Bytes,
    pub token_name: Bytes,
    pub token_document_url: Bytes,
    pub token_document_hash: Option<[u8; 32]>,
    pub decimals: u32,
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
    /// 0000...000000 if token_id is incomplete
    pub token_id: TokenId,
    pub token_type: TokenType,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TokenBurn {
    pub meta: TokenMeta,
    pub amount: u128,
    pub burn_mint_batons: bool,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
)]
pub enum Token {
    Amount(Amount),
    MintBaton,
    Unknown(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxData {
    pub meta: TokenMeta,
    pub tx_type: TxTypeVariant,
    pub output_tokens: Vec<Option<Token>>,
    pub group_token_id: Option<TokenId>,
    pub burns: Vec<TokenBurn>,
    pub error: Option<VerifyError>,
    pub genesis_info: Option<GenesisInfo>,
}

impl Token {
    pub fn amount(&self) -> Amount {
        match *self {
            Token::Amount(amount) => amount,
            Token::MintBaton => 0,
            Token::Unknown(_) => 0,
        }
    }

    pub fn is_mint_baton(&self) -> bool {
        *self == Token::MintBaton
    }
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

impl TokenMeta {
    pub fn fungible(token_id: TokenId) -> TokenMeta {
        TokenMeta {
            token_id,
            token_type: TokenType::Fungible,
        }
    }

    pub fn nft1_group(token_id: TokenId) -> TokenMeta {
        TokenMeta {
            token_id,
            token_type: TokenType::Nft1Group,
        }
    }

    pub fn nft1_child(token_id: TokenId) -> TokenMeta {
        TokenMeta {
            token_id,
            token_type: TokenType::Nft1Child,
        }
    }
}

impl TokenType {
    pub fn to_u16(&self) -> u16 {
        match *self {
            TokenType::Fungible => TOKEN_TYPE_V1[0].into(),
            TokenType::Nft1Group => TOKEN_TYPE_V1_NFT1_GROUP[0].into(),
            TokenType::Nft1Child => TOKEN_TYPE_V1_NFT1_CHILD[0].into(),
            TokenType::Unknown(token_type) => token_type,
        }
    }
}
