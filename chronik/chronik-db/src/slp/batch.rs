use std::collections::{BTreeSet, HashMap};

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_slp::{
    slp::{self, SlpSpentOutput},
    slpv2,
};
use itertools::{Either, Itertools};
use thiserror::Error;
use topo_sort::TopoSort;

use crate::{
    index_tx::IndexTx,
    io::TxNum,
    slp::{
        data::{
            DbToken, DbTxData, EitherMeta, EitherToken, EitherTxData, Protocol,
            FLAGS_HAS_GROUP_TOKEN_ID,
        },
        io::TokenNum,
    },
};

pub struct ParsedTx<'tx> {
    tx: &'tx IndexTx<'tx>,
    parsed: Protocol<slp::ParseData, slpv2::ColoredTx>,
}

pub struct BatchProcessor<'tx> {
    parsed_txs: HashMap<TxNum, ParsedTx<'tx>>,
    non_slp_txs: Vec<&'tx IndexTx<'tx>>,
    has_any_genesis: bool,
    valid: HashMap<TxNum, EitherTxData>,
}

pub struct BatchDbData {
    pub next_token_num: TokenNum,
    pub data: HashMap<TxNum, DbTxData>,
    pub token_metas: BiMap<TokenNum, EitherMeta>,
}

pub type NewToken = Protocol<
    (slp::TokenMeta, slp::GenesisInfo),
    (slpv2::TokenMeta, slpv2::GenesisInfo),
>;

pub struct BatchProcessResult {
    pub new_tokens: Vec<(TxNum, TokenNum, NewToken)>,
    pub new_tx_data: HashMap<TxNum, DbTxData>,
}

