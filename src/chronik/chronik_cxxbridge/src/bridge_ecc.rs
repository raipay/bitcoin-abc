use bitcoinsuite_core::{
    ecc::{Ecc, EccError, PubKey, SecKey, VerifySignatureError, PUBKEY_LENGTH},
    ByteArray, Bytes,
};
use crate::ffi;

pub struct BridgeEcc;

impl Ecc for BridgeEcc {
    fn pubkey_from_array(&self, pubkey: [u8; PUBKEY_LENGTH]) -> Result<PubKey, EccError> {
        Ok(PubKey::new_unchecked(pubkey))
    }

    fn seckey_from_array(&self, _seckey: [u8; 32]) -> Result<SecKey, EccError> {
        unimplemented!("Not used in Chronik")
    }

    fn sign(&self, _seckey: &SecKey, _msg: ByteArray<32>) -> Bytes {
        unimplemented!("Not used in Chronik")
    }

    fn schnorr_sign(&self, _seckey: &SecKey, _msg: ByteArray<32>) -> Bytes {
        unimplemented!("Not used in Chronik")
    }

    fn verify(
        &self,
        _pubkey: &PubKey,
        _msg: ByteArray<32>,
        _sig: &Bytes,
    ) -> Result<(), VerifySignatureError> {
        unimplemented!("Not used in Chronik")
    }

    fn schnorr_verify(
        &self,
        _pubkey: &PubKey,
        _msg: ByteArray<32>,
        _sig: &Bytes,
    ) -> Result<(), VerifySignatureError> {
        unimplemented!("Not used in Chronik")
    }

    fn derive_pubkey(&self, _seckey: &SecKey) -> PubKey {
        unimplemented!("Not used in Chronik")
    }

    fn serialize_pubkey_uncompressed(&self, pubkey: &PubKey) -> [u8; 65] {
        ffi::serialize_pubkey_uncompressed(pubkey.array())
    }

    fn normalize_sig(&self, _sig: &Bytes) -> Result<Bytes, EccError> {
        unimplemented!("Not used in Chronik")
    }
}
