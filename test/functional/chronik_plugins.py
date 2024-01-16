#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's plugin interface.
"""

import os
import os.path

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import OP_RETURN, CScript
from test_framework.test_framework import BitcoinTestFramework
from test_framework.util import assert_equal


class ChronikPluginsTest(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [["-chronik"]]
        self.rpc_timeout = 240

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        from test_framework.chronik.client import pb

        node = self.nodes[0]

        plugins_dir = os.path.join(node.datadir, "plugins")
        os.mkdir(plugins_dir)

        plugin_file = open(
            os.path.join(plugins_dir, "test_plugin.py"), "w", encoding="utf-8"
        )
        plugin_file.write(
            """
from chronik_plugin.tx import Tx
__lokad_id__ = b"RTST"
__version__ = "0.1"
def __plugin_outputs__(tx: Tx):
    return [
        {"idx": 1, "data": [b"payload"], "group": b"group"}
    ]
"""
        )
        plugin_file.close()

        self.restart_node(0)

        chronik = node.get_chronik_client()

        node.add_p2p_connection(P2PDataStore())
        node.setmocktime(1300000000)

        # Verify queried genesis tx matches
        coinblockhash = self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        coinblock = node.getblock(coinblockhash)
        cointx = coinblock["tx"][0]

        self.generatetoaddress(node, 100, ADDRESS_ECREG_UNSPENDABLE)

        coinvalue = 5000000000
        tx = CTransaction()
        tx.vin = [
            CTxIn(
                outpoint=COutPoint(int(cointx, 16), 0),
                scriptSig=SCRIPTSIG_OP_TRUE,
            )
        ]
        tx.vout = [
            CTxOut(0, CScript([OP_RETURN, b"RTST"])),
            CTxOut(coinvalue - 10000, P2SH_OP_TRUE),
        ]
        tx.nLockTime = 0

        # Submit tx to mempool
        txid = node.sendrawtransaction(tx.serialize().hex())

        proto_tx = chronik.tx(txid).ok()
        assert_equal(
            proto_tx.outputs[1].plugins,
            {"test_plugin": pb.PluginEntry(groups=[b"group"], data=[b"payload"])},
        )
        assert_equal(
            chronik.plugin("test_plugin", b"group".hex()).history().ok(),
            pb.TxHistoryPage(
                txs=[proto_tx],
                num_pages=1,
                num_txs=1,
            ),
        )
        assert_equal(
            chronik.plugin("test_plugin", b"group".hex()).utxos().ok(),
            pb.Utxos(
                utxos=[
                    pb.Utxo(
                        outpoint=pb.OutPoint(txid=bytes.fromhex(txid)[::-1], out_idx=1),
                        block_height=-1,
                        value=coinvalue - 10000,
                        script=bytes(P2SH_OP_TRUE),
                        plugins={
                            "test_plugin": pb.PluginEntry(
                                groups=[b"group"], data=[b"payload"]
                            )
                        },
                    )
                ]
            ),
        )

        self.generatetoaddress(node, 1, ADDRESS_ECREG_UNSPENDABLE)

        proto_tx = chronik.tx(txid).ok()
        assert_equal(
            proto_tx.outputs[1].plugins,
            {"test_plugin": pb.PluginEntry(groups=[b"group"], data=[b"payload"])},
        )
        assert_equal(
            chronik.plugin("test_plugin", b"group".hex()).history().ok(),
            pb.TxHistoryPage(
                txs=[proto_tx],
                num_pages=1,
                num_txs=1,
            ),
        )
        assert_equal(
            chronik.plugin("test_plugin", b"group".hex()).utxos().ok(),
            pb.Utxos(
                utxos=[
                    pb.Utxo(
                        outpoint=pb.OutPoint(txid=bytes.fromhex(txid)[::-1], out_idx=1),
                        block_height=102,
                        value=coinvalue - 10000,
                        script=bytes(P2SH_OP_TRUE),
                        plugins={
                            "test_plugin": pb.PluginEntry(
                                groups=[b"group"], data=[b"payload"]
                            )
                        },
                    )
                ]
            ),
        )


if __name__ == "__main__":
    ChronikPluginsTest().main()
