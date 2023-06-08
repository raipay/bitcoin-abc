use bitcoinsuite_slp::{slp, slpv2};
use serde::{Deserialize, Serialize};

use crate::slp::io::TokenNum;

pub const FLAGS_HAS_GENESIS: u8 = 1;
pub const FLAGS_HAS_GROUP_TOKEN_ID: u8 = 2;

pub type EitherMeta = Protocol<slp::TokenMeta, slpv2::TokenMeta>;
pub type EitherToken = Protocol<slp::SlpSpentOutput, slpv2::Token>;
pub type EitherTxData = Protocol<slp::TxData, slpv2::TxData>;
pub type EitherGenesisInfo = Protocol<slp::GenesisInfo, slpv2::GenesisInfo>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DbTxData {
    pub token_nums: Vec<TokenNum>,
    pub inputs: Vec<Option<DbToken>>,
    pub outputs: Vec<Option<DbToken>>,
    pub flags: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DbToken {
    pub token_num_idx: usize,
    pub variant: slp::Token,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DbGenesisData {
    pub token_num: TokenNum,
    pub genesis_info: EitherGenesisInfo,
}

impl DbTxData {
    pub fn output(&self, out_idx: usize) -> Option<(TokenNum, &DbToken)> {
        let db_output = self.outputs.get(out_idx)?.as_ref()?;
        let token_num = self.token_nums[db_output.token_num_idx];
        Some((token_num, db_output))
    }

    pub fn assemble_tokens(
        &self,
        db_tokens: &[Option<DbToken>],
        metas: &[EitherMeta],
    ) -> Vec<Option<EitherToken>> {
        let has_group = (self.flags & FLAGS_HAS_GROUP_TOKEN_ID) != 0;
        let group_token_id = match metas.get(0) {
            Some(Protocol::Slp(meta)) if has_group => Some(meta.token_id),
            _ => None,
        };
        db_tokens
            .iter()
            .map(|output| {
                output.as_ref().and_then(|token| {
                    let meta = &metas[token.token_num_idx];
                    Some(match *meta {
                        Protocol::Slp(meta) => {
                            Protocol::Slp(slp::SlpSpentOutput {
                                meta,
                                token: token.variant,
                                group_token_id,
                            })
                        }
                        Protocol::Slpv2(meta) => {
                            Protocol::Slpv2(slpv2::Token {
                                meta,
                                variant: slpv2::TokenVariant::from_slpv1(
                                    token.variant,
                                )?,
                            })
                        }
                    })
                })
            })
            .collect::<Vec<_>>()
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
pub enum Protocol<A, B> {
    Slp(A),
    Slpv2(B),
}

impl Protocol<slp::ParseData, slpv2::ColoredTx> {
    pub fn is_genesis(&self) -> bool {
        match self {
            Protocol::Slp(slp) => {
                slp.tx_type.tx_type_variant() == slp::TxTypeVariant::Genesis
            }
            Protocol::Slpv2(slpv2) => {
                if slpv2.sections.is_empty() {
                    return false;
                }
                slpv2.sections[0].section_type == slpv2::SectionType::GENESIS
            }
        }
    }
}

impl<A: Clone, B: Clone> Protocol<&A, &B> {
    pub fn cloned(&self) -> Protocol<A, B> {
        match *self {
            Protocol::Slp(a) => Protocol::Slp(a.clone()),
            Protocol::Slpv2(b) => Protocol::Slpv2(b.clone()),
        }
    }
}

pub fn only_slpv1_inputs(
    inputs: &[Option<EitherToken>],
) -> Vec<Option<slp::SlpSpentOutput>> {
    inputs
        .iter()
        .map(|input| match input {
            Some(Protocol::Slp(token)) => Some(token.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
}

pub fn only_slpv2_inputs(
    inputs: &[Option<EitherToken>],
) -> Vec<Option<slpv2::Token>> {
    inputs
        .iter()
        .map(|input| match input {
            Some(Protocol::Slpv2(token)) => Some(token.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
}
