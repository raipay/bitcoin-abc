# Copyright (c) 2024 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test the Chronik plugin system gets sets up correctly.
"""

import os
import os.path

from test_framework.test_framework import BitcoinTestFramework


class ChronikPluginsSetup(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [["-chronik"]]

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik_plugins()

    def run_test(self):
        node = self.nodes[0]
        # Chronik doesn't even initialize Python if there's no /plugins dir
        with node.assert_debug_log(
            [f"Plugin dir {node.datadir}/plugins doesn't exist, skipping"]
        ):
            self.restart_node(0, ["-chronik"])

        # Create plugin dir
        plugins_dir = os.path.join(node.datadir, "plugins")
        os.mkdir(plugins_dir)

        # Now Chronik initializes Python
        with node.assert_debug_log(["Plugin context initialized Python"]):
            self.restart_node(0, ["-chronik"])


if __name__ == "__main__":
    ChronikPluginsSetup().main()
