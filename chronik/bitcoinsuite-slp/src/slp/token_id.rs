use bitcoinsuite_core::{hash::{Sha256d, Hashed}, error::DataError, tx::TxId};

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenId(Sha256d);

impl std::fmt::Debug for TokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TokenId({})", self.0.hex_be())
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

    pub fn from_txid(txid: TxId) -> Self {
        TokenId(Sha256d::from_le_bytes(txid.to_bytes()))
    }

    /// Returns the txid bytes in little-endian byte order.
    ///
    /// ```
    /// # use bitcoinsuite_core::{tx::TxId, hash::{Sha256d, Hashed}};
    /// # use hex_literal::hex;
    /// let hash = hex!(
    ///     "3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a"
    /// );
    /// let txid = TxId::from(Sha256d(hash));
    /// assert_eq!(txid.to_bytes(), hash);
    /// ```
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    /// Returns the txid as [`Vec<u8>`] in little-endian byte order.
    ///
    /// ```
    /// # use bitcoinsuite_core::{tx::TxId, hash::{Sha256d, Hashed}};
    /// # use hex_literal::hex;
    /// let hash = hex!(
    ///     "3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a"
    /// );
    /// let txid = TxId::from(Sha256d(hash));
    /// assert_eq!(txid.to_vec(), hash.to_vec());
    /// ```
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
