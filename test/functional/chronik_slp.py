#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's /tx endpoint.
"""

from typing import List, Optional

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import GENESIS_CB_TXID, create_block, create_coinbase
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import OP_EQUAL, OP_HASH160, CScript, hash160, OP_RETURN
from test_framework.test_framework import BitcoinTestFramework
from test_framework.util import assert_equal


def slp_genesis(
    token_ticker: bytes,
    token_name: bytes,
    token_document_url: bytes,
    token_document_hash: bytes,
    decimals: int,
    mint_baton_vout: Optional[int],
    initial_mint_amount: int,
) -> CScript:
    return CScript([
        OP_RETURN,
        b"SLP\0",
        b"\x01",
        b"GENESIS",
        token_ticker,
        token_name,
        token_document_url,
        token_document_hash,
        bytes([decimals]),
        bytes([mint_baton_vout]),
        initial_mint_amount.to_bytes(8, 'big'),
    ])


def slp_mint(
    token_id: bytes,
    mint_baton_vout: Optional[int],
    mint_amount: int,
) -> CScript:
    return CScript([
        OP_RETURN,
        b"SLP\0",
        b"\x01",
        b"MINT",
        token_id,
        bytes([mint_baton_vout]),
        mint_amount.to_bytes(8, 'big'),
    ])


def slp_send(
    token_id: bytes,
    amounts: List[int],
) -> CScript:
    ops = [
        OP_RETURN,
        b"SLP\0",
        b"\x01",
        b"SEND",
        token_id,
    ]
    for amount in amounts:
        ops.append(amount.to_bytes(8, 'big'))
    return CScript(ops)


class ChronikTxTest(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [['-chronik']]

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        from test_framework.chronik.client import ChronikClient, pb
        from test_framework.chronik.test_data import genesis_cb_tx

        node = self.nodes[0]
        chronik = ChronikClient('127.0.0.1', node.chronik_port)

        peer = node.add_p2p_connection(P2PDataStore())
        node.setmocktime(1333333337)

        coinblockhash = self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        coinblock = node.getblock(coinblockhash)
        cointx = coinblock['tx'][0]

        self.generatetoaddress(node, 100, ADDRESS_ECREG_UNSPENDABLE)

        coinvalue = 5000000000
        genesis_tx = CTransaction()
        genesis_tx.vin = [CTxIn(outpoint=COutPoint(int(cointx, 16), 0),
                                scriptSig=SCRIPTSIG_OP_TRUE)]
        genesis_tx.vout = [
            CTxOut(0, slp_genesis(
                token_ticker = b'TEST',
                token_name = b'Test Token',
                token_document_url = b'http://example.com',
                token_document_hash = b'T' * 32,
                decimals = 2,
                mint_baton_vout = 5,
                initial_mint_amount = 1000,
            )),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(coinvalue - 100000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        print('validate genesis', chronik.validate_tx(genesis_tx.serialize()).ok())
        genesis_txid = node.sendrawtransaction(genesis_tx.serialize().hex())
        print('genesis', chronik.tx(genesis_txid).ok())
        #print('info', chronik.slpv2_token_info(genesis_txid).ok())

        mint_tx = CTransaction()
        mint_tx.vin = [CTxIn(outpoint=COutPoint(int(genesis_txid, 16), 5),
                             scriptSig=SCRIPTSIG_OP_TRUE)]
        mint_tx.vout = [
            CTxOut(0, slp_mint(
                token_id=bytes.fromhex(genesis_txid),
                mint_baton_vout = 2,
                mint_amount = 200,
            )),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        mint_txid = node.sendrawtransaction(mint_tx.serialize().hex())
        print('mint', chronik.tx(mint_txid).ok())

        send_tx = CTransaction()
        send_tx.vin = [
            CTxIn(outpoint=COutPoint(int(genesis_txid, 16), 1),
                  scriptSig=SCRIPTSIG_OP_TRUE),
            CTxIn(outpoint=COutPoint(int(mint_txid, 16), 1),
                  scriptSig=SCRIPTSIG_OP_TRUE),
        ]
        send_tx.vout = [
            CTxOut(0, slp_send(
                token_id=bytes.fromhex(genesis_txid),
                amounts = [300, 400, 100],
            )),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        send_txid = node.sendrawtransaction(send_tx.serialize().hex())
        print('send', chronik.tx(send_txid).ok())

        self.generatetoaddress(node, 1, ADDRESS_ECREG_UNSPENDABLE)

        #print('**** MINED BLOCK')
        #print('multi', chronik.tx(multi_txid).ok())

        print('send', chronik.tx(send_txid).ok())

        assert False

  

if __name__ == '__main__':
    ChronikTxTest().main()
