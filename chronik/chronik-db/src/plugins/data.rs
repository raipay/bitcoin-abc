//! Module for common structs used when indexing by plugins

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Data attached to an output by all loaded plugins.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginOutput {
    /// Entries for each plugin, indentified by plugin name
    pub plugins: BTreeMap<String, PluginOutputEntry>,
}

/// Data attached to an output by an individual plugin.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginOutputEntry {
    /// Groups assigned to the output
    pub groups: Vec<Vec<u8>>,
    /// Data assigned to the output
    pub data: Vec<Vec<u8>>,
}
