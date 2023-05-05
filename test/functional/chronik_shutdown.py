#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik properly shuts down.
"""

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import GENESIS_BLOCK_HASH, create_block, create_coinbase
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut, msg_block
from test_framework.p2p import P2PDataStore
from test_framework.test_framework import BitcoinTestFramework
from test_framework.txtools import pad_tx
from test_framework.util import assert_equal, iter_chunks
from test_framework.script import CScript, OP_RETURN, OP_NOP


class ChronikShutdown(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 10
        self.extra_args = [["-chronik"]] * 10
        self.rpc_timeout = 240

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        from test_framework.chronik.client import ChronikClient, pb
        from test_framework.chronik.test_data import genesis_cb_tx

        mocktime = 1300000000
        peers = []
        for node in self.nodes:
            node.setmocktime(mocktime)
            peers.append(node.add_p2p_connection(P2PDataStore()))

        last_block_hash = GENESIS_BLOCK_HASH
        blocks = []
        for i in range(1, 102):
            coinbase_tx = create_coinbase(i)
            coinbase_tx.vout[0].scriptPubKey = CScript([OP_RETURN] + [OP_NOP] * 950_000)
            coinbase_tx.rehash()
            block = create_block(int(last_block_hash, 16), coinbase_tx, mocktime + i)
            block.solve()
            blocks.append(block)
            last_block_hash = block.hash
            for peer in peers:
                peer.send_message(msg_block(block))

        # Wait until the last node has processed the first 10 blocks, then shut all down
        self.wait_until(lambda: self.nodes[-1].getblockcount() >= 10,
                        timeout=60)
        self.stop_nodes()


if __name__ == "__main__":
    ChronikShutdown().main()
