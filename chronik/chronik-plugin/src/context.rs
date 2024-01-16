// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module for [`PluginContext`].

use std::{
    collections::{btree_map::Entry, BTreeMap},
    io::Read,
    path::{Path, PathBuf},
};

use abc_rust_error::Result;
use bitcoinsuite_core::{
    script::{opcode::OP_RETURN, Op},
    tx::Tx,
};
use bitcoinsuite_slp::{
    empp, structs::LokadId, token_tx::TokenTx, verify::SpentToken,
};
use chronik_util::log;
use pyo3::{
    types::{PyDict, PyList, PyModule},
    PyAny, PyErr, PyObject, PyResult, Python,
};

use crate::{module::load_plugin_module, tx::TxModule};

/// Config for the plugin system
#[derive(Debug)]
pub struct PluginConfig {
    /// Directory where the plugins reside
    pub plugin_dir: PathBuf,
}

/// Struct managing all things relating to Chronik plugins.
#[derive(Debug)]
pub struct PluginContext {
    plugins: Vec<Plugin>,
    lokad_id_indices: BTreeMap<LokadId, Vec<usize>>,
    tx_module: Option<TxModule>,
}

/// Individual handle on a plugin
#[derive(Debug)]
pub struct Plugin {
    /// Name of the plugin
    pub name: String,
    /// __version__ of the plugin
    pub version: String,
    /// __lokad_id__ of the plugin
    pub lokad_ids: Vec<LokadId>,
    fn_plugin_outputs: PyObject,
}

/// Result of running a tx agains all the plugins
#[derive(Debug, Default)]
pub struct PluginResult<'ctx, 'py> {
    /// Data assigned to outputs by plugins
    pub outputs: BTreeMap<usize, BTreeMap<&'ctx str, PluginResultEntry<'py>>>,
    /// Exceptions raised by plugins
    pub exceptions: BTreeMap<&'ctx str, PyErr>,
}

/// Data assigned to an output by an individual plugin
#[derive(Debug, Default)]
pub struct PluginResultEntry<'py> {
    /// Groups assigned to a plugin
    pub groups: Vec<&'py [u8]>,
    /// Data assigned to a plugin
    pub data: Vec<&'py [u8]>,
}

