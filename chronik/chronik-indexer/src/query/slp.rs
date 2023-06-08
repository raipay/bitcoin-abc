use std::borrow::Cow;

use abc_rust_error::Result;
use bitcoinsuite_core::tx::{Tx, TxId};
use bitcoinsuite_slp::{
    slp::{self, ParseData},
    slpv2::{self, ColoredTx},
};
use chronik_db::{
    db::Db,
    io::{BlockHeight, BlockReader, TxNum, TxReader},
    mem::Mempool,
    slp::{
        data::{
            only_slpv1_inputs, only_slpv2_inputs, DbToken, EitherToken,
            EitherTxData, Protocol,
        },
        io::{SlpReader, TokenNum},
        mem::MempoolSlp,
    },
};
use chronik_proto::proto;
use chronik_util::log;
use thiserror::Error;

use crate::avalanche::Avalanche;

pub struct SlpDbData<'a> {
    pub token_inputs: Cow<'a, [Option<EitherToken>]>,
    pub tx_data: Cow<'a, EitherTxData>,
    pub slpv1_error: Option<Cow<'a, slp::ParseError>>,
}

/// Errors indicating something went wrong with reading txs.
#[derive(Debug, Error, PartialEq)]
pub enum SlpDbDataError {
    /// Transaction not in mempool nor DB.
    #[error("400: Input tx {0} not found")]
    InputTxNotFound(TxId),

    /// Token num not found.
    #[error("500: Inconsistent DB: Token num {0} not found")]
    TokenNumDoesntExist(TokenNum),

    #[error("500: Inconsistent DB: TxData for token {0} not in mempool")]
    TokenTxDataNotInMempool(TxId),

    #[error("500: Inconsistent DB: MempoolTx for token {0} not in mempool")]
    TokenTxNotInMempool(TxId),

    #[error("500: Inconsistent DB: Mixing SLP and SLPv2")]
    MixingSlpAndSlpv2,

    #[error("500: Inconsistent DB: Missing block for height {0}")]
    MissingBlockForHeight(BlockHeight),
}

use self::SlpDbDataError::*;

impl<'a> SlpDbData<'a> {
    pub fn from_mempool(mempool: &'a MempoolSlp, txid: &TxId) -> Option<Self> {
        let tx_data = mempool.tx_data(txid);
        let token_inputs = mempool.tx_token_inputs(txid);
        let slpv1_error = mempool.slpv1_error(txid);
        if tx_data.is_none() && token_inputs.is_none() {
            return None;
        }
        log!("token_inputs for {} = {:?}\n", txid, token_inputs);
        Some(SlpDbData {
            token_inputs: token_inputs
                .map(|inputs| Cow::Borrowed(inputs.as_slice()))
                .unwrap_or(Cow::Borrowed(&[])),
            tx_data: tx_data.map(Cow::Borrowed).unwrap_or(Cow::Owned(
                Protocol::Slpv2(slpv2::TxData::default()),
            )),
            slpv1_error: slpv1_error.map(Cow::Borrowed),
        })
    }

    pub fn from_db(db: &Db, tx_num: TxNum, tx: &Tx) -> Result<Option<Self>> {
        let slp_reader = SlpReader::new(db)?;
        let (metas, db_tx_data) =
            match slp_reader.token_metas_and_db_by_tx_num(tx_num)? {
                Some(db_data) => db_data,
                None => return Ok(None),
            };
        let token_inputs =
            db_tx_data.assemble_tokens(&db_tx_data.inputs, &metas);
        Self::parse_and_verify(tx, token_inputs)
    }

