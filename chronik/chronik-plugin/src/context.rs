// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`PluginContext`].

use std::{collections::BTreeMap, marker::PhantomData};

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{OutPoint, Tx};
use bitcoinsuite_slp::{token_tx::TokenTx, verify::SpentToken};
use chronik_plugin_common::{
    config::PluginConfig,
    data::{PluginOutput, PluginTxOutputs},
};

/// Dummy plugin context that does nothing, used when the plugin system is
/// disabled
#[derive(Debug, Default)]
pub struct PluginContext;

/// Dummy Python handle
#[derive(Clone, Copy, Debug)]
pub struct Python<'py>(PhantomData<&'py ()>);

impl PluginContext {
    /// Fallback for the real PluginContext::setup that does nothing
    pub fn setup(_config: PluginConfig) -> Result<Self> {
        Ok(PluginContext)
    }

    /// Fallback for acquiring Python that always returns `R::default()`
    pub fn with_py<F, R>(&self, _f: F) -> Result<R>
    where
        F: for<'py> FnOnce(Python<'py>) -> Result<R>,
        R: Default,
    {
        Ok(R::default())
    }

    /// Run the tx by all the plugins and return their results.
    pub fn run_plugin_outputs(
        &self,
        _py: Python<'_>,
        _tx: &Tx,
        _plugin_outputs: &BTreeMap<OutPoint, PluginOutput>,
        _token_data: Option<(&TokenTx, &[Option<SpentToken>])>,
    ) -> Result<PluginTxOutputs> {
        Ok(Default::default())
    }

    /// Fallback that always returns `false`
    pub fn tx_matches_any_plugin(&self, _tx: &Tx) -> bool {
        false
    }
}
