// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

use std::{
    borrow::Cow,
    collections::{hash_map::Entry, BTreeMap, HashMap},
    str::FromStr,
};

use abc_rust_error::{Report, Result};
use bitcoinsuite_core::{
    block::BlockHash,
    ser::BitcoinSer,
    tx::{OutPoint, SpentBy, Tx, TxId},
};
use bitcoinsuite_slp::slpv2::{self};
use chronik_db::{
    db::Db,
    io::{BlockHeight, DbBlock, SpentByEntry, SpentByReader, TxNum, TxReader},
    mem::Mempool,
};
use chronik_proto::proto;
use thiserror::Error;

use crate::avalanche::Avalanche;

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum QueryUtilError {
    /// DB contains a spent-by entry whose referenced tx_num cannot be found.
    #[error(
        "500: Inconsistent DB: tx_num = {tx_num} has spent entry {entry:?} in \
         the DB, but the tx cannot be not found in the index"
    )]
    SpendingTxNotFound {
        /// Which tx has an output spent by an unknown tx.
        tx_num: TxNum,
        /// The offending entry in the DB that references an unknown tx.
        entry: SpentByEntry,
    },

    /// Query is neither a hex hash nor an integer string
    #[error("400: Not a hash or height: {0}")]
    NotHashOrHeight(String),
}

use self::QueryUtilError::*;

/// Make a [`proto::Tx`].
pub(crate) fn make_tx_proto(
    tx: &Tx,
    outputs_spent: &OutputsSpent<'_>,
    time_first_seen: i64,
    is_coinbase: bool,
    block: Option<&DbBlock>,
    avalanche: &Avalanche,
    slpv2_tx_data: Option<&slpv2::TxData>,
) -> proto::Tx {
    proto::Tx {
        txid: tx.txid().to_vec(),
        version: tx.version,
        inputs: tx
            .inputs
            .iter()
            .enumerate()
            .map(|(input_idx, input)| {
                let coin = input.coin.as_ref();
                let (output_script, value) = coin
                    .map(|coin| {
                        (coin.output.script.to_vec(), coin.output.value)
                    })
                    .unwrap_or_default();
                proto::TxInput {
                    prev_out: Some(make_outpoint_proto(&input.prev_out)),
                    input_script: input.script.to_vec(),
                    output_script,
                    value,
                    sequence_no: input.sequence,
                    slpv2: slpv2_tx_data.and_then(|tx_data| {
                        Some(make_slpv2_token_proto(
                            tx_data,
                            tx_data.inputs[input_idx].as_ref()?,
                        ))
                    }),
                }
            })
            .collect(),
        outputs: tx
            .outputs
            .iter()
            .enumerate()
            .map(|(output_idx, output)| proto::TxOutput {
                value: output.value,
                output_script: output.script.to_vec(),
                spent_by: outputs_spent
                    .spent_by(output_idx as u32)
                    .map(|spent_by| make_spent_by_proto(&spent_by)),
                slpv2: slpv2_tx_data.and_then(|tx_data| {
                    Some(make_slpv2_token_proto(
                        tx_data,
                        tx_data.outputs[output_idx].as_ref()?,
                    ))
                }),
            })
            .collect(),
        lock_time: tx.locktime,
        block: block.map(|block| proto::BlockMetadata {
            hash: block.hash.to_vec(),
            height: block.height,
            timestamp: block.timestamp,
            is_final: avalanche.is_final_height(block.height),
        }),
        time_first_seen,
        size: tx.ser_len() as u32,
        is_coinbase,
        slpv2_sections: slpv2_sections(slpv2_tx_data),
        slpv2_errors: slpv2_errors(tx, slpv2_tx_data),
        slpv2_burn_token_ids: slpv2_burn_token_ids(slpv2_tx_data),
    }
}

pub(crate) fn make_outpoint_proto(outpoint: &OutPoint) -> proto::OutPoint {
    proto::OutPoint {
        txid: outpoint.txid.to_vec(),
        out_idx: outpoint.out_idx,
    }
}

