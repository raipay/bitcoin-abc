use bitcoinsuite_slp::slpv2::{self, SectionType};

use crate::slpv2::io::TokenNum;

#[derive(Clone)]
pub struct DbTxData {
    pub sections: Vec<DbTxSection>,
    pub burn_token_nums: Vec<TokenNum>,
    pub input_tokens: Vec<Option<slpv2::TokenOutputData>>,
    pub output_tokens: Vec<Option<slpv2::TokenOutputData>>,
}

#[derive(Clone)]
pub struct DbTxSection {
    pub token_num: TokenNum,
    pub section_type: SectionType,
    pub required_input_sum: slpv2::Amount,
    pub burn_amount: slpv2::Amount,
}
