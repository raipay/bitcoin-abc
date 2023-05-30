
use bitcoinsuite_core::script::{Script, ScriptMut, opcode::{OP_RETURN, OP_RESERVED}, Op};
use bytes::{Bytes, BytesMut, BufMut};

use crate::{slpv2::{TokenType, GenesisInfo, MintData, consts::SLPV2_LOKAD_ID, TokenId, Amount}, common::{GENESIS, MINT, BURN, SEND}
};

pub fn genesis_section(
    token_type: TokenType,
    genesis_info: &GenesisInfo,
    mint_data: &MintData,
) -> Bytes {
    let mut section = BytesMut::new();
    section.put_slice(&SLPV2_LOKAD_ID);
    section.put_slice(&[token_type.to_u8()]);
    section.put_slice(&[GENESIS.len() as u8]);
    section.put_slice(GENESIS);

    section.put_slice(&[genesis_info.token_ticker.len() as u8]);
    section.put_slice(&genesis_info.token_ticker);

    section.put_slice(&[genesis_info.token_name.len() as u8]);
    section.put_slice(&genesis_info.token_name);

    section.put_slice(&[genesis_info.url.len() as u8]);
    section.put_slice(&genesis_info.url);

    section.put_slice(&[genesis_info.data.len() as u8]);
    section.put_slice(&genesis_info.data);

    section.put_slice(&[genesis_info.auth_pubkey.len() as u8]);
    section.put_slice(&genesis_info.auth_pubkey);

    section.put_slice(&[genesis_info.decimals]);
    put_mint_data(&mut section, mint_data);
    section.freeze()
}

pub fn mint_section(token_id: &TokenId, token_type: TokenType, mint_data: &MintData) -> Bytes {
    let mut section = BytesMut::new();
    section.put_slice(&SLPV2_LOKAD_ID);
    section.put_slice(&[token_type.to_u8()]);
    section.put_slice(&[MINT.len() as u8]);
    section.put_slice(MINT);
    section.put_slice(token_id.as_bytes());
    put_mint_data(&mut section, mint_data);
    
    section.freeze()
}

pub fn burn_section(token_id: &TokenId, token_type: TokenType, amount: Amount) -> Bytes {
    let mut section = BytesMut::new();
    section.put_slice(&SLPV2_LOKAD_ID);
    section.put_slice(&[token_type.to_u8()]);
    section.put_slice(&[BURN.len() as u8]);
    section.put_slice(BURN);
    section.put_slice(token_id.as_bytes());
    put_amount(&mut section, amount);
    section.freeze()
}

fn put_mint_data(section: &mut BytesMut, mint_data: &MintData) {
    section.put_slice(&[mint_data.amounts.len() as u8]);
    for &amount in &mint_data.amounts {
        put_amount(section, amount);
    }
    section.put_slice(&[mint_data.num_batons as u8]);
}

fn put_amount(section: &mut BytesMut, amount: Amount) {
    section.put_slice(&amount.int().to_le_bytes()[..6]);
}

pub fn send_section(
    token_id: &TokenId,
    token_type: TokenType,
    send_amounts: impl ExactSizeIterator<Item = Amount>,
) -> Bytes {
    let mut section = BytesMut::new();
    section.put_slice(&SLPV2_LOKAD_ID);
    section.put_slice(&[token_type.to_u8()]);
    section.put_slice(&[SEND.len() as u8]);
    section.put_slice(SEND);
    section.put_slice(token_id.as_bytes());

    section.put_slice(&[send_amounts.len() as u8]);
    for send_amount in send_amounts {
        put_amount(&mut section, send_amount);
    }
    section.freeze()
}

pub fn sections_opreturn(sections: Vec<Bytes>) -> Script {
    let mut script = ScriptMut::default();
    script.put_opcodes([OP_RETURN, OP_RESERVED]);
    for section in sections {
        script.put_op(Op::push_bytes(section));
    }
    script.freeze()
}