    pub fn from_tx(
        db: &Db,
        mempool: &'a MempoolSlp,
        tx: &Tx,
        is_mempool_tx: impl Fn(&TxId) -> bool,
    ) -> Result<Option<Self>> {
        let tx_reader = TxReader::new(db)?;
        let slp_reader = SlpReader::new(db)?;
        let mut token_inputs = Vec::with_capacity(tx.inputs.len());
        for tx_input in &tx.inputs {
            let input_txid = &tx_input.prev_out.txid;
            let out_idx = tx_input.prev_out.out_idx as usize;
            if let Some(tx_data) = mempool.tx_data(input_txid) {
                token_inputs.push(match tx_data {
                    Protocol::Slp(tx_data) => {
                        match tx_data.output_tokens.get(out_idx) {
                            Some(&Some(token)) => {
                                Some(Protocol::Slp(slp::SlpSpentOutput {
                                    meta: tx_data.meta,
                                    token,
                                    group_token_id: tx_data.group_token_id,
                                }))
                            }
                            _ => None,
                        }
                    }
                    Protocol::Slpv2(tx_data) => tx_data
                        .outputs()
                        .nth(out_idx)
                        .flatten()
                        .map(Protocol::Slpv2),
                });
                continue;
            }
            if is_mempool_tx(input_txid) {
                token_inputs.push(None);
                continue;
            }
            let tx_num = tx_reader
                .tx_num_by_txid(input_txid)?
                .ok_or(InputTxNotFound(*input_txid))?;
            let db_tx_data = match slp_reader.tx_data_by_tx_num(tx_num)? {
                Some(db_tx_data) => db_tx_data,
                None => {
                    token_inputs.push(None);
                    continue;
                }
            };
            if let Some((token_num, db_token)) = db_tx_data.output(out_idx) {
                let meta = slp_reader
                    .token_meta_by_token_num(token_num)?
                    .ok_or(TokenNumDoesntExist(token_num))?;
                let db_token = DbToken {
                    token_num_idx: 0,
                    ..*db_token
                };
                token_inputs.push(
                    db_tx_data
                        .assemble_tokens(&[Some(db_token)], &[meta])
                        .remove(0),
                );
            } else {
                token_inputs.push(None);
            }
        }
        Self::parse_and_verify(tx, token_inputs)
    }

    fn parse_and_verify(
        tx: &Tx,
        token_inputs: Vec<Option<EitherToken>>,
    ) -> Result<Option<Self>> {
        match slp::parse_tx(tx) {
            Ok(parse_data) => {
                let actual_inputs = only_slpv1_inputs(&token_inputs);
                let tx_data = slp::verify(&parse_data, &actual_inputs);
                Ok(Some(SlpDbData {
                    token_inputs: Cow::Owned(token_inputs),
                    tx_data: Cow::Owned(Protocol::Slp(tx_data)),
                    slpv1_error: None,
                }))
            }
            Err(slpv1_error) => {
                let actual_inputs = only_slpv2_inputs(&token_inputs);
                let colored_tx = slpv2::ColoredTx::parse_tx(tx);
                let tx_data = slpv2::verify(colored_tx, &actual_inputs);
                Ok(Some(SlpDbData {
                    token_inputs: Cow::Owned(token_inputs),
                    tx_data: Cow::Owned(Protocol::Slpv2(tx_data)),
                    slpv1_error: Some(Cow::Owned(slpv1_error)),
                }))
            }
        }
    }

    pub fn input_token_proto(
        &self,
        input_idx: usize,
    ) -> Option<proto::SlpToken> {
        match self.token_inputs.get(input_idx)?.as_ref()? {
            Protocol::Slp(token) => {
                let is_burned = match self.tx_data.as_ref() {
                    Protocol::Slp(tx_data) => {
                        if tx_data.is_total_burn {
                            true
                        } else if tx_data.meta == token.meta {
                            false
                        } else {
                            tx_data
                                .burns
                                .iter()
                                .any(|burn| burn.meta == token.meta)
                        }
                    }
                    Protocol::Slpv2(_) => true,
                };
                Some(proto::SlpToken {
                    token_id: token.meta.token_id.to_vec(),
                    token_protocol: proto::TokenProtocol::Slpv1 as _,
                    slpv1_token_type: token.meta.token_type.to_u16().into(),
                    slpv2_token_type: Default::default(),
                    slpv2_section_idx: Default::default(),
                    is_burned,
                    amount: token.token.amount(),
                    is_mint_baton: token.token == slp::Token::MintBaton,
                })
            }
            Protocol::Slpv2(token) => {
                let (is_burned, slpv2_section_idx) =
                    match &self.tx_data.as_ref() {
                        Protocol::Slp(_) => (true, -1),
                        Protocol::Slpv2(tx_data) => tx_data
                            .sections
                            .iter()
                            .position(|section| section.meta == token.meta)
                            .map(|section| (false, section as i32))
                            .unwrap_or((true, -1)),
                    };
                Some(proto::SlpToken {
                    token_id: token.meta.token_id.to_vec(),
                    token_protocol: proto::TokenProtocol::Slpv2 as _,
                    slpv1_token_type: Default::default(),
                    slpv2_token_type: token.meta.token_type.to_u8().into(),
                    slpv2_section_idx,
                    is_burned,
                    amount: token.variant.amount().int().try_into().ok()?,
                    is_mint_baton: token.variant
                        == slpv2::TokenVariant::MintBaton,
                })
            }
        }
    }