impl PluginContext {
    /// Setup a plugin context, i.e. setting up an embedded Python instance and
    /// loading plugins.
    pub fn setup(conf: PluginConfig) -> Result<Self> {
        if !conf.plugin_dir.exists() {
            log!(
                "Plugin dir {} doesn't exist, skipping\n",
                conf.plugin_dir.to_string_lossy(),
            );
            return Ok(PluginContext {
                plugins: vec![],
                lokad_id_indices: BTreeMap::new(),
                tx_module: None,
            });
        }

        load_plugin_module();
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<_> {
            let module = PyModule::from_code(
                py,
                "
import platform
version = platform.python_version()
                ",
                "get_version.py",
                "get_version",
            )?;
            let version = module.getattr("version")?;
            let version = version.extract::<String>()?;
            log!("Plugin context initialized Python {}\n", version);

            let mut ctx = PluginContext {
                plugins: vec![],
                lokad_id_indices: BTreeMap::new(),
                tx_module: Some(TxModule::import(py)?),
            };
            for plugin_dir_entry in std::fs::read_dir(conf.plugin_dir)? {
                let plugin_dir_entry = plugin_dir_entry?;
                let plugin_file_name = plugin_dir_entry.file_name();
                let plugin_file_name = plugin_file_name.to_string_lossy();
                let Some(plugin_module_name) =
                    plugin_file_name.strip_suffix(".py")
                else {
                    continue;
                };
                ctx.load_plugin(
                    py,
                    &plugin_dir_entry.path(),
                    &plugin_file_name,
                    plugin_module_name,
                )?;
            }

            Ok(ctx)
        })
    }

    fn load_plugin(
        &mut self,
        py: Python<'_>,
        path: &Path,
        file_name: &str,
        module_name: &str,
    ) -> Result<()> {
        let mut plugin_file = std::fs::File::open(path)?;
        let mut plugin_code = String::new();
        plugin_file.read_to_string(&mut plugin_code)?;
        let module =
            PyModule::from_code(py, &plugin_code, file_name, module_name)?;

        let Ok(py_lokad_id) = module.getattr("__lokad_id__") else {
            log!(
                "Warning: Plugin {module_name} has no __lokad_id__ set, \
                 skipping\n"
            );
            return Ok(());
        };

        let Ok(py_version) = module.getattr("__version__") else {
            log!(
                "Warning: Plugin {module_name} has no __version__ set, \
                 skipping\n"
            );
            return Ok(());
        };
        let Ok(version) = py_version.extract::<String>() else {
            log!(
                "Warning: Plugin {module_name} __version__ must be str, got \
                 {py_version}, skipping\n"
            );
            return Ok(());
        };

        // __LOKAD_ID__ can either be bytes or List[bytes]
        let lokad_ids =
            extract_bytes_or_list(py_lokad_id)
                .ok()
                .and_then(|lokad_ids| {
                    lokad_ids
                        .into_iter()
                        .map(LokadId::try_from)
                        .collect::<Result<Vec<_>, _>>()
                        .ok()
                });
        let Some(lokad_ids) = lokad_ids else {
            log!(
                "Warning: Plugin {module_name} has invalid __lokad_id__, must \
                 be 4 bytes\n"
            );
            return Ok(());
        };

        log!("Plugin {} has lokad IDs {}\n", module_name, py_lokad_id);

        let Ok(fn_plugin_outputs) = module.getattr("__plugin_outputs__") else {
            log!(
                "Warning: Plugin {module_name} has no __plugin_outputs__ \
                 function defined, skipping\n"
            );
            return Ok(());
        };

        let plugin_idx = self.plugins.len();
        self.plugins.push(Plugin {
            name: module_name.to_string(),
            version,
            lokad_ids: lokad_ids.clone(),
            fn_plugin_outputs: fn_plugin_outputs.into(),
        });
        for lokad_id in lokad_ids {
            self.lokad_id_indices
                .entry(lokad_id)
                .or_default()
                .push(plugin_idx);
        }

        Ok(())
    }

    /// Run the tx by all the plugins and return their results.
    pub fn run_plugin_outputs<'ctx, 'py>(
        &'ctx self,
        py: Python<'py>,
        tx: &Tx,
        token_data: Option<(&TokenTx, &[Option<SpentToken>])>,
    ) -> Result<PluginResult<'ctx, 'py>> {
        let mut result = PluginResult::default();
        let Some(tx_module) = self.tx_module.as_ref() else {
            return Ok(result);
        };
        let py_tx = tx_module.bridge_tx(py, tx, token_data)?;

        // TODO: filter tx again by plugin LOKAD ID
        for plugin in &self.plugins {
            let plugin_call_result =
                plugin.fn_plugin_outputs.call1(py, (py_tx.clone_ref(py),));
            let outputs = match plugin_call_result {
                Ok(outputs) => outputs.into_ref(py),
                Err(err) => {
                    result.exceptions.insert(&plugin.name, err);
                    continue;
                }
            };
            let Ok(outputs) = outputs.downcast::<PyList>() else {
                log!(
                    "Plugin {} returned unexpected result {outputs}, \
                     skipping\n",
                    plugin.name
                );
                continue;
            };
            for (idx, output) in outputs.iter().enumerate() {
                let Ok(output) = output.downcast::<PyDict>() else {
                    log!(
                        "Plugin {} returned unexpected list entry at index \
                         {idx}: {output}, skipping\n",
                        plugin.name
                    );
                    continue;
                };
                let Some(output_idx) = output.get_item("idx")? else {
                    log!(
                        "Plugin {} returned dict with missing \"idx\" at \
                         index {idx}, skipping\n",
                        plugin.name
                    );
                    continue;
                };
                let Ok(output_idx) = output_idx.extract::<usize>() else {
                    log!(
                        "Plugin {} returned dict with invalid \"idx\" at \
                         index {idx}: {output_idx}, expected int, skipping\n",
                        plugin.name
                    );
                    continue;
                };
                if output_idx >= tx.outputs.len() {
                    log!(
                        "Plugin {} returned dict with out-of-bounds \"idx\" \
                         at index {idx}: {output_idx} >= {}, skipping\n",
                        plugin.name,
                        tx.outputs.len()
                    );
                    continue;
                }
                let Some(groups) = output.get_item("group")? else {
                    log!(
                        "Plugin {} returned dict with missing \"group\" at \
                         index {idx}, skipping\n",
                        plugin.name
                    );
                    continue;
                };
                let Ok(groups) = extract_bytes_or_list(groups) else {
                    log!(
                        "Plugin {} returned dict with invalid \"group\" at \
                         index {idx}: {groups}, expected bytes or iterable of \
                         bytes, skipping\n",
                        plugin.name
                    );
                    continue;
                };
                let mut data = vec![];
                if let Some(py_data) = output.get_item("data")? {
                    let Ok(py_data) = extract_bytes_or_list(py_data) else {
                        log!(
                            "Plugin {} returned dict with invalid \"data\" at \
                             index {idx}: {py_data}, expected bytes or \
                             iterable of bytes, skipping\n",
                            plugin.name
                        );
                        continue;
                    };
                    data = py_data;
                }

                let entry = result
                    .outputs
                    .entry(output_idx)
                    .or_insert(BTreeMap::new())
                    .entry(plugin.name.as_str());
                match entry {
                    Entry::Occupied(_) => {
                        log!(
                            "Plugin {} returned a duplicate entry for output \
                             index {output_idx}, skipping\n",
                            plugin.name
                        );
                        continue;
                    }
                    Entry::Vacant(vacant) => {
                        vacant.insert(PluginResultEntry { groups, data });
                    }
                }
            }
        }

        Ok(result)
    }

    /// Fast pre-filter whether the tx matches any of the LOKAD IDs of any
    /// plugin.
    pub fn tx_matches_any_plugin(&self, tx: &Tx) -> bool {
        // OP_RETURN <LOKAD_ID> ...
        if let Some(lokad_id) = self.tx_opreturn_lokad_id(tx) {
            if self.lokad_id_indices.contains_key(&lokad_id) {
                return true;
            }
        }

        // eMPP: [<LOKAD ID>...]*
        if self.tx_matches_empp_lokad_ids(tx) {
            return true;
        }

        // script sig: <LOKAD ID> ...
        if self.tx_matches_input_lokad_ids(tx) {
            return true;
        }

        false
    }

    fn tx_opreturn_lokad_id(&self, tx: &Tx) -> Option<LokadId> {
        let first_output = tx.outputs.first()?;
        let mut ops = first_output.script.iter_ops();
        let op_return = ops.next()?.ok()?;
        if op_return != Op::Code(OP_RETURN) {
            return None;
        }
        let Op::Push(_, lokad_id) = ops.next()?.ok()? else {
            return None;
        };
        LokadId::try_from(lokad_id.as_ref()).ok()
    }

    fn tx_matches_empp_lokad_ids(&self, tx: &Tx) -> bool {
        let Some(first_output) = tx.outputs.first() else {
            return false;
        };
        let Ok(Some(empp_data)) = empp::parse(&first_output.script) else {
            return false;
        };
        empp_data
            .iter()
            .filter_map(|pushdata| LokadId::try_from(pushdata.get(..4)?).ok())
            .any(|lokad_id| self.lokad_id_indices.contains_key(&lokad_id))
    }

    fn tx_matches_input_lokad_ids(&self, tx: &Tx) -> bool {
        tx.inputs
            .iter()
            .filter_map(|input| {
                let mut ops = input.script.iter_ops();
                match ops.next()?.ok()? {
                    Op::Push(_, lokad_id) => {
                        LokadId::try_from(lokad_id.as_ref()).ok()
                    }
                    _ => None,
                }
            })
            .any(|lokad_id| self.lokad_id_indices.contains_key(&lokad_id))
    }
}

fn extract_bytes_or_list(py_any: &PyAny) -> PyResult<Vec<&[u8]>> {
    py_any
        .extract::<&[u8]>()
        .map(|bytes| vec![bytes])
        .or_else(|_| py_any.extract::<Vec<&[u8]>>())
}
