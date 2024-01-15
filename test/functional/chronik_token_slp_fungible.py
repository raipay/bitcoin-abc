#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's SLP + ALP integration.
"""

from typing import List, Optional

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import (
    create_block,
    create_coinbase,
    make_conform_to_ctor,
)
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import (
    OP_12,
    OP_EQUAL,
    OP_HASH160,
    OP_RESERVED,
    OP_RETURN,
    CScript,
    hash160,
)
from test_framework.test_framework import BitcoinTestFramework
from test_framework.txtools import pad_tx
from test_framework.util import assert_equal
from test_framework.chronik.slp import slp_genesis, slp_mint, slp_send
from test_framework.chronik.token_tx import TokenTx


class ChronikTokenSlpFungible(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [["-chronik"]]

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        from test_framework.chronik.client import ChronikClient, pb

        def slp_token(token_type=None, **kwargs) -> pb.Token:
            return pb.Token(
                token_type=token_type or pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                **kwargs,
            )

        node = self.nodes[0]
        chronik = ChronikClient("127.0.0.1", node.chronik_port)

        peer = node.add_p2p_connection(P2PDataStore())
        mocktime = 1300000000
        node.setmocktime(mocktime)

        coinblockhash = self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        coinblock = node.getblock(coinblockhash)
        cointx = coinblock["tx"][0]

        block_hashes = self.generatetoaddress(node, 100, ADDRESS_ECREG_UNSPENDABLE)

        coinvalue = 5000000000

        txs = []

        tx = CTransaction()
        tx.vin = [
            CTxIn(COutPoint(int(cointx, 16), 0), SCRIPTSIG_OP_TRUE)
        ]
        tx.vout = [
            CTxOut(
                0,
                slp_genesis(
                    token_type=pb.SLP_TOKEN_TYPE_FUNGIBLE,
                    token_ticker=b"SLPTEST",
                    token_name=b"Test SLP Token 3",
                    token_document_url=b"http://example/slp",
                    token_document_hash=b"x" * 32,
                    decimals=4,
                    mint_baton_vout=2,
                    initial_mint_amount=5000,
                ),
            ),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 400000, P2SH_OP_TRUE),
        ]
        tx.rehash()
        genesis = TokenTx(
            tx=tx,
            entries=[
                pb.TokenEntry(
                    token_id=tx.hash,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                    tx_type=pb.GENESIS,
                    actual_burn_amount="0",
                ),
            ],
            inputs=[pb.Token()],
            outputs=[
                pb.Token(),
                pb.Token(
                    token_id=tx.hash,
                    amount=5000,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                ),
                pb.Token(
                    token_id=tx.hash,
                    is_mint_baton=True,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                ),
                pb.Token(),
            ],
        )
        txs.append(genesis)
        genesis.send(node)
        genesis.test(chronik)

        tx = CTransaction()
        tx.vin = [
            CTxIn(COutPoint(int(genesis.txid, 16), 2), SCRIPTSIG_OP_TRUE)
        ]
        tx.vout = [
            CTxOut(
                0,
                slp_mint(
                    token_type=pb.SLP_TOKEN_TYPE_FUNGIBLE,
                    token_id=genesis.txid,
                    mint_baton_vout=3,
                    mint_amount=20,
                ),
            ),
            CTxOut(2000, P2SH_OP_TRUE),
            CTxOut(2000, P2SH_OP_TRUE),
            CTxOut(2000, P2SH_OP_TRUE),
        ]
        mint = TokenTx(
            tx=tx,
            entries=[
                pb.TokenEntry(
                    token_id=genesis.txid,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                    tx_type=pb.MINT,
                    actual_burn_amount="0",
                ),
            ],
            inputs=[slp_token(token_id=genesis.txid, is_mint_baton=True)],
            outputs=[
                pb.Token(),
                slp_token(token_id=genesis.txid, amount=20),
                pb.Token(),
                slp_token(token_id=genesis.txid, is_mint_baton=True),
            ],
        )
        txs.append(mint)
        mint.send(node)
        mint.test(chronik)

        tx = CTransaction()
        tx.vin = [
            CTxIn(COutPoint(int(genesis.txid, 16), 1), SCRIPTSIG_OP_TRUE)
        ]
        tx.vout = [
            CTxOut(0, slp_send(
                token_type=pb.SLP_TOKEN_TYPE_FUNGIBLE,
                token_id=genesis.txid,
                amounts=[1000, 4000],
            )),
            CTxOut(4000, P2SH_OP_TRUE),
            CTxOut(4000, P2SH_OP_TRUE),
        ]
        send = TokenTx(
            tx=tx,
            entries=[
                pb.TokenEntry(
                    token_id=genesis.txid,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                    tx_type=pb.SEND,
                    actual_burn_amount="0",
                ),
            ],
            inputs=[
                slp_token(token_id=genesis.txid, amount=5000),
            ],
            outputs=[
                pb.Token(),
                slp_token(token_id=genesis.txid, amount=1000),
                slp_token(token_id=genesis.txid, amount=4000),
            ],
        )
        txs.append(send)
        send.send(node)
        send.test(chronik)

        # SLP GENESIS with empty GenesisInfo
        tx = CTransaction()
        tx.vin = [
            CTxIn(COutPoint(int(genesis.txid, 16), 3), SCRIPTSIG_OP_TRUE)
        ]
        tx.vout = [
            CTxOut(
                0,
                slp_genesis(
                    token_type=pb.SLP_TOKEN_TYPE_FUNGIBLE,
                    mint_baton_vout=None,
                    initial_mint_amount=0,
                ),
            ),
            CTxOut(coinvalue - 500000, P2SH_OP_TRUE),
        ]
        tx.rehash()
        genesis_empty = TokenTx(
            tx=tx,
            entries=[
                pb.TokenEntry(
                    token_id=tx.hash,
                    token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                    tx_type=pb.GENESIS,
                    actual_burn_amount="0",
                ),
            ],
            inputs=[pb.Token()],
            outputs=[
                pb.Token(),
                pb.Token(),
            ],
        )
        txs.append(genesis_empty)
        genesis_empty.send(node)
        genesis_empty.test(chronik)

        # After mining, all txs still work fine
        block_hash = self.generatetoaddress(node, 1, ADDRESS_ECREG_UNSPENDABLE)[0]
        for tx in txs:
            tx.test(chronik, block_hash)

        # Undo block + test again
        node.invalidateblock(block_hash)
        for tx in txs:
            tx.test(chronik)


if __name__ == "__main__":
    ChronikTokenSlpFungible().main()
