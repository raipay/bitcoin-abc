use bitcoinsuite_core::{error::DataError, tx::TxId};
use bitcoinsuite_slp::slpv2::{
    parse_section, Amount, Genesis, GenesisInfo, MintData, ParseError, Section,
    SectionVariant, Send, TokenId, TokenMeta, TokenType,
};

const SLP2: &[u8] = b"SLP2";
const TXID: TxId = TxId::new([4; 32]);

fn parse(data: Vec<u8>) -> Result<Section, ParseError> {
    parse_section(&TXID, data.into())
}

fn invalid_len(expected: usize, actual: usize) -> Result<Section, ParseError> {
    Err(ParseError::DataError(DataError::InvalidLength {
        expected,
        actual,
    }))
}

#[test]
fn test_parse_slpv2_intro() {
    assert_eq!(
        parse(vec![]),
        Err(ParseError::MissingLokadId(vec![].into())),
    );
    assert_eq!(
        parse(vec![1, 2, 3]),
        Err(ParseError::MissingLokadId(vec![1, 2, 3].into())),
    );
    assert_eq!(
        parse(vec![1, 2, 3, 4]),
        Err(ParseError::InvalidLokadId([1, 2, 3, 4])),
    );
    assert_eq!(parse(SLP2.to_vec()), invalid_len(1, 0));
    assert_eq!(
        parse([SLP2, &[99]].concat()),
        Ok(Section {
            meta: TokenMeta {
                token_id: TokenId::from([0; 32]),
                token_type: TokenType::Unknown(99),
            },
            variant: SectionVariant::Unknown
        }),
    );
    assert_eq!(parse([SLP2, &[0]].concat()), invalid_len(1, 0));
    assert_eq!(parse([SLP2, &[0], &[99]].concat()), invalid_len(99, 0));
    assert_eq!(
        parse([SLP2, &[0], &[0]].concat()),
        Err(ParseError::UnknownTxType(vec![].into())),
    );
    assert_eq!(
        parse([SLP2, &[0], &[4], b"bork"].concat()),
        Err(ParseError::UnknownTxType(b"bork".to_vec().into())),
    );
}

