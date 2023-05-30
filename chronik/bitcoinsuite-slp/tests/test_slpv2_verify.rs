use bitcoinsuite_slp::slpv2::{
    consts::MAX_TX_INPUTS, verify, ColoredTx, SectionData, SectionType,
    TokenBurn, TokenId, TokenMeta, TxData, Amount, BurnError,
};

#[test]
fn test_verify_slpv2_too_many_inputs() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TokenId::from([4; 32])),
            section_type: SectionType::SEND,
            genesis_info: None,
        }],
        ..Default::default()
    };
    // using MAX_TX_INPUTS is fine
    assert_eq!(
        verify(colored_tx.clone(), &vec![None; MAX_TX_INPUTS]),
        TxData {
            sections: vec![SectionData {
                meta: TokenMeta::standard(TokenId::from([4; 32])),
                section_type: SectionType::SEND,
                genesis_info: None,
            }],
            burns: vec![],
            inputs: vec![None; MAX_TX_INPUTS],
            outputs: vec![],
        },
    );
    // using MAX_TX_INPUTS + 1 burns all tokens
    assert_eq!(
        verify(colored_tx, &vec![None; MAX_TX_INPUTS + 1]),
        TxData {
            sections: vec![],
            burns: vec![TokenBurn {
                meta: TokenMeta::standard(TokenId::from([4; 32])),
                intentional_burn: None,
                actual_burn: Amount::ZERO,
                is_total: true,
                error: Some(BurnError::TooManyTxInputs(MAX_TX_INPUTS + 1)),
            }],
            inputs: vec![None; MAX_TX_INPUTS + 1],
            outputs: vec![],
        },
    );
}

#[test]
fn test_verify_slpv2_missing_mint_baton() {
    let colored_tx = ColoredTx {
        sections: vec![SectionData {
            meta: TokenMeta::standard(TokenId::from([4; 32])),
            section_type: SectionType::MINT,
            genesis_info: None,
        }],
        ..Default::default()
    };
}
