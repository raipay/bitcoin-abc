use bitcoinsuite_slp::slp;

use crate::slp::io::TokenNum;

#[derive(Clone)]
pub struct DbTxData {
    pub token_num: TokenNum,
    pub token_type: slp::TokenType,
    pub tx_type: slp::TxTypeVariant,
    pub burns: Vec<Option<DbBurn>>,
    pub input_tokens: Vec<slp::Token>,
    pub output_tokens: Vec<slp::Token>,
    pub group_token_num: Option<TokenNum>,
}

#[derive(Clone)]
pub struct DbBurn {
    pub token_num: TokenNum,
    pub token: slp::Token,
}
