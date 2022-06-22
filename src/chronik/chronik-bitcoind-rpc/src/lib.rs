use bitcoinsuite_error::{ErrorMeta, Result};
use chronik_interface::BitcoindInterface;
use thiserror::Error;

pub struct BitcoindRpc<'a, BI> {
    bitcoind_interface: &'a BI,
}

#[derive(Debug, Error, ErrorMeta)]
pub enum BitcoindRpcError {
    #[critical()]
    #[error("JSONRPC returned invalid JSON: {0}")]
    InvalidJson(String),

    #[critical()]
    #[error("JSONRPC returned json with missing {0:?} field")]
    MissingJsonField(&'static str),
}

use self::BitcoindRpcError::*;

impl<'a, BI: BitcoindInterface> BitcoindRpc<'a, BI> {
    pub fn new(bitcoind_interface: &'a BI) -> Self {
        BitcoindRpc { bitcoind_interface }
    }

    pub fn run_rpc_command(
        &self,
        command: &str,
        params: &[&str],
    ) -> Result<Result<json::JsonValue, String>> {
        let result = self.bitcoind_interface.run_rpc_command(command, params);
        match result {
            Ok(json_result) => Ok(Ok(Self::parse_json(&json_result)?)),
            Err(ex) => {
                let error = Self::parse_json(ex.what())?;
                return Ok(Err(error["message"]
                    .as_str()
                    .ok_or(MissingJsonField("message"))?
                    .to_string()));
            }
        }
    }

    pub fn test_mempool_accept(&self, raw_tx: &[u8]) -> Result<Result<(), String>> {
        let result = self.run_rpc_command(
            "testmempoolaccept",
            &[&format!("\"{}\"", hex::encode(raw_tx))],
        )?;
        match result {
            Ok(json_result) => {
                let tx_result = &json_result[0];
                if !tx_result["allowed"]
                    .as_bool()
                    .ok_or(MissingJsonField("allowed"))?
                {
                    return Ok(Err(tx_result["reject-reason"]
                        .as_str()
                        .ok_or(MissingJsonField("reject-reason"))?
                        .to_string()));
                }
                Ok(Ok(()))
            }
            Err(message) => Ok(Err(message)),
        }
    }

    fn parse_json(json: &str) -> Result<json::JsonValue> {
        Ok(json::parse(json).map_err(|err| InvalidJson(err.to_string()))?)
    }
}
