#!/usr/bin/env python3
# Copyright (c) 2024 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

from typing import Optional, List
import itertools

from test_framework.messages import CTransaction
from test_framework.util import assert_equal
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.script import (
    OP_12,
    OP_EQUAL,
    OP_HASH160,
    OP_RESERVED,
    OP_RETURN,
    CScript,
    hash160,
)


class TokenTx:
    def __init__(self, *, tx: CTransaction, entries=[], inputs=[], outputs=[]):
        tx.rehash()
        self.tx = tx
        self.txid = tx.hash
        self.entries = entries
        self.inputs = inputs
        self.outputs = outputs

    def send(self, node):
        node.sendrawtransaction(self.tx.serialize().hex())

    def test(self, chronik, block_hash=None):
        import chronik_pb2 as pb
        proto_tx = chronik.tx(self.txid).ok()
        assert_equal(list(proto_tx.token_entries), self.entries)
        assert_equal(
            [tx_input.token for tx_input in proto_tx.inputs], self.inputs
        )
        assert_equal(
            [tx_output.token for tx_output in proto_tx.outputs], self.outputs
        )
        if block_hash is None:
            assert_equal(proto_tx.block, pb.BlockMetadata())
        else:
            assert_equal(proto_tx.block.hash, bytes.fromhex(block_hash)[::-1])