#[test]
fn test_parse_slpv2_genesis() {
    // missing token_ticker
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS"].concat()),
        invalid_len(1, 0),
    );
    // token_ticker missing 99 bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[99]].concat()),
        invalid_len(99, 0),
    );
    // missing token_name
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0]].concat()),
        invalid_len(1, 0),
    );
    // token_name missing 99 bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0], &[99]].concat()),
        invalid_len(99, 0),
    );
    // missing url
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0]].concat()),
        invalid_len(1, 0),
    );
    // url missing 99 bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0], &[99]].concat()),
        invalid_len(99, 0),
    );
    // missing token data
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0]].concat()),
        invalid_len(1, 0),
    );
    // token data missing 99 bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0], &[99]].concat()),
        invalid_len(99, 0),
    );
    // missing auth_pubkey
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0]].concat()),
        invalid_len(1, 0),
    );
    // auth_pubkey missing 99 bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0], &[99]].concat()),
        invalid_len(99, 0),
    );
    // missing decimals
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0]].concat()),
        invalid_len(1, 0),
    );
    // missing num amounts
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, 0]].concat()),
        invalid_len(1, 0),
    );
    // missing num batons
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, 0, 0]].concat()),
        invalid_len(1, 0),
    );
    // parsing default empty Genesis succeeded
    assert_eq!(
        parse([SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, 0, 0, 0]].concat()),
        Ok(Section {
            meta: TokenMeta {
                token_id: TokenId::from(TXID),
                token_type: TokenType::Standard,
            },
            variant: SectionVariant::Genesis(Genesis::default()),
        }),
    );
    // leftover bytes
    assert_eq!(
        parse(
            [
                SLP2,
                &[0],
                b"\x07GENESIS",
                &[0, 0, 0, 0, 0, 0, 0, 0],
                b"hello"
            ]
            .concat()
        ),
        Err(ParseError::LeftoverBytes(b"hello".to_vec().into())),
    );
    // any size > 127 for token_ticker, token_name, url, data, auth_pubkey, num
    // amounts and num batons is invalid
    for i in [0, 1, 2, 3, 4, 6, 7] {
        let mut tail = [0, 0, 0, 0, 0, 0, 0, 0];
        for invalid in 128..=255 {
            tail[i] = invalid;
            assert_eq!(
                parse([SLP2, &[0], b"\x07GENESIS", &tail].concat()),
                Err(ParseError::SizeOutOfRange(invalid)),
            );
        }
    }
    // any decimals > 9 is invalid
    for invalid in 10..=255 {
        assert_eq!(
            parse(
                [SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, invalid, 0, 0]]
                    .concat()
            ),
            Err(ParseError::DecimalsOutOfRange(invalid)),
        );
    }
    let meta = TokenMeta {
        token_id: TokenId::from(TXID),
        token_type: TokenType::Standard,
    };
    // exhaustively test all allowed sizes for token_ticker, token_name, url,
    // data, auth_pubkey
    for i in 0..5 {
        for size in 0..=127 {
            let mut pushdata = Vec::new();
            pushdata.extend([SLP2, &[0], b"\x07GENESIS"].concat());
            pushdata.extend(vec![0; i]);
            pushdata.push(size as u8);
            pushdata.extend(vec![0x76; size]);
            pushdata.extend(vec![0; 4 - i]);
            pushdata.extend([0, 0, 0]);
            let mut info = GenesisInfo::default();
            let GenesisInfo {
                token_ticker,
                token_name,
                url,
                data,
                auth_pubkey,
                ..
            } = &mut info;
            let fields = [token_ticker, token_name, url, data, auth_pubkey];
            *fields[i] = vec![0x76; size].into();
            assert_eq!(
                parse(pushdata),
                Ok(Section {
                    meta,
                    variant: SectionVariant::Genesis(Genesis {
                        data: info,
                        mint_data: MintData::default(),
                    }),
                }),
            );
        }
    }
    for size in 0..=127 {
        let mut pushdata =
            [SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, 0]].concat();
        pushdata.push(size as u8);
        let mut amounts = Vec::with_capacity(size);
        for i in 0..size {
            pushdata.push(i as u8);
            pushdata.extend([0; 5]);
            amounts.push(Amount::new(i as i64));
        }
        pushdata.push(0);
        assert_eq!(
            parse(pushdata),
            Ok(Section {
                meta,
                variant: SectionVariant::Genesis(Genesis {
                    data: GenesisInfo::default(),
                    mint_data: MintData {
                        amounts,
                        num_batons: 0,
                    },
                }),
            }),
        );
    }
    for size in 0..=127 {
        let mut pushdata =
            [SLP2, &[0], b"\x07GENESIS", &[0, 0, 0, 0, 0, 0, 0]].concat();
        pushdata.push(size);
        assert_eq!(
            parse(pushdata),
            Ok(Section {
                meta,
                variant: SectionVariant::Genesis(Genesis {
                    data: GenesisInfo::default(),
                    mint_data: MintData {
                        amounts: vec![],
                        num_batons: size as usize,
                    },
                }),
            }),
        );
    }
    // valid example
    assert_eq!(
        parse(
            [
                SLP2,
                &[0],
                b"\x07GENESIS",
                b"\x0212",
                b"\x03345",
                b"\x046789",
                b"\x05abcde",
                b"\x06fghijk",
                &[7],
                &[3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8],
                &[4],
            ]
            .concat(),
        ),
        Ok(Section {
            meta,
            variant: SectionVariant::Genesis(Genesis {
                data: GenesisInfo {
                    token_ticker: b"12".to_vec().into(),
                    token_name: b"345".to_vec().into(),
                    url: b"6789".to_vec().into(),
                    data: b"abcde".to_vec().into(),
                    auth_pubkey: b"fghijk".to_vec().into(),
                    decimals: 7,
                },
                mint_data: MintData {
                    amounts: vec![
                        Amount::new(0x60504030201),
                        Amount::new(0x20100090807),
                        Amount::new(0x80706050403),
                    ],
                    num_batons: 4,
                },
            }),
        }),
    );
}