    pub fn output_token_proto(
        &self,
        output_idx: usize,
    ) -> Option<proto::SlpToken> {
        match self.tx_data.as_ref() {
            Protocol::Slp(tx_data) => {
                tx_data.output_tokens.get(output_idx).and_then(|token| {
                    Some(Self::slpv1_output_token_proto(
                        token.as_ref()?,
                        tx_data,
                    ))
                })
            }
            Protocol::Slpv2(tx_data) => {
                tx_data.outputs.get(output_idx).and_then(|output_token| {
                    Some(Self::slpv2_output_token_proto(
                        output_token.as_ref()?,
                        tx_data,
                    ))
                })
            }
        }
    }

    fn slpv1_output_token_proto(
        output_token: &slp::Token,
        tx_data: &slp::TxData,
    ) -> proto::SlpToken {
        proto::SlpToken {
            token_id: tx_data.meta.token_id.to_vec(),
            token_protocol: proto::TokenProtocol::Slpv1 as _,
            slpv1_token_type: tx_data.meta.token_type.to_u16().into(),
            slpv2_token_type: Default::default(),
            slpv2_section_idx: Default::default(),
            is_burned: false,
            amount: output_token.amount(),
            is_mint_baton: output_token == &slp::Token::MintBaton,
        }
    }

    fn slpv2_output_token_proto(
        output_token_data: &slpv2::TokenData,
        tx_data: &slpv2::TxData,
    ) -> proto::SlpToken {
        let output_token = tx_data.token(output_token_data);
        proto::SlpToken {
            token_id: output_token.meta.token_id.to_vec(),
            token_protocol: proto::TokenProtocol::Slpv2 as _,
            slpv1_token_type: Default::default(),
            slpv2_token_type: output_token.meta.token_type.to_u8().into(),
            slpv2_section_idx: output_token_data.section_idx as _,
            is_burned: false,
            amount: output_token.variant.amount().int() as _,
            is_mint_baton: output_token.variant
                == slpv2::TokenVariant::MintBaton,
        }
    }

    pub fn slpv1_tx_data(&self) -> Option<proto::Slpv1TxData> {
        match self.tx_data.as_ref() {
            Protocol::Slp(tx_data) => Some(proto::Slpv1TxData {
                token_id: tx_data.meta.token_id.to_vec(),
                token_type: tx_data.meta.token_type.to_u16().into(),
                tx_type: match tx_data.tx_type {
                    slp::TxTypeVariant::Genesis => proto::SlpTxType::Genesis,
                    slp::TxTypeVariant::Send => proto::SlpTxType::Send,
                    slp::TxTypeVariant::Mint => proto::SlpTxType::Mint,
                    slp::TxTypeVariant::Burn => proto::SlpTxType::Burn,
                    slp::TxTypeVariant::Unknown => proto::SlpTxType::Unknown,
                } as _,
                group_token_id: tx_data
                    .group_token_id
                    .as_ref()
                    .map(|group_token_id| group_token_id.to_vec())
                    .unwrap_or_default(),
            }),
            Protocol::Slpv2(_) => None,
        }
    }

    pub fn slpv2_sections_proto(&self) -> Vec<proto::Slpv2Section> {
        let tx_data = match self.tx_data.as_ref() {
            Protocol::Slp(_) => return vec![],
            Protocol::Slpv2(tx_data) => tx_data,
        };
        tx_data
            .sections
            .iter()
            .map(|section| proto::Slpv2Section {
                token_id: section.meta.token_id.to_vec(),
                token_type: section.meta.token_type.to_u8().into(),
                section_type: match section.section_type {
                    slpv2::SectionType::GENESIS => proto::SlpTxType::Genesis,
                    slpv2::SectionType::MINT => proto::SlpTxType::Mint,
                    slpv2::SectionType::SEND => proto::SlpTxType::Send,
                    slpv2::SectionType::UNKNOWN => proto::SlpTxType::Unknown,
                    // should be unreachable
                    slpv2::SectionType::BURN => proto::SlpTxType::Burn,
                } as _,
            })
            .collect()
    }

