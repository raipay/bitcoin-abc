// Copyright (c) 2022 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Structs storing the results of cryptographic hash functions.

/// Hash of the SHA-256 algorithm as defined by NIST.
///
/// Bitcoin usually uses the double-Sha256 (see [`Sha256d`]) for hashes,
/// mostly block hashes, tx hashes and txids.
#[derive(Debug)]
pub struct Sha256([u8; 32]);

/// SHA-256 algorithm applied twice (i.e. sha256(sha256(x))), see
/// [`Sha256`].
///
/// This is the most commonly used hash in Bitcoin, most prominently for
/// block hashes, tx hashes and txids.
#[derive(Debug)]
pub struct Sha256d([u8; 32]);
