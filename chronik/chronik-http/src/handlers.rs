//! Module for Chronik handlers.

use std::{collections::HashMap, fmt::Display, str::FromStr};

use abc_rust_error::{Report, Result};
use chronik_indexer::indexer::ChronikIndexer;
use chronik_proto::proto;
use hyper::Uri;
use thiserror::Error;

#[cfg(feature = "plugins")]
use crate::parse::parse_hex;
use crate::{error::ReportError, parse::parse_script_variant_hex};

/// Errors for HTTP handlers.
#[derive(Debug, Error, PartialEq)]
pub enum ChronikHandlerError {
    /// Not found
    #[error("404: Not found: {0}")]
    RouteNotFound(Uri),

    /// Query parameter has an invalid value
    #[error("400: Invalid param {param_name}: {param_value}, {msg}")]
    InvalidParam {
        /// Name of the param
        param_name: String,
        /// Value of the param
        param_value: String,
        /// Human-readable error message.
        msg: String,
    },
}

use self::ChronikHandlerError::*;

fn get_param<T: FromStr>(
    params: &HashMap<String, String>,
    param_name: &str,
) -> Result<Option<T>>
where
    T::Err: Display,
{
    let Some(param) = params.get(param_name) else {
        return Ok(None);
    };
    Ok(Some(param.parse::<T>().map_err(|err| InvalidParam {
        param_name: param_name.to_string(),
        param_value: param.to_string(),
        msg: err.to_string(),
    })?))
}

/// Fallback route that returns a 404 response
pub async fn handle_not_found(uri: Uri) -> Result<(), ReportError> {
    Err(Report::from(RouteNotFound(uri)).into())
}

/// Return a page of the txs of a block.
pub async fn handle_block_txs(
    hash_or_height: String,
    query_params: &HashMap<String, String>,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let blocks = indexer.blocks();
    let page_num: u32 = get_param(query_params, "page")?.unwrap_or(0);
    let page_size: u32 = get_param(query_params, "page_size")?.unwrap_or(25);
    blocks.block_txs(hash_or_height, page_num as usize, page_size as usize)
}

/// Return a page of the confirmed txs of the given script.
/// Scripts are identified by script_type and payload.
pub async fn handle_script_confirmed_txs(
    script_type: &str,
    payload: &str,
    query_params: &HashMap<String, String>,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let script_variant = parse_script_variant_hex(script_type, payload)?;
    let script_history = indexer.script_history()?;
    let page_num: u32 = get_param(query_params, "page")?.unwrap_or(0);
    let page_size: u32 = get_param(query_params, "page_size")?.unwrap_or(25);
    let script = script_variant.to_script();
    script_history.confirmed_txs(&script, page_num as usize, page_size as usize)
}

/// Return a page of the tx history of the given script, in reverse
/// chronological order, i.e. the latest transaction first and then going back
/// in time. Scripts are identified by script_type and payload.
pub async fn handle_script_history(
    script_type: &str,
    payload: &str,
    query_params: &HashMap<String, String>,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let script_variant = parse_script_variant_hex(script_type, payload)?;
    let script_history = indexer.script_history()?;
    let page_num: u32 = get_param(query_params, "page")?.unwrap_or(0);
    let page_size: u32 = get_param(query_params, "page_size")?.unwrap_or(25);
    let script = script_variant.to_script();
    script_history.rev_history(&script, page_num as usize, page_size as usize)
}

/// Return a page of the confirmed txs of the given script.
/// Scripts are identified by script_type and payload.
pub async fn handle_script_unconfirmed_txs(
    script_type: &str,
    payload: &str,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let script_variant = parse_script_variant_hex(script_type, payload)?;
    let script_history = indexer.script_history()?;
    let script = script_variant.to_script();
    script_history.unconfirmed_txs(&script)
}

/// Return the UTXOs of the given script.
/// Scripts are identified by script_type and payload.
pub async fn handle_script_utxos(
    script_type: &str,
    payload: &str,
    indexer: &ChronikIndexer,
) -> Result<proto::ScriptUtxos> {
    let script_variant = parse_script_variant_hex(script_type, payload)?;
    let script_utxos = indexer.script_utxos()?;
    let script = script_variant.to_script();
    let utxos = script_utxos.utxos(&script)?;
    Ok(proto::ScriptUtxos {
        script: script.bytecode().to_vec(),
        utxos,
    })
}

/// Return a page of the confirmed txs of the given script.
/// Scripts are identified by script_type and payload.
#[cfg(feature = "plugins")]
pub async fn handle_plugin_confirmed_txs(
    plugin_name: &str,
    payload: &str,
    query_params: &HashMap<String, String>,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let plugin = indexer.plugins()?;
    let payload = parse_hex(payload)?;
    let page_num: u32 = get_param(query_params, "page")?.unwrap_or(0);
    let page_size: u32 = get_param(query_params, "page_size")?.unwrap_or(25);
    plugin.confirmed_txs(
        plugin_name,
        &payload,
        page_num as usize,
        page_size as usize,
    )
}

/// Return a page of the tx history of the given script, in reverse
/// chronological order, i.e. the latest transaction first and then going back
/// in time. Scripts are identified by script_type and payload.
#[cfg(feature = "plugins")]
pub async fn handle_plugin_history(
    plugin_name: &str,
    payload: &str,
    query_params: &HashMap<String, String>,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let payload = parse_hex(payload)?;
    let plugin = indexer.plugins()?;
    let page_num: u32 = get_param(query_params, "page")?.unwrap_or(0);
    let page_size: u32 = get_param(query_params, "page_size")?.unwrap_or(25);
    plugin.rev_history(
        plugin_name,
        &payload,
        page_num as usize,
        page_size as usize,
    )
}

/// Return a page of the confirmed txs of the given script.
/// Scripts are identified by script_type and payload.
#[cfg(feature = "plugins")]
pub async fn handle_plugin_unconfirmed_txs(
    plugin_name: &str,
    payload: &str,
    indexer: &ChronikIndexer,
) -> Result<proto::TxHistoryPage> {
    let payload = parse_hex(payload)?;
    let plugin = indexer.plugins()?;
    plugin.unconfirmed_txs(plugin_name, &payload)
}

/// Return the UTXOs of the given script.
/// Scripts are identified by script_type and payload.
#[cfg(feature = "plugins")]
pub async fn handle_plugin_utxos(
    plugin_name: &str,
    payload: &str,
    indexer: &ChronikIndexer,
) -> Result<proto::Utxos> {
    let payload = parse_hex(payload)?;
    let plugin = indexer.plugins()?;
    let utxos = plugin.utxos(plugin_name, &payload)?;
    Ok(proto::Utxos { utxos })
}