struct ProcessedTx {
    new_token: Option<NewToken>,
    outputs: Vec<Option<EitherToken>>,
    group_token_meta: Option<slp::TokenMeta>,
    tx_data: Option<EitherTxData>,
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
        let (parsed_txs, non_slp_txs): (HashMap<_, _>, Vec<_>) =
            txs.iter().partition_map(|tx| {
                let parsed = if let Ok(slp) = slp::parse_tx(tx.tx) {
                    Protocol::Slp(slp)
                } else {
                    let colored_tx = slpv2::ColoredTx::parse_tx(tx.tx);
                    if colored_tx.sections.is_empty() {
                        return Either::Right(tx);
                    }
                    Protocol::Slpv2(colored_tx)
                };
                Either::Left((tx.tx_num, ParsedTx { tx, parsed }))
            });
        let has_any_genesis =
            parsed_txs.values().any(|tx| tx.parsed.is_genesis());
        BatchProcessor {
            parsed_txs,
            non_slp_txs,
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
        for (&tx_num, batch_tx) in &self.parsed_txs {
            topo_sort.insert_from_slice(tx_num, &batch_tx.tx.input_nums);
        }
        let mut new_tokens: Vec<(TxNum, TokenNum, NewToken)> = Vec::new();
        let mut new_tx_data: HashMap<u64, DbTxData> = HashMap::new();
        for tx_num in topo_sort.into_nodes() {
            let tx_num = tx_num.map_err(|_| Cycle)?;
            let parsed_tx = self.parsed_txs.remove(&tx_num).unwrap();
            let inputs = self.tx_token_inputs(parsed_tx.tx, db_data)?;
            let has_any_tokens = inputs.iter().any(|input| input.is_some());

            let processed_tx = match parsed_tx.parsed {
                Protocol::Slp(parsed) => {
                    let actual_inputs = inputs
                        .iter()
                        .map(|input| match input {
                            Some(Protocol::Slp(token)) => Some(token.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let tx_data = slp::verify(&parsed, &actual_inputs);
                    let has_any_outputs = tx_data
                        .output_tokens
                        .iter()
                        .any(|token| token.is_some());
                    if !has_any_outputs && !has_any_tokens {
                        continue;
                    }
                    ProcessedTx {
                        new_token: match parsed.tx_type {
                            slp::TxType::Genesis(genesis_info) => {
                                Some(Protocol::Slp((parsed.meta, genesis_info)))
                            }
                            _ => None,
                        },
                        outputs: tx_data
                            .output_tokens
                            .iter()
                            .map(|token| {
                                token.as_ref().map(|&token| {
                                    Protocol::Slp(SlpSpentOutput {
                                        meta: tx_data.meta,
                                        token,
                                        group_token_id: tx_data.group_token_id,
                                    })
                                })
                            })
                            .collect::<Vec<_>>(),
                        group_token_meta: tx_data.group_token_id.map(
                            |token_id| slp::TokenMeta {
                                token_id,
                                token_type: slp::TokenType::Nft1Group,
                            },
                        ),
                        tx_data: Some(Protocol::Slp(tx_data)),
                    }
                }
                Protocol::Slpv2(colored_tx) => {
                    let actual_inputs = inputs
                        .iter()
                        .map(|input| match input {
                            Some(Protocol::Slpv2(token)) => Some(token.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let tx_data = slpv2::verify(colored_tx, &actual_inputs);
                    if tx_data.sections.is_empty() && !has_any_tokens {
                        continue;
                    }
                    ProcessedTx {
                        new_token: tx_data.sections.get(0).and_then(
                            |section| {
                                Some(Protocol::Slpv2((
                                    section.meta,
                                    section.genesis_info.clone()?,
                                )))
                            },
                        ),
                        outputs: tx_data
                            .outputs()
                            .map(|output| output.map(Protocol::Slpv2))
                            .collect(),
                        group_token_meta: None,
                        tx_data: Some(Protocol::Slpv2(tx_data)),
                    }
                }
            };

            let has_genesis = processed_tx.new_token.is_some();
            if let Some(new_token) = processed_tx.new_token {
                match new_token {
                    Protocol::Slp((meta, genesis_info)) => {
                        db_data.token_metas.insert(
                            db_data.next_token_num,
                            Protocol::Slp(meta),
                        );
                        new_tokens.push((
                            tx_num,
                            db_data.next_token_num,
                            Protocol::Slp((meta, genesis_info)),
                        ));
                    }
                    Protocol::Slpv2((meta, genesis_info)) => {
                        db_data.token_metas.insert(
                            db_data.next_token_num,
                            Protocol::Slpv2(meta),
                        );
                        new_tokens.push((
                            tx_num,
                            db_data.next_token_num,
                            Protocol::Slpv2((meta, genesis_info)),
                        ));
                    }
                }
                db_data.next_token_num += 1;
            }

            let mut token_metas = BTreeSet::new();
            for token in inputs.iter().chain(&processed_tx.outputs) {
                match token {
                    Some(Protocol::Slp(token)) => {
                        token_metas.insert(Protocol::Slp(token.meta));
                    }
                    Some(Protocol::Slpv2(token)) => {
                        token_metas.insert(Protocol::Slpv2(token.meta));
                    }
                    None => {}
                }
            }
            let mut flags = has_genesis as u8;
            let mut all_token_metas = Vec::with_capacity(token_metas.len() + 1);
            if let Some(group_token_meta) = processed_tx.group_token_meta {
                all_token_metas.push(Protocol::Slp(group_token_meta));
                token_metas.remove(&Protocol::Slp(group_token_meta));
                flags |= FLAGS_HAS_GROUP_TOKEN_ID;
            }
            all_token_metas.extend(token_metas);

            let mut db_token_nums = Vec::with_capacity(all_token_metas.len());
            let mut metas = Vec::with_capacity(all_token_metas.len());
            for meta in all_token_metas {
                if let Some(&token_num) =
                    db_data.token_metas.get_by_right(&meta)
                {
                    db_token_nums.push(token_num);
                    metas.push(meta);
                }
            }

            let db_tx_data = DbTxData {
                token_nums: db_token_nums,
                inputs: to_db_tokens(&inputs, &metas),
                outputs: to_db_tokens(&processed_tx.outputs, &metas),
                flags,
            };

            new_tx_data.insert(tx_num, db_tx_data);
            if let Some(tx_data) = processed_tx.tx_data {
                self.valid.insert(tx_num, tx_data);
            }
        }
        for tx in &self.non_slp_txs {
            let mut has_any_token = false;
            let mut db_token_nums = Vec::new();
            let mut db_inputs = Vec::with_capacity(tx.input_nums.len());
            for (&input_tx_num, input) in
                tx.input_nums.iter().zip(&tx.tx.inputs)
            {
                let out_idx = input.prev_out.out_idx as usize;
                let token = new_tx_data
                    .get(&input_tx_num)
                    .or_else(|| db_data.data.get(&input_tx_num))
                    .and_then(|db_tx_data| db_tx_data.output(out_idx));
                let (token_num, token) = match token {
                    Some(token) => token,
                    None => {
                        db_inputs.push(None);
                        continue;
                    }
                };
                let token_num_idx = db_token_nums
                    .iter()
                    .position(|&num| num == token_num)
                    .unwrap_or_else(|| {
                        db_token_nums.push(token_num);
                        db_token_nums.len() - 1
                    });
                db_inputs.push(Some(DbToken {
                    token_num_idx,
                    variant: token.variant,
                }));
                has_any_token = true;
            }
            if has_any_token {
                new_tx_data.insert(
                    tx.tx_num,
                    DbTxData {
                        token_nums: db_token_nums,
                        inputs: db_inputs,
                        outputs: vec![None; tx.tx.outputs.len()],
                        flags: 0,
                    },
                );
            }
        }
        Ok(BatchProcessResult {
            new_tokens,
            new_tx_data,
        })
    }

    fn tx_token_inputs<'a>(
        &'a self,
        tx: &IndexTx<'_>,
        db_data: &BatchDbData,
    ) -> Result<Vec<Option<EitherToken>>> {
        if tx.is_coinbase {
            Ok(vec![])
        } else {
            let mut inputs = Vec::with_capacity(tx.input_nums.len());
            for (&input_num, input) in tx.input_nums.iter().zip(&tx.tx.inputs) {
                inputs.push(self.token_output(
                    input_num,
                    input.prev_out.out_idx as usize,
                    db_data,
                )?);
            }
            Ok(inputs)
        }
    }

    fn token_output(
        &self,
        tx_num: TxNum,
        out_idx: usize,
        db_data: &BatchDbData,
    ) -> Result<Option<EitherToken>> {
        if let Some(tx_data) = self.valid.get(&tx_num) {
            return Ok(match tx_data {
                Protocol::Slp(tx_data) => {
                    Some(Protocol::Slp(slp::SlpSpentOutput {
                        meta: tx_data.meta,
                        token: match tx_data.output_tokens.get(out_idx) {
                            Some(&Some(token)) => token,
                            _ => return Ok(None),
                        },
                        group_token_id: tx_data.group_token_id,
                    }))
                }
                Protocol::Slpv2(tx_data) => tx_data
                    .outputs()
                    .nth(out_idx)
                    .flatten()
                    .map(Protocol::Slpv2),
            });
        }
        let db_tx_data = match db_data.data.get(&tx_num) {
            Some(db_tx_data) => db_tx_data,
            None => return Ok(None),
        };
        let (token_num, db_output) = match db_tx_data.output(out_idx) {
            Some(output) => output,
            None => return Ok(None),
        };
        let token_meta = db_data
            .token_metas
            .get_by_left(&token_num)
            .ok_or(BatchError::MissingTokenNum(token_num))?;
        match *token_meta {
            Protocol::Slp(meta) => {
                let has_group =
                    (db_tx_data.flags & FLAGS_HAS_GROUP_TOKEN_ID) != 0;
                let group_token_id = if has_group {
                    let group_meta = db_data
                        .token_metas
                        .get_by_left(&db_tx_data.token_nums[0])
                        .ok_or(BatchError::MissingTokenNum(token_num))?;
                    match group_meta {
                        Protocol::Slp(meta) => Some(meta.token_id),
                        Protocol::Slpv2(_) => None,
                    }
                } else {
                    None
                };
                Ok(Some(Protocol::Slp(slp::SlpSpentOutput {
                    meta,
                    token: db_output.variant,
                    group_token_id,
                })))
            }
            Protocol::Slpv2(meta) => Ok(Some(Protocol::Slpv2(slpv2::Token {
                meta,
                variant: match slpv2::TokenVariant::from_slpv1(
                    db_output.variant,
                ) {
                    Some(variant) => variant,
                    None => return Ok(None),
                },
            }))),
        }
    }
}

fn to_db_tokens(
    tokens: &[Option<EitherToken>],
    metas: &[EitherMeta],
) -> Vec<Option<DbToken>> {
    tokens
        .iter()
        .map(|token| {
            token.as_ref().and_then(|token| {
                let (token_meta, variant) = match token {
                    Protocol::Slp(slp) => (Protocol::Slp(slp.meta), slp.token),
                    Protocol::Slpv2(slpv2) => {
                        (Protocol::Slpv2(slpv2.meta), slpv2.variant.to_slpv1()?)
                    }
                };
                Some(DbToken {
                    token_num_idx: metas
                        .iter()
                        .position(|meta| meta == &token_meta)?,
                    variant,
                })
            })
        })
        .collect()
}
