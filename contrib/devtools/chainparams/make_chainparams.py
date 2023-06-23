#!/usr/bin/env python3
# Copyright (c) 2019 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

import argparse
import math
import os.path
import re
import sys
from enum import Enum

sys.path.append("../../../test/functional/test_framework")
from authproxy import AuthServiceProxy  # noqa: E402


class Chain(Enum):
    MainNet = "MAINNET"
    TestNet = "TESTNET"


GIGABYTE = 1024 * 1024 * 1024


def get_chainparams(rpc_caller, block):
    # Fetch initial chain info
    chaininfo = rpc_caller.getblockchaininfo()
    if chaininfo["chain"] == "main":
        chain = Chain.MainNet
    else:
        chain = Chain.TestNet

    # Use highest valid chainwork. This doesn't need to match the block hash
    # used by assume valid.
    chainwork = chaininfo["chainwork"]
    if not re.match("^[0-9a-z]{64}$", chainwork):
        raise Exception("Chain work is not a valid uint256 hex value.")

    # Default to N blocks from the chain tip, depending on which chain we're on
    if not block:
        block = chaininfo["blocks"]
        if chain == Chain.MainNet:
            block -= 10
        else:
            block -= 2000

    block = str(block)
    if not re.match("^[0-9a-z]{64}$", block):
        if re.match("^[0-9]*$", block):
            # Fetch block hash using block height
            block = rpc_caller.getblockhash(int(block))
        else:
            raise Exception("Block hash is not a valid block hash or height.")

    # Make sure the block hash is part of the chain. This call with raise an
    # exception if not.
    rpc_caller.getblockheader(block)

    # Block size on disk (in GB) with some margin for growth
    diskSizeBlocks = str(int(math.ceil(chaininfo["size_on_disk"] / GIGABYTE * 1.3)))

    # Chainstate size on disk (in GB) with some margin for growth
    utxos = rpc_caller.gettxoutsetinfo()
    diskSizeChainstate = str(int(math.ceil(utxos["disk_size"] / GIGABYTE * 1.3)))

    return (block, chainwork, diskSizeBlocks, diskSizeChainstate)


def main(args):
    return "\n".join(get_chainparams(args["rpc"], args["block"]))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description=(
            "Make chainparams file.\nPrerequisites: RPC access to a bitcoind node.\n\n"
        ),
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument(
        "--address",
        "-a",
        default="127.0.0.1:8332",
        help=(
            "Node address for making RPC calls.\n"
            "The chain (MainNet or TestNet) will be automatically detected.\n"
            "Default: '127.0.0.1:8332'"
        ),
    )
    parser.add_argument(
        "--block",
        "-b",
        help=(
            "The block hash or height to use for fetching chainparams.\n"
            "MainNet default: 10 blocks from the chain tip."
            "TestNet default: 2000 blocks from the chain tip."
        ),
    )
    parser.add_argument(
        "--config",
        "-c",
        default="~/.bitcoin/bitcoin.conf",
        help=(
            "Path to bitcoin.conf for RPC authentication arguments (rpcuser &"
            " rpcpassword).\nDefault: ~/.bitcoin/bitcoin.conf"
        ),
    )
    args = parser.parse_args()
    args.config = os.path.expanduser(args.config)

    # Get user and password from config
    user = None
    password = None
    if os.path.isfile(args.config):
        with open(args.config, "r", encoding="utf8") as f:
            for line in f:
                if line.startswith("rpcuser="):
                    # Ensure that there is only one rpcuser line
                    assert user is None
                    user = line.split("=")[1].strip("\n")
                if line.startswith("rpcpassword="):
                    # Ensure that there is only one rpcpassword line
                    assert password is None
                    password = line.split("=")[1].strip("\n")
    else:
        raise FileNotFoundError("Missing bitcoin.conf")
    if user is None:
        raise ValueError("Config is missing rpcuser")
    if password is None:
        raise ValueError("Config is missing rpcpassword")

    args.rpc = AuthServiceProxy(
        service_url=f"http://{user}:{password}@{args.address}", timeout=1200
    )
    output = main(vars(args))
    if output:
        print(output)
