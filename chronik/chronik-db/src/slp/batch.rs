use std::{borrow::Cow, collections::HashMap};

use abc_rust_error::Result;
use bimap::BiMap;
use bitcoinsuite_slp::slp;
use itertools::Itertools;
use thiserror::Error;
use topo_sort::TopoSort;

use crate::{
    index_tx::IndexTx,
    io::TxNum,
    slp::{
        io::TokenNum,
        structs::{DbBurn, DbTxData},
    },
};

pub struct BatchProcessor<'tx> {
    parsed_txs: HashMap<TxNum, (&'tx IndexTx<'tx>, slp::ParseData)>,
    has_any_genesis: bool,
    valid: HashMap<TxNum, slp::TxData>,
}

pub struct BatchDbData {
    pub next_token_num: TokenNum,
    pub data: HashMap<TxNum, DbTxData>,
    pub token_ids: BiMap<TokenNum, slp::TokenId>,
}

pub struct BatchProcessResult {
    pub new_tokens: Vec<(TokenNum, slp::TokenMeta, slp::GenesisInfo)>,
    pub new_tx_data: Vec<(TxNum, DbTxData)>,
}

#[derive(Debug, Error, PartialEq)]
pub enum BatchError {
    #[error("Cycle in SLPv2 txs")]
    Cycle,

    #[error("Inconsistent BatchDbData: Missing TokenId for token_num {0}")]
    MissingTokenNum(TokenNum),

    #[error("Inconsistent BatchDbData: Missing TokenNum for token_id {0}")]
    MissingTokenId(slp::TokenId),
}

use self::BatchError::*;

impl<'tx> BatchProcessor<'tx> {
    pub fn prepare(txs: &'tx [IndexTx<'tx>]) -> Self {
        let parsed_txs = txs
            .iter()
            .filter_map(|tx| {
                Some((tx.tx_num, (tx, slp::parse_tx(&tx.tx).ok()?)))
            })
            .collect::<HashMap<_, _>>();
        let has_any_genesis = parsed_txs
            .values()
            .any(|(_, data)| matches!(data.tx_type, slp::TxType::Genesis(_)));
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
            let tx_data = match slp::validate(&parsed, &inputs) {
                Ok(tx_data) => tx_data,
                Err(err) => continue,
            };
            if let slp::TxType::Genesis(genesis_info) =
                &parsed.tx_type
            {
                db_data.token_ids.insert(
                    db_data.next_token_num,
                    tx_data.token_id,
                );
                new_tokens.push((
                    db_data.next_token_num,
                    slp::TokenMeta {
                        token_id: tx_data.token_id,
                        token_type: tx_data.token_type,
                    },
                    genesis_info.as_ref().clone(),
                ));
                db_data.next_token_num += 1;
            }
            let token_num = *db_data
                .token_ids
                .get_by_right(&tx_data.token_id)
                .ok_or(MissingTokenId(tx_data.token_id))?;
            let mut db_burns = Vec::with_capacity(tx_data.slp_burns.len());
            for burn in &tx_data.slp_burns {
                db_burns.push(match burn {
                    Some(burn) => {
                        let burn_token_num = *db_data
                            .token_ids
                            .get_by_right(&burn.token_id)
                            .ok_or(MissingTokenId(burn.token_id))?;
                        Some(DbBurn {
                            token_num: burn_token_num,
                            token: burn.token,
                        })
                    }
                    None => None,
                });
            }
            let group_token_num = match &tx_data.group_token_id {
                Some(group_token_id) => Some(
                    *db_data
                        .token_ids
                        .get_by_right(group_token_id.as_ref())
                        .ok_or(MissingTokenId(
                            tx_data.token_id,
                        ))?,
                ),
                None => None,
            };
            let db_tx_data = DbTxData {
                token_num,
                token_type: tx_data.token_type,
                tx_type: tx_data.tx_type,
                burns: db_burns,
                input_tokens: tx_data.input_tokens.clone(),
                output_tokens: tx_data.output_tokens.clone(),
                group_token_num,
            };
            new_tx_data.push((tx_num, db_tx_data));
            self.valid.insert(tx_num, tx_data);
        }
        Ok(BatchProcessResult {
            new_tokens,
            new_tx_data,
        })
    }

    fn token_output(
        &self,
        tx_num: TxNum,
        out_idx: usize,
        db_data: &BatchDbData,
    ) -> Result<Option<slp::SlpSpentOutput>> {
        if let Some(tx_data) = self.valid.get(&tx_num) {
            match tx_data.output_tokens.get(out_idx) {
                Some(token) => {
                    return Ok(Some(slp::SlpSpentOutput {
                        token_id: tx_data.token_id,
                        token_type: tx_data.token_type,
                        token: *token,
                        group_token_id: tx_data.group_token_id.clone(),
                    }));
                }
                None => return Ok(None),
            }
        }
        let db_tx_data = match db_data.data.get(&tx_num) {
            Some(db_tx_data) => db_tx_data,
            None => return Ok(None),
        };
        let db_output = match db_tx_data.output_tokens.get(out_idx) {
            Some(db_output) => db_output,
            None => return Ok(None),
        };
        let token_num = db_tx_data.token_num;
        let token_id = match db_data.token_ids.get_by_left(&token_num) {
            Some(token_id) => token_id,
            None => return Err(BatchError::MissingTokenNum(token_num).into()),
        };
        let group_token_id = match db_tx_data.group_token_num {
            Some(token_num) => {
                match db_data.token_ids.get_by_left(&token_num) {
                    Some(token_id) => Some(Box::new(*token_id)),
                    None => {
                        return Err(
                            BatchError::MissingTokenNum(token_num).into()
                        )
                    }
                }
            }
            None => None,
        };
        Ok(Some(slp::SlpSpentOutput {
            token_id: *token_id,
            token_type: db_tx_data.token_type,
            token: db_output.clone(),
            group_token_id,
        }))
    }
}
