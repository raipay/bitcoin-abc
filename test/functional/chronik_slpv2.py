#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's /tx endpoint.
"""

from typing import List

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import GENESIS_CB_TXID, create_block, create_coinbase
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import OP_EQUAL, OP_HASH160, CScript, hash160, OP_RETURN, OP_RESERVED
from test_framework.test_framework import BitcoinTestFramework
from test_framework.util import assert_equal


def slpv2_genesis(
    token_ticker: bytes,
    token_name: bytes,
    url: bytes,
    data: bytes,
    auth_pubkey: bytes,
    decimals: int,
    mint_amounts: List[int],
    num_batons: int,
) -> bytes:
    result = bytearray()
    result.extend(b'SLP2')
    result.append(200)

    result.append(len(b'GENESIS'))
    result.extend(b'GENESIS')

    result.append(len(token_ticker))
    result.extend(token_ticker)

    result.append(len(token_name))
    result.extend(token_name)

    result.append(len(url))
    result.extend(url)

    result.append(len(data))
    result.extend(data)

    result.append(len(auth_pubkey))
    result.extend(auth_pubkey)

    result.append(decimals)

    result.append(len(mint_amounts))
    for amount in mint_amounts:
        result.extend(amount.to_bytes(6, 'little'))

    result.append(num_batons)
    return result


def slpv2_mint(
    token_id: bytes,
    input_baton_idx: int,
    mint_amounts: List[int],
    num_batons: int,
) -> bytes:
    result = bytearray()
    result.extend(b'SLP2')
    result.append(200)

    result.append(len(b'MINT'))
    result.extend(b'MINT')

    result.extend(token_id)

    result.append(input_baton_idx)

    result.append(len(mint_amounts))
    for amount in mint_amounts:
        result.extend(amount.to_bytes(6, 'little'))

    result.append(num_batons)

    return result


def slpv2_send(
    token_id: bytes,
    input_amounts: List[int],
    output_amounts: List[int],
    intentional_burn_amount = None
) -> bytes:
    result = bytearray()
    result.extend(b'SLP2')
    result.append(200)

    result.append(len(b'SEND'))
    result.extend(b'SEND')

    result.extend(token_id)

    result.append(len(input_amounts))
    for amount in input_amounts:
        result.extend(amount.to_bytes(6, 'little'))

    result.append(len(output_amounts))
    for amount in output_amounts:
        result.extend(amount.to_bytes(6, 'little'))

    if intentional_burn_amount is not None:
        result.append(len(b'BURN'))
        result.extend(b'BURN')
        result.extend(intentional_burn_amount.to_bytes(6, 'little'))

    return result


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
            CTxOut(0, CScript([OP_RETURN, OP_RESERVED, slpv2_genesis(
                token_ticker = b'TEST',
                token_name = b'Test Token',
                url = b'http://example.com',
                data = b'Token Data',
                auth_pubkey = b'Token Pubkey',
                decimals = 4,
                mint_amounts = [10, 20, 30, 0],
                num_batons = 2,
            )])),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(coinvalue - 100000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        genesis_txid = node.sendrawtransaction(genesis_tx.serialize().hex())
        print('genesis', chronik.tx(genesis_txid).ok())
        print('info', chronik.slpv2_token_info(genesis_txid).ok())

        genesis_tx = CTransaction()
        genesis_tx.vin = [CTxIn(outpoint=COutPoint(int(cointx, 16), 0),
                                scriptSig=SCRIPTSIG_OP_TRUE)]

        mint_tx = CTransaction()
        mint_tx.vin = [CTxIn(outpoint=COutPoint(int(genesis_txid, 16), 5),
                             scriptSig=SCRIPTSIG_OP_TRUE)]
        mint_tx.vout = [
            CTxOut(0, CScript([OP_RETURN, OP_RESERVED, slpv2_mint(
                token_id=bytes.fromhex(genesis_txid)[::-1],
                input_baton_idx = 0,
                mint_amounts = [5, 0],
                num_batons = 1,
            )])),
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
            CTxOut(0, CScript([OP_RETURN, OP_RESERVED, slpv2_send(
                token_id=bytes.fromhex(genesis_txid)[::-1],
                input_amounts=[10, 5],
                output_amounts=[3, 12],
            )])),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        send_txid = node.sendrawtransaction(send_tx.serialize().hex())
        print('send', chronik.tx(send_txid).ok())

        coinvalue = 5000000000
        genesis2_tx = CTransaction()
        genesis2_tx.vin = [CTxIn(outpoint=COutPoint(int(genesis_txid, 16), 4),
                                 scriptSig=SCRIPTSIG_OP_TRUE)]
        genesis2_tx.vout = [
            CTxOut(0, CScript([OP_RETURN, OP_RESERVED, slpv2_genesis(
                token_ticker = b'TEST2',
                token_name = b'Test Token 2',
                url = b'http://example.com/2',
                data = b'Token Data 2',
                auth_pubkey = b'Token Pubkey 2',
                decimals = 2,
                mint_amounts = [100],
                num_batons = 1,
            )])),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 200000, P2SH_OP_TRUE),
        ]

        genesis2_txid = node.sendrawtransaction(genesis2_tx.serialize().hex())
        print('genesis2', chronik.tx(genesis2_txid).ok())
        print('info', chronik.slpv2_token_info(genesis2_txid).ok())

        multi_tx = CTransaction()
        multi_tx.vin = [
            CTxIn(outpoint=COutPoint(int(send_txid, 16), 1),
                  scriptSig=SCRIPTSIG_OP_TRUE),
            CTxIn(outpoint=COutPoint(int(genesis2_txid, 16), 2),
                  scriptSig=SCRIPTSIG_OP_TRUE),
        ]
        multi_tx.vout = [
            CTxOut(0, CScript([
                OP_RETURN,
                OP_RESERVED,
                slpv2_genesis(
                    token_ticker = b'',
                    token_name = b'',
                    url = b'',
                    data = b'',
                    auth_pubkey = b'',
                    decimals = 3,
                    mint_amounts = [1000, 0],
                    num_batons = 1,
                ),
                slpv2_mint(
                    token_id=bytes.fromhex(genesis2_txid)[::-1],
                    input_baton_idx = 1,
                    mint_amounts = [0, 5, 0],
                    num_batons = 1,
                ),
                slpv2_send(
                    token_id=bytes.fromhex(genesis_txid)[::-1],
                    input_amounts=[3],
                    output_amounts=[0, 0, 0, 0, 1, 2],
                ),
            ])),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]

        print(multi_tx.vout[0].scriptPubKey)
        print(len(multi_tx.vout[0].scriptPubKey))

        multi_txid = node.sendrawtransaction(multi_tx.serialize().hex())
        print('multi', chronik.tx(multi_txid).ok())
        print('info', chronik.slpv2_token_info(multi_txid).ok())
        print('multi txid', multi_txid)

        self.generatetoaddress(node, 1, ADDRESS_ECREG_UNSPENDABLE)

        #print('**** MINED BLOCK')
        #print('multi', chronik.tx(multi_txid).ok())


        assert False

  

if __name__ == '__main__':
    ChronikTxTest().main()
