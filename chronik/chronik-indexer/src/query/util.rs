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
use bitcoinsuite_slp::{slp, slpv2};
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
    slp_tx_data: Option<Result<slp::TxData, String>>,
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
                    slp_token: slp_tx_data.as_ref().and_then(|tx_data| {
                        Some(make_slp_token_proto(
                            tx_data
                                .as_ref()
                                .ok()?
                                .input_tokens
                                .get(input_idx)?,
                        ))
                    }),
                    slp_burn: slp_tx_data.as_ref().and_then(|tx_data| {
                        let tx_data = tx_data.as_ref().ok()?;
                        let burn =
                            tx_data.slp_burns.get(input_idx)?.as_ref()?;
                        Some(proto::SlpBurn {
                            token: Some(make_slp_token_proto(&burn.token)),
                            token_id: burn.token_id.to_vec(),
                        })
                    }),
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
                slp_token: slp_tx_data.as_ref().and_then(|tx_data| {
                    Some(make_slp_token_proto(
                        tx_data.as_ref().ok()?.output_tokens.get(output_idx)?,
                    ))
                }),
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
        slp_tx_data: slp_tx_data
            .as_ref()
            .and_then(|slp| slp.as_ref().ok().map(make_slp_tx_data)),
        slp_error_msg: slp_tx_data
            .and_then(|msg| msg.err().map(|msg| msg.to_string()))
            .unwrap_or_default(),
        slpv2_sections: slpv2_sections(slpv2_tx_data),
        slpv2_errors: slpv2_errors(tx, slpv2_tx_data),
        slpv2_burn_token_ids: slpv2_burn_token_ids(slpv2_tx_data),
        network: proto::Network::Xec as _,
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

pub(crate) fn make_slp_meta(slp_tx_data: &slp::TxData) -> proto::SlpMeta {
    proto::SlpMeta {
        token_id: slp_tx_data.token_id.to_vec(),
        token_type: match slp_tx_data.token_type {
            slp::TokenType::Fungible => proto::SlpTokenType::Fungible,
            slp::TokenType::Nft1Group => proto::SlpTokenType::Nft1Group,
            slp::TokenType::Nft1Child => proto::SlpTokenType::Nft1Child,
            slp::TokenType::Unknown => proto::SlpTokenType::UnknownTokenType,
        } as _,
        tx_type: match slp_tx_data.tx_type {
            slp::TxTypeVariant::Genesis => proto::SlpTxType::Genesis,
            slp::TxTypeVariant::Send => proto::SlpTxType::Send,
            slp::TxTypeVariant::Mint => proto::SlpTxType::Mint,
            slp::TxTypeVariant::Burn => proto::SlpTxType::Burn,
            slp::TxTypeVariant::Unknown => proto::SlpTxType::UnknownTxType,
        } as _,
        group_token_id: match &slp_tx_data.group_token_id {
            Some(group_token_id) => group_token_id.to_vec(),
            None => vec![],
        },
    }
}

pub(crate) fn make_slp_tx_data(slp_tx_data: &slp::TxData) -> proto::SlpTxData {
    proto::SlpTxData {
        genesis_info: None,
        slp_meta: Some(make_slp_meta(slp_tx_data)),
    }
}

pub(crate) fn make_slp_token_proto(
    token_output: &slp::Token,
) -> proto::SlpToken {
    proto::SlpToken {
        amount: token_output.amount,
        is_mint_baton: token_output.is_mint_baton,
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
            .enumerate()
            .map(|(section_idx, section)| proto::Slpv2Section {
                token_id: section.meta.token_id.to_vec(),
                token_type: slpv2_token_type(section.meta.token_type) as _,
                section_type: slpv2_section_type(section.section_type) as _,
                burn_amount: {
                    let input_sum = slpv2_tx_data
                        .inputs
                        .iter()
                        .flatten()
                        .filter(|token_output| {
                            token_output.section_idx == section_idx
                        })
                        .map(|token_output| token_output.amount)
                        .sum::<slpv2::Amount>();
                    let output_sum = slpv2_tx_data
                        .outputs
                        .iter()
                        .flatten()
                        .filter(|token_output| {
                            token_output.section_idx == section_idx
                        })
                        .map(|token_output| token_output.amount)
                        .sum::<slpv2::Amount>();
                    (input_sum - output_sum).max(0)
                },
            })
            .collect(),
        None => vec![],
    }
}

