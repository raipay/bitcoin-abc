use bitcoinsuite_core::{
    error::DataError,
    hash::{Hashed, Sha256d},
    tx::TxId,
};
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Default,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub struct TokenId(Sha256d);

impl std::fmt::Debug for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slp::TokenId({})", self.0.hex_be())
    }
}

impl std::fmt::Display for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.hex_be().fmt(f)
    }
}

impl TokenId {
    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        TokenId(Sha256d::from_be_bytes(bytes))
    }

    pub const fn from_le_hash(hash: Sha256d) -> Self {
        TokenId(hash)
    }

    pub fn from_txid(txid: TxId) -> Self {
        TokenId(Sha256d::from_le_bytes(txid.to_bytes()))
    }

    /// Returns the token ID bytes in big-endian byte order.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    /// Returns the token ID bytes as [`Vec<u8>`] in big-endian byte order.
    pub fn to_vec(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
}

impl std::str::FromStr for TokenId {
    type Err = DataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TokenId(Sha256d::from_be_hex(s)?))
    }
}
