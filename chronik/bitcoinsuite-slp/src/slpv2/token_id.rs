use bitcoinsuite_core::{error::DataError, hash::Sha256d, tx::TxId};

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenId(TxId);

impl std::fmt::Debug for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slpv2::TokenId({})", self.0)
    }
}

impl std::fmt::Display for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TokenId {
    pub const fn new(txid: TxId) -> Self {
        TokenId(txid)
    }

    /// Returns the token ID bytes in little-endian byte order.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Returns a reference to the token ID bytes in little-endian byte order.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Returns the token ID bytes as [`Vec<u8>`] in little-endian byte order.
    pub fn to_vec(&self) -> Vec<u8> {
        self.to_bytes().to_vec()
    }
}

impl std::str::FromStr for TokenId {
    type Err = DataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TokenId(s.parse()?))
    }
}

impl TryFrom<&'_ [u8]> for TokenId {
    type Error = DataError;

    fn try_from(value: &'_ [u8]) -> Result<Self, Self::Error> {
        Ok(TokenId(value.try_into()?))
    }
}

impl From<[u8; 32]> for TokenId {
    fn from(array: [u8; 32]) -> Self {
        TokenId(TxId::from(array))
    }
}

impl From<Sha256d> for TokenId {
    fn from(hash: Sha256d) -> Self {
        TokenId(TxId::from(hash))
    }
}

impl From<TxId> for TokenId {
    fn from(txid: TxId) -> Self {
        TokenId(txid)
    }
}

impl AsRef<[u8]> for TokenId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