#[test]
fn test_parse_slpv2_mint() {
    for size in 0..=31 {
        assert_eq!(
            parse([SLP2, &[0], b"\x04MINT", &vec![0; size]].concat()),
            invalid_len(32, size),
        );
    }
    assert_eq!(
        parse([SLP2, &[0], b"\x04MINT", &[0x76; 32]].concat()),
        invalid_len(1, 0),
    );
    // any size > 127 for num amounts and num batons is invalid
    for i in [0, 1] {
        let mut tail = [0, 0];
        for invalid in 128..=255 {
            tail[i] = invalid;
            assert_eq!(
                parse([SLP2, &[0], b"\x04MINT", &[0x76; 32], &tail].concat()),
                Err(ParseError::SizeOutOfRange(invalid)),
            );
        }
    }
    let meta = TokenMeta {
        token_id: TokenId::from([0x76; 32]),
        token_type: TokenType::Standard,
    };
    assert_eq!(
        parse([SLP2, &[0], b"\x04MINT", &[0x76; 32], &[0, 0]].concat()),
        Ok(Section {
            meta,
            variant: SectionVariant::Mint(MintData::default()),
        }),
    );
    // leftover bytes
    assert_eq!(
        parse(
            [SLP2, &[0], b"\x04MINT", &[0x76; 32], &[0, 0], b"hello"].concat()
        ),
        Err(ParseError::LeftoverBytes(b"hello".to_vec().into())),
    );
    for size in 0..=127 {
        let mut pushdata = [SLP2, &[0], b"\x04MINT", &[0x76; 32]].concat();
        pushdata.push(size as u8);
        let mut amounts = Vec::with_capacity(size);
        for i in 0..size {
            pushdata.push(i as u8);
            pushdata.extend([0; 5]);
            amounts.push(Amount::new(i as i64));
        }
        pushdata.push(0);
        assert_eq!(
            parse(pushdata),
            Ok(Section {
                meta,
                variant: SectionVariant::Mint(MintData {
                    amounts,
                    num_batons: 0,
                }),
            }),
        );
    }
    for size in 0..=127 {
        let mut pushdata =
            [SLP2, &[0], b"\x04MINT", &[0x76; 32], &[0]].concat();
        pushdata.push(size);
        assert_eq!(
            parse(pushdata),
            Ok(Section {
                meta,
                variant: SectionVariant::Mint(MintData {
                    amounts: vec![],
                    num_batons: size as usize,
                }),
            }),
        );
    }
    assert_eq!(
        parse(
            [
                SLP2,
                &[0],
                b"\x04MINT",
                &[0x76; 32],
                &[3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8],
                &[4],
            ]
            .concat()
        ),
        Ok(Section {
            meta: TokenMeta {
                token_id: TokenId::from([0x76; 32]),
                token_type: TokenType::Standard,
            },
            variant: SectionVariant::Mint(MintData {
                amounts: vec![
                    Amount::new(0x60504030201),
                    Amount::new(0x20100090807),
                    Amount::new(0x80706050403),
                ],
                num_batons: 4,
            }),
        }),
    );
}

#[test]
fn test_parse_slpv2_send() {
    for size in 0..=31 {
        assert_eq!(
            parse([SLP2, &[0], b"\x04SEND", &vec![0; size]].concat()),
            invalid_len(32, size),
        );
    }
    assert_eq!(
        parse([SLP2, &[0], b"\x04SEND", &[0x76; 32]].concat()),
        invalid_len(1, 0),
    );
    // any size > 127 for num amounts is invalid
    for invalid in 128..=255 {
        assert_eq!(
            parse([SLP2, &[0], b"\x04SEND", &[0x76; 32], &[invalid]].concat()),
            Err(ParseError::SizeOutOfRange(invalid)),
        );
    }
    let meta = TokenMeta {
        token_id: TokenId::from([0x76; 32]),
        token_type: TokenType::Standard,
    };
    assert_eq!(
        parse([SLP2, &[0], b"\x04SEND", &[0x76; 32], &[0]].concat()),
        Ok(Section {
            meta,
            variant: SectionVariant::Send(Send(vec![])),
        }),
    );
    // leftover bytes
    assert_eq!(
        parse([SLP2, &[0], b"\x04SEND", &[0x76; 32], &[0], b"hello"].concat()),
        Err(ParseError::LeftoverBytes(b"hello".to_vec().into())),
    );
    for size in 0..=127 {
        let mut pushdata = [SLP2, &[0], b"\x04SEND", &[0x76; 32]].concat();
        pushdata.push(size as u8);
        let mut amounts = Vec::with_capacity(size);
        for i in 0..size {
            pushdata.push(i as u8);
            pushdata.extend([0; 5]);
            amounts.push(Amount::new(i as i64));
        }
        assert_eq!(
            parse(pushdata),
            Ok(Section {
                meta,
                variant: SectionVariant::Send(Send(amounts)),
            }),
        );
    }
}

#[test]
fn test_parse_slpv2_burn() {
    for size in 0..=31 {
        assert_eq!(
            parse([SLP2, &[0], b"\x04BURN", &vec![0; size]].concat()),
            invalid_len(32, size),
        );
    }
    for size in 0..=5 {
        assert_eq!(
            parse(
                [SLP2, &[0], b"\x04BURN", &[0x76; 32], &vec![0; size]].concat()
            ),
            invalid_len(6, size),
        );
    }
    assert_eq!(
        parse(
            [SLP2, &[0], b"\x04BURN", &[0x76; 32], &[1, 2, 3, 4, 5, 6]]
                .concat()
        ),
        Ok(Section {
            meta: TokenMeta {
                token_id: TokenId::from([0x76; 32]),
                token_type: TokenType::Standard,
            },
            variant: SectionVariant::Burn(Amount::new(0x60504030201)),
        }),
    );
    // leftover bytes
    assert_eq!(
        parse(
            [SLP2, &[0], b"\x04BURN", &[0x76; 32], &[0; 6], b"hello"].concat()
        ),
        Err(ParseError::LeftoverBytes(b"hello".to_vec().into())),
    );
}
