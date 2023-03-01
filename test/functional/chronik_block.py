#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

import http.client

from test_framework.address import ADDRESS_ECREG_P2SH_OP_TRUE
from test_framework.blocktools import GENESIS_BLOCK_HASH, TIME_GENESIS_BLOCK
from test_framework.test_framework import BitcoinTestFramework
from test_framework.util import assert_equal


class ChronikServeTest(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [['-chronik']]

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        import chronik_pb2 as pb

        def query_block(block_height):
            client = http.client.HTTPConnection('127.0.0.1', 18442, timeout=4)
            client.request('GET', f'/block/{block_height}')
            response = client.getresponse()
            assert_equal(response.getheader('Content-Type'),
                         'application/x-protobuf')
            return response

        # Expected genesis block
        genesis_block = pb.Block(
            block_info=pb.BlockInfo(
                hash=bytes.fromhex(GENESIS_BLOCK_HASH)[::-1],
                prev_hash=bytes(32),
                n_bits=0x207fffff,
                timestamp=TIME_GENESIS_BLOCK,
            ),
        )

        # Query genesis block
        response = query_block(0)
        assert_equal(response.status, 200)
        proto_block = pb.Block()
        proto_block.ParseFromString(response.read())
        assert_equal(proto_block, genesis_block)

        # Block 1 not found
        response = query_block(1)
        assert_equal(response.status, 404)
        proto_error = pb.Error()
        proto_error.ParseFromString(response.read())
        assert_equal(proto_error.msg, '404: Block not found: 1')

        # Generate 100 blocks, verify they form a chain
        node = self.nodes[0]
        self.generatetoaddress(node, 100, ADDRESS_ECREG_P2SH_OP_TRUE)
        block_hashes = [genesis_block.block_info.hash]
        for i in range(1, 101):
            response = query_block(i)
            assert_equal(response.status, 200)
            proto_block = pb.Block()
            proto_block.ParseFromString(response.read())
            assert_equal(proto_block.block_info.prev_hash, block_hashes[-1])
            block_hashes.append(proto_block.block_info.hash)

        # Invalidate in the middle of the chain
        node.invalidateblock(block_hashes[50][::-1].hex())
        # Gives 404 for the invalidated blocks
        for i in range(50, 101):
            response = query_block(i)
            assert_equal(response.status, 404)
            proto_error = pb.Error()
            proto_error.ParseFromString(response.read())
            assert_equal(proto_error.msg, f'404: Block not found: {i}')
        # Previous blocks are still fine
        for i in range(0, 50):
            response = query_block(i)
            assert_equal(response.status, 200)


if __name__ == '__main__':
    ChronikServeTest().main()
