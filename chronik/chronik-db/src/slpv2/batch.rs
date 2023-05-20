use std::{borrow::Cow, collections::HashMap};

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_slp::slpv2;
use thiserror::Error;
use topo_sort::TopoSort;

use crate::{
    index_tx::IndexTx,
    io::TxNum,
    slpv2::{
        io::TokenNum,
        structs::{DbTxData, DbTxSection},
    },
};

pub struct BatchProcessor<'tx> {
    parsed_txs: HashMap<TxNum, (&'tx IndexTx<'tx>, Vec<slpv2::ParsedPushdata>)>,
    has_any_genesis: bool,
    valid: HashMap<TxNum, slpv2::TxData>,
}

pub struct BatchDbData {
    pub next_token_num: TokenNum,
    pub data: HashMap<TxNum, DbTxData>,
    pub token_ids: BiMap<TokenNum, slpv2::TokenId>,
}

pub struct BatchProcessResult {
    pub new_tokens: Vec<(TokenNum, slpv2::TokenMeta, slpv2::GenesisData)>,
    pub new_tx_data: Vec<(TxNum, DbTxData)>,
}

#[derive(Debug, Error, PartialEq)]
pub enum BatchError {
    #[error("Cycle in SLPv2 txs")]
    Cycle,

    #[error("Inconsistent BatchDbData: Missing TokenId for token_num {0}")]
    MissingTokenNum(TokenNum),

    #[error("Inconsistent BatchDbData: Missing TokenNum for token_id {0}")]
    MissingTokenId(slpv2::TokenId),
}

use self::BatchError::*;

impl<'tx> BatchProcessor<'tx> {
    pub fn prepare(txs: &'tx [IndexTx<'tx>]) -> Self {
        let parsed_txs = txs
            .iter()
            .filter_map(|tx| {
                let parsed = slpv2::parse_tx(&tx.tx).ok()?;
                if parsed.iter().any(|pushdata| {
                    matches!(pushdata, slpv2::ParsedPushdata::Error(_))
                }) {
                    return Some((tx.tx_num, (tx, parsed)));
                }
                None
            })
            .collect::<HashMap<_, _>>();
        let has_any_genesis = parsed_txs.values().any(|(_, data)| {
            data.iter().any(|pushdata| match pushdata {
                slpv2::ParsedPushdata::Section(section) => {
                    section.variant.section_type()
                        == slpv2::SectionType::GENESIS
                }
                _ => false,
            })
        });
        BatchProcessor {
            parsed_txs,
            has_any_genesis,
            valid: HashMap::new(),
        }
    }

    pub fn has_any_genesis(&self) -> bool {
        self.has_any_genesis
    }

    pub fn verify(
        &mut self,
        db_data: &mut BatchDbData,
    ) -> Result<BatchProcessResult> {
        let mut topo_sort = TopoSort::with_capacity(self.parsed_txs.len());
        for (&tx_num, &(tx, _)) in &self.parsed_txs {
            topo_sort.insert_from_slice(tx_num, &tx.input_nums);
        }
        let mut new_tokens = Vec::new();
        let mut new_tx_data = Vec::new();
        for tx_num in topo_sort.into_nodes() {
            let tx_num = tx_num.map_err(|_| Cycle)?;
            let (tx, parsed) = self.parsed_txs.remove(&tx_num).unwrap();
            let inputs = if tx.is_coinbase {
                vec![]
            } else {
                let mut inputs = Vec::with_capacity(tx.input_nums.len());
                for (&input_num, input) in
                    tx.input_nums.iter().zip(&tx.tx.inputs)
                {
                    inputs.push(self.token_output(
                        input_num,
                        input.prev_out.out_idx as usize,
                        db_data,
                    )?);
                }
                inputs
            };
            let tx_spec = slpv2::TxSpec::process_parsed_pushdata(parsed, tx.tx);
            let (tx_data, burns) = slpv2::verify(tx_spec.sections, tx_spec.outputs, &inputs);
            if tx_data.sections.is_empty() {
                continue;
            }
            if let Some((meta, genesis_data)) = tx_spec.genesis_data {
                db_data
                    .token_ids
                    .insert(db_data.next_token_num, meta.token_id);
                new_tokens.push((
                    db_data.next_token_num,
                    meta.clone(),
                    genesis_data,
                ));
                db_data.next_token_num += 1;
            }
            let mut db_sections = Vec::with_capacity(tx_data.sections.len());
            for section in &tx_data.sections {
                let token_num = *db_data
                    .token_ids
                    .get_by_right(&section.meta.token_id)
                    .ok_or(MissingTokenId(section.meta.token_id))?;
                let input_sum = tx_data
                    .inputs()
                    .filter_map(|input| {
                        let input = input?;
                        (input.token_id.as_ref() == &section.meta.token_id)
                            .then(|| input.amount)
                    })
                    .sum::<slpv2::Amount>();
                db_sections.push(DbTxSection {
                    token_num,
                    section_type: section.section_type,
                    required_input_sum: section.required_input_sum,
                    burn_amount: (input_sum - section.required_input_sum)
                        .max(0),
                });
            }
            let mut db_burn_token_nums =
                Vec::with_capacity(tx_data.burn_token_ids.len());
            for burn_token_id in &tx_data.burn_token_ids {
                db_burn_token_nums.push(
                    *db_data
                        .token_ids
                        .get_by_right(burn_token_id)
                        .ok_or(MissingTokenId(*burn_token_id))?,
                );
            }
            let db_tx_data = DbTxData {
                sections: db_sections,
                burn_token_nums: db_burn_token_nums,
                input_tokens: tx_data.inputs.clone(),
                output_tokens: tx_data.outputs.clone(),
            };
            new_tx_data.push((tx_num, db_tx_data));
            self.valid.insert(tx_num, tx_data);
        }
        Ok(BatchProcessResult {
            new_tokens,
            new_tx_data,
        })
    }

    fn token_output<'a>(
        &'a self,
        tx_num: TxNum,
        out_idx: usize,
        db_data: &BatchDbData,
    ) -> Result<Option<slpv2::Token<'a>>> {
        if let Some(tx_data) = self.valid.get(&tx_num) {
            return Ok(tx_data.outputs().nth(out_idx).flatten());
        }
        let db_tx_data = match db_data.data.get(&tx_num) {
            Some(db_tx_data) => db_tx_data,
            None => return Ok(None),
        };
        let db_output = match db_tx_data.output_tokens.get(out_idx) {
            Some(Some(db_output)) => db_output,
            _ => return Ok(None),
        };
        let token_num = db_tx_data.sections[db_output.section_idx].token_num;
        let token_id = match db_data.token_ids.get_by_left(&token_num) {
            Some(token_id) => token_id,
            None => return Err(BatchError::MissingTokenNum(token_num).into()),
        };
        Ok(Some(slpv2::Token {
            token_id: Cow::Owned(*token_id),
            amount: db_output.amount,
            is_mint_baton: db_output.is_mint_baton,
        }))
    }
}