    pub fn burns(
        &'a self,
    ) -> (Cow<'a, [slp::TokenBurn]>, Cow<'a, [slpv2::TokenBurn]>) {
        match self.tx_data.as_ref() {
            Protocol::Slp(tx_data) => {
                let slpv2_inputs = only_slpv2_inputs(&self.token_inputs);
                let slpv2_data =
                    slpv2::verify(ColoredTx::default(), &slpv2_inputs);
                (Cow::Borrowed(&tx_data.burns), Cow::Owned(slpv2_data.burns))
            }
            Protocol::Slpv2(tx_data) => {
                let slpv1_dummy = ParseData {
                    meta: slp::TokenMeta {
                        token_id: Default::default(),
                        token_type: slp::TokenType::Unknown(0xffff),
                    },
                    tx_type: slp::TxType::Unknown, // always burns all tokens
                    output_tokens: vec![],
                };
                let slpv1_inputs = only_slpv1_inputs(&self.token_inputs);
                let slpv1_data = slp::verify(&slpv1_dummy, &slpv1_inputs);
                (Cow::Owned(slpv1_data.burns), Cow::Borrowed(&tx_data.burns))
            }
        }
    }

    pub fn burns_proto(&self) -> Vec<proto::SlpBurn> {
        let (slp_burns, slpv2_burns) = self.burns();
        let mut burns = Vec::with_capacity(slp_burns.len() + slpv2_burns.len());
        for burn in slp_burns.as_ref() {
            burns.push(proto::SlpBurn {
                token_id: burn.meta.token_id.to_vec(),
                token_protocol: proto::TokenProtocol::Slpv1 as _,
                slpv1_token_type: burn.meta.token_type.to_u16().into(),
                burn_error: burn
                    .error
                    .as_ref()
                    .map(|err| err.to_string())
                    .unwrap_or_default(),
                slpv1_actual_burn: burn.amount.to_string(),
                burn_mint_batons: burn.burn_mint_batons,
                ..Default::default()
            });
        }
        for burn in slpv2_burns.as_ref() {
            burns.push(proto::SlpBurn {
                token_id: burn.meta.token_id.to_vec(),
                token_protocol: proto::TokenProtocol::Slpv2 as _,
                burn_error: burn
                    .error
                    .as_ref()
                    .map(|err| err.to_string())
                    .unwrap_or_default(),
                burn_mint_batons: burn.burn_mint_batons,
                slpv2_token_type: burn.meta.token_type.to_u8().into(),
                slpv2_intentional_burn: burn
                    .intentional_burn
                    .unwrap_or_default()
                    .int(),
                slpv2_actual_burn: burn.actual_burn.int(),
                ..Default::default()
            });
        }
        burns
    }

    pub fn slp_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut any_slpv1_inputs = false;
        let mut any_slpv2_inputs = false;
        for input in self.token_inputs.as_ref() {
            match input {
                Some(Protocol::Slp(_)) => any_slpv1_inputs = true,
                Some(Protocol::Slpv2(_)) => any_slpv2_inputs = true,
                _ => {}
            }
        }
        if let Protocol::Slpv2(tx_data) = self.tx_data.as_ref() {
            if tx_data.sections.is_empty() && any_slpv1_inputs {
                // SLPv1 parse errors end up becoming SLPv2 TxData
                if let Some(err) = &self.slpv1_error {
                    errors.push(err.to_string());
                }
            }
            for err in &tx_data.color_errors {
                if !any_slpv2_inputs && err.should_ignore() {
                    continue;
                }
                errors.push(err.to_string());
            }
        }
        errors
    }
}

pub fn db_output(
    db: &Db,
    tx_num: TxNum,
    out_idx: u32,
) -> Result<Option<EitherToken>> {
    let slp_reader = SlpReader::new(db)?;
    let db_tx_data = match slp_reader.tx_data_by_tx_num(tx_num)? {
        Some(db_tx_data) => db_tx_data,
        None => return Ok(None),
    };
    let db_output = match db_tx_data.outputs.get(out_idx as usize) {
        Some(&Some(output)) => output,
        _ => return Ok(None),
    };
    let token_num = db_tx_data.token_nums[db_output.token_num_idx];
    let token_meta = match slp_reader.token_meta_by_token_num(token_num)? {
        Some(token_meta) => token_meta,
        None => return Ok(None),
    };
    Ok(db_tx_data
        .assemble_tokens(&[Some(db_output)], &[token_meta])
        .remove(0))
}