fn slpv2_errors(tx: &Tx, slpv2_tx_data: Option<&slpv2::TxData>) -> Vec<String> {
    let parsed = match slpv2::parse_tx(tx) {
        Ok(parsed) => parsed,
        Err(err) => return vec![format!("[IGNORED] EMPP error: {err}")],
    };
    let tx_spec = slpv2::TxSpec::process_parsed_pushdata(parsed, tx);
    let actual_inputs = match slpv2_tx_data {
        Some(actual_tx_data) => actual_tx_data.inputs().collect::<Vec<_>>(),
        None => vec![None; tx.inputs.len()],
    };
    let (tx_data, verify_errs) =
        slpv2::verify(tx_spec.sections, tx_spec.outputs, &actual_inputs);

    let mut errors = Vec::new();
    for (pushdata_idx, parse_error) in tx_spec.parse_errors {
        let error = format!(
            "Parse error at pushdata index {pushdata_idx}: {parse_error}"
        );
        if parse_error.should_ignore() {
            errors.push(format!("[IGNORED] {error}"));
        } else {
            errors.push(error);
        }
    }
    for (pushdata_idx, process_error) in tx_spec.process_errors {
        errors.push(format!(
            "Process error at section index {}: {process_error}",
            tx_data.sections.len()
        ));
    }
    for verify_error in verify_errs {
        errors.push(verify_error.to_string());
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

pub fn validate_slp_tx(
    tx: &Tx,
    mempool: &Mempool,
    db: &Db,
) -> Result<Option<Result<slp::TxData, String>>> {
    let parsed = match slp::parse_tx(&tx) {
        Ok(parsed) => parsed,
        Err(err) if err.should_ignore() => return Ok(None),
        Err(err) => return Ok(Some(Err(err.to_string()))),
    };
    let mut tx_data_inputs =
        HashMap::<TxId, Option<Cow<'_, slp::TxData>>>::new();
    let mut actual_inputs = Vec::with_capacity(tx.inputs.len());
    for input in &tx.inputs {
        if let Entry::Vacant(entry) = tx_data_inputs.entry(input.prev_out.txid)
        {
            let tx_data =
                mempool.slp().tx_data_or_read(db, entry.key(), |txid| {
                    mempool.tx(txid).is_some()
                })?;
            entry.insert(tx_data);
        }
    }
    for input in &tx.inputs {
        let tx_spec_input = &tx_data_inputs[&input.prev_out.txid];
        actual_inputs.push(tx_spec_input.as_ref().and_then(|tx_data| {
            tx_data
                .output_tokens
                .get(input.prev_out.out_idx as usize)
                .map(|token| slp::SlpSpentOutput {
                    token_id: tx_data.token_id,
                    token_type: tx_data.token_type,
                    token: *token,
                    group_token_id: tx_data.group_token_id.clone(),
                })
        }));
    }
    match slp::validate(&parsed, &actual_inputs) {
        Ok(tx_data) => Ok(Some(Ok(tx_data))),
        Err(err) => Ok(Some(Err(err.to_string()))),
    }
}

pub fn validate_slpv2_tx(
    tx: &Tx,
    mempool: &Mempool,
    db: &Db,
) -> Result<Option<(slpv2::TxData, Vec<String>)>> {
    let parsed = match slpv2::parse_tx(tx) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };
    let tx_spec = slpv2::TxSpec::process_parsed_pushdata(parsed, tx);
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
    let (tx_data, verify_errs) =
        slpv2::verify(tx_spec.sections, tx_spec.outputs, &actual_inputs);
    let mut errors = Vec::new();
    for (_, parse_err) in &tx_spec.parse_errors {
        if !parse_err.should_ignore() {
            errors.push(parse_err.to_string());
        }
    }
    for (_, process_err) in &tx_spec.process_errors {
        errors.push(process_err.to_string());
    }
    for verify_err in &verify_errs {
        errors.push(verify_err.to_string());
    }
    let token_ids = tx_data.sections.iter().map(|section| section.meta.token_id).chain(tx_data.burn_token_ids.clone());
    for token_id in token_ids {
        let input_sum = tx_data
            .inputs()
            .flatten()
            .filter(|token_output| token_output.token_id.as_ref() == &token_id)
            .map(|token_output| token_output.amount)
            .sum::<slpv2::Amount>();
        let output_sum = tx_data
            .outputs()
            .flatten()
            .filter(|token_output| token_output.token_id.as_ref() == &token_id)
            .map(|token_output| token_output.amount)
            .sum::<slpv2::Amount>();
        let actual_burn = (input_sum - output_sum).max(0);
        if actual_burn == 0 {
            continue;
        }
        let intentional_burn_amount = tx_spec
            .intentional_burns
            .iter()
            .find(|burn| burn.token_id == token_id)
            .map(|burn| burn.amount)
            .unwrap_or_default();
        if intentional_burn_amount != actual_burn {
            errors.push(format!("Unintentionally burning {actual_burn} base tokens of token ID {token_id}, but intentionally burning {intentional_burn_amount} base tokens"));
        }
    }
    Ok(Some((tx_data, errors)))
}
