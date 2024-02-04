// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`PluginConfig`].

use std::path::PathBuf;

/// Config for the plugin system
#[derive(Debug, Default)]
pub struct PluginConfig {
    /// Directory where the plugins reside
    pub plugin_dir: PathBuf,
}