pub fn make_slp_token_proto(token: &EitherToken) -> proto::SlpToken {
    match token {
        Protocol::Slp(token) => proto::SlpToken {
            token_id: token.meta.token_id.to_vec(),
            token_protocol: proto::TokenProtocol::Slpv1 as _,
            slpv1_token_type: token.meta.token_type.to_u16().into(),
            amount: token.token.amount(),
            is_mint_baton: token.token.is_mint_baton(),
            ..Default::default()
        },
        Protocol::Slpv2(token) => proto::SlpToken {
            token_id: token.meta.token_id.to_vec(),
            token_protocol: proto::TokenProtocol::Slpv2 as _,
            slpv2_token_type: token.meta.token_type.to_u8() as _,
            slpv2_section_idx: -1,
            amount: token.variant.amount().int() as u64,
            is_mint_baton: token.variant.is_mint_baton(),
            ..Default::default()
        },
    }
}

pub struct TokenInfo {
    pub data: Protocol<
        (slp::GenesisInfo, slp::TokenMeta),
        (slpv2::GenesisInfo, slpv2::TokenMeta),
    >,
    pub block: Option<proto::BlockMetadata>,
    pub time_first_seen: i64,
}

pub fn token_info(
    db: &Db,
    mempool: &Mempool,
    avalanche: &Avalanche,
    token_id_txid: &TxId,
) -> Result<Option<TokenInfo>> {
    if let Some(genesis_info) = mempool.slp().genesis_info(token_id_txid) {
        let tx_data = mempool
            .slp()
            .tx_data(token_id_txid)
            .ok_or(TokenTxDataNotInMempool(*token_id_txid))?;
        let mempool_tx = mempool
            .tx(token_id_txid)
            .ok_or(TokenTxNotInMempool(*token_id_txid))?;
        return match (genesis_info, tx_data) {
            (Protocol::Slp(genesis_info), Protocol::Slp(tx_data)) => {
                Ok(Some(TokenInfo {
                    data: Protocol::Slp((genesis_info.clone(), tx_data.meta)),
                    block: None,
                    time_first_seen: mempool_tx.time_first_seen,
                }))
            }
            (Protocol::Slpv2(genesis_info), Protocol::Slpv2(tx_data)) => {
                Ok(Some(TokenInfo {
                    data: Protocol::Slpv2((
                        genesis_info.clone(),
                        tx_data.sections[0].meta,
                    )),
                    block: None,
                    time_first_seen: mempool_tx.time_first_seen,
                }))
            }
            _ => Err(MixingSlpAndSlpv2.into()),
        };
    }
    let tx_reader = TxReader::new(db)?;
    let slp_reader = SlpReader::new(db)?;
    let block_reader = BlockReader::new(db)?;
    let (tx_num, block_tx) =
        match tx_reader.tx_and_num_by_txid(token_id_txid)? {
            Some(tuple) => tuple,
            None => return Ok(None),
        };
    let block = block_reader
        .by_height(block_tx.block_height)?
        .ok_or(MissingBlockForHeight(block_tx.block_height))?;
    let db_genesis = match slp_reader.genesis_info_by_tx_num(tx_num)? {
        Some(db_genesis) => db_genesis,
        None => return Ok(None),
    };
    let meta = slp_reader
        .token_meta_by_token_num(db_genesis.token_num)?
        .ok_or(TokenNumDoesntExist(db_genesis.token_num))?;
    let slp_data = match (db_genesis.genesis_info, meta) {
        (Protocol::Slp(genesis_info), Protocol::Slp(meta)) => {
            Protocol::Slp((genesis_info, meta))
        }
        (Protocol::Slpv2(genesis_info), Protocol::Slpv2(meta)) => {
            Protocol::Slpv2((genesis_info, meta))
        }
        _ => return Err(MixingSlpAndSlpv2.into()),
    };
    Ok(Some(TokenInfo {
        data: slp_data,
        block: Some(proto::BlockMetadata {
            height: block_tx.block_height,
            hash: block.hash.to_vec(),
            timestamp: block.timestamp,
            is_final: avalanche.is_final_height(block_tx.block_height),
        }),
        time_first_seen: block_tx.entry.time_first_seen,
    }))
}
