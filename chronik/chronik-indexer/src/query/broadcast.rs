use abc_rust_error::Result;
use bitcoinsuite_core::{
    error::DataError,
    ser::BitcoinSer,
    tx::{Tx, TxId, TxMut},
};
use bitcoinsuite_slp::{slp, slpv2};
use bytes::Bytes;
use chronik_bridge::ffi;
use chronik_db::{db::Db, mem::Mempool};
use thiserror::Error;

use crate::query::{validate_slp_tx, validate_slpv2_tx};

pub struct QueryBroadcast<'a> {
    /// Database
    pub db: &'a Db,
    /// Mempool
    pub mempool: &'a Mempool,
    pub bridge: &'a cxx::UniquePtr<ffi::ChronikBridge>,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum QueryBroadcastError {
    /// Transaction not in mempool nor DB.
    #[error("400: Parsing tx failed {0}")]
    ParsingFailed(DataError),

    #[error("400: Slp validation error {0}")]
    SlpValidationError(String),

    #[error("400: Slp burn {0:?}")]
    SlpBurn(Box<slp::Burn>),

    #[error("400: SLPv2 errors: {0:?}")]
    Slpv2Errors(Vec<String>),

    #[error("400: Broadcast failed: {0}")]
    BroadcastFailed(String),
}

use self::QueryBroadcastError::*;

impl QueryBroadcast<'_> {
    pub fn broadcast_txs(&self, raw_txs: Vec<Bytes>) -> Result<Vec<TxId>> {
        for mut raw_tx in raw_txs.iter().cloned() {
            let tx = TxMut::deser(&mut raw_tx).map_err(ParsingFailed)?;
            let tx = Tx::with_txid(TxId::default(), tx);

            let slp_result = validate_slp_tx(&tx, self.mempool, self.db)?;
            match slp_result {
                Some(Ok(tx_data)) => {
                    if let Some(burn) =
                        tx_data.slp_burns.into_iter().flatten().next()
                    {
                        return Err(SlpBurn(burn).into());
                    }
                }
                Some(Err(err)) => return Err(SlpValidationError(err).into()),
                None => {}
            }

            if let Some((_, errors)) =
                validate_slpv2_tx(&tx, self.mempool, self.db)?
            {
                if !errors.is_empty() {
                    return Err(Slpv2Errors(errors).into());
                }
            }
        }

        let mut txids = Vec::with_capacity(raw_txs.len());
        for raw_tx in raw_txs.iter() {
            txids.push(TxId::from(
                self.bridge
                    .broadcast_tx(raw_tx, 1000000)
                    .map_err(|err| BroadcastFailed(err.to_string()))?,
            ));
        }

        Ok(txids)
    }
}