fn make_spent_by_proto(spent_by: &SpentBy) -> proto::SpentBy {
    proto::SpentBy {
        txid: spent_by.txid.to_vec(),
        input_idx: spent_by.input_idx,
    }
}

pub(crate) fn make_slpv2_token_proto(
    tx_data: &slpv2::TxData,
    token_output: &slpv2::TokenOutputData,
) -> proto::Slpv2Token {
    proto::Slpv2Token {
        token_id: tx_data.token(token_output).token_id.to_vec(),
        section_idx: token_output.section_idx as u32,
        amount: token_output.amount,
        is_mint_baton: token_output.is_mint_baton,
    }
}

pub(crate) fn slpv2_token_type(
    token_type: slpv2::TokenType,
) -> proto::Slpv2TokenType {
    match token_type {
        slpv2::TokenType::Standard => proto::Slpv2TokenType::Standard,
    }
}

fn slpv2_section_type(
    section_type: slpv2::SectionType,
) -> proto::Slpv2SectionType {
    match section_type {
        slpv2::SectionType::GENESIS => proto::Slpv2SectionType::Slpv2Genesis,
        slpv2::SectionType::MINT => proto::Slpv2SectionType::Slpv2Mint,
        slpv2::SectionType::SEND => proto::Slpv2SectionType::Slpv2Send,
    }
}

fn slpv2_sections(
    slpv2_tx_data: Option<&slpv2::TxData>,
) -> Vec<proto::Slpv2Section> {
    match slpv2_tx_data {
        Some(slpv2_tx_data) => slpv2_tx_data
            .sections
            .iter()
            .map(|section| proto::Slpv2Section {
                token_id: section.meta.token_id.to_vec(),
                token_type: slpv2_token_type(section.meta.token_type) as _,
                section_type: slpv2_section_type(section.section_type) as _,
                intentional_burn_amount: section.intentional_burn_amount,
            })
            .collect(),
        None => vec![],
    }
}

fn slpv2_errors(tx: &Tx, slpv2_tx_data: Option<&slpv2::TxData>) -> Vec<String> {
    let parsed = slpv2::parse_tx(tx);
    let parse_error = parsed.first_err;
    let (mut tx_data, process_error) =
        slpv2::TxSpec::process_parsed(&parsed.parsed, tx);
    let actual_inputs = match slpv2_tx_data {
        Some(actual_tx_data) => actual_tx_data.inputs().collect::<Vec<_>>(),
        None => vec![None; tx.inputs.len()],
    };
    let mismatches = slpv2::verify(&mut tx_data, &actual_inputs);

    let mut errors = Vec::new();
    if let Some(parse_error) = parse_error {
        let error = format!(
            "Parse error at section index {}: {parse_error}",
            parsed.parsed.sections.len()
        );
        if parse_error.should_ignore() {
            errors.push(format!("[IGNORED] {error}"));
        } else {
            errors.push(error);
        }
    }
    if let Some(process_error) = process_error {
        errors.push(format!(
            "Process error at section index {}: {process_error}",
            tx_data.sections.len()
        ));
    }
    for mismatch in mismatches {
        errors.push(mismatch.to_string());
    }
    errors
}

fn slpv2_burn_token_ids(slpv2_tx_data: Option<&slpv2::TxData>) -> Vec<Vec<u8>> {
    match slpv2_tx_data {
        Some(tx_data) => tx_data
            .burn_token_ids
            .iter()
            .map(|token_id| token_id.to_vec())
            .collect(),
        None => vec![],
    }
}

pub(crate) enum HashOrHeight {
    Hash(BlockHash),
    Height(BlockHeight),
}

impl FromStr for HashOrHeight {
    type Err = Report;

    fn from_str(hash_or_height: &str) -> Result<Self> {
        if let Ok(hash) = hash_or_height.parse::<BlockHash>() {
            Ok(HashOrHeight::Hash(hash))
        } else {
            let height = match hash_or_height.parse::<BlockHeight>() {
                // disallow leading zeros
                Ok(0) if hash_or_height.len() == 1 => 0,
                Ok(height) if !hash_or_height.starts_with('0') => height,
                _ => {
                    return Err(
                        NotHashOrHeight(hash_or_height.to_string()).into()
                    );
                }
            };
            Ok(HashOrHeight::Height(height))
        }
    }
}

/// Helper struct for querying which tx outputs have been spent by DB or mempool
/// txs.
#[derive(Default)]
pub(crate) struct OutputsSpent<'a> {
    spent_by_mempool: Option<&'a BTreeMap<u32, SpentBy>>,
    spent_by_blocks: Vec<SpentByEntry>,
    txid_by_num: HashMap<TxNum, TxId>,
}

impl<'a> OutputsSpent<'a> {
    pub(crate) fn new_mempool(
        spent_by_mempool: Option<&'a BTreeMap<u32, SpentBy>>,
    ) -> Self {
        OutputsSpent {
            spent_by_mempool,
            spent_by_blocks: vec![],
            txid_by_num: HashMap::new(),
        }
    }

    pub(crate) fn query(
        spent_by_reader: &SpentByReader<'_>,
        tx_reader: &TxReader<'_>,
        spent_by_mempool: Option<&'a BTreeMap<u32, SpentBy>>,
        tx_num: TxNum,
    ) -> Result<Self> {
        let spent_by_blocks =
            spent_by_reader.by_tx_num(tx_num)?.unwrap_or_default();
        let mut txid_by_num = HashMap::<TxNum, TxId>::new();
        for spent_by in &spent_by_blocks {
            if let Entry::Vacant(entry) = txid_by_num.entry(spent_by.tx_num) {
                let txid = tx_reader
                    .txid_by_tx_num(spent_by.tx_num)?
                    .ok_or_else(|| SpendingTxNotFound {
                        tx_num,
                        entry: spent_by.clone(),
                    })?;
                entry.insert(txid);
            }
        }
        Ok(OutputsSpent {
            spent_by_mempool,
            spent_by_blocks,
            txid_by_num,
        })
    }

    pub(crate) fn spent_by(&self, output_idx: u32) -> Option<SpentBy> {
        if let Some(spent_by_mempool) = self.spent_by_mempool {
            if let Some(outpoint) = spent_by_mempool.get(&output_idx) {
                return Some(*outpoint);
            }
        }
        let search_idx = self
            .spent_by_blocks
            .binary_search_by_key(&output_idx, |entry| entry.out_idx);
        let entry = match search_idx {
            Ok(found_idx) => &self.spent_by_blocks[found_idx],
            Err(_) => return None,
        };
        let txid = self.txid_by_num.get(&entry.tx_num).unwrap();
        Some(SpentBy {
            txid: *txid,
            input_idx: entry.input_idx,
        })
    }
}

pub fn validate_slpv2_tx(
    tx: &Tx,
    mempool: &Mempool,
    db: &Db,
) -> Result<Option<slpv2::TxData>> {
    let parsed = slpv2::parse_tx(&tx);
    if parsed.parsed.sections.is_empty() {
        return Ok(None);
    }
    let (mut tx_spec, error) =
        slpv2::TxSpec::process_parsed(&parsed.parsed, &tx);
    let mut tx_data_inputs =
        HashMap::<TxId, Option<Cow<'_, slpv2::TxData>>>::new();
    let mut actual_inputs = Vec::with_capacity(tx.inputs.len());
    for input in &tx.inputs {
        if let Entry::Vacant(entry) = tx_data_inputs.entry(input.prev_out.txid)
        {
            let tx_data =
                mempool.slpv2().tx_data_or_read(db, entry.key(), |txid| {
                    mempool.tx(txid).is_some()
                })?;
            entry.insert(tx_data);
        }
    }
    for input in &tx.inputs {
        let tx_spec_input = &tx_data_inputs[&input.prev_out.txid];
        actual_inputs.push(
            tx_spec_input
                .as_ref()
                .and_then(|tx_spec| {
                    tx_spec.outputs().nth(input.prev_out.out_idx as usize)
                })
                .flatten(),
        );
    }
    slpv2::verify(&mut tx_spec, &actual_inputs);
    let tx_data = slpv2::TxData::from_spec_and_inputs(tx_spec, &actual_inputs);
    Ok(Some(tx_data))
}
