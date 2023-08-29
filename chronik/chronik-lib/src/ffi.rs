// Copyright (c) 2022 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

//! Module containing the cxx definitions for the bridge from Rust to C++.

pub use self::ffi_inner::*;
use crate::bridge::{setup_chronik, Chronik};

#[allow(unsafe_code)]
#[cxx::bridge(namespace = "chronik_bridge")]
mod ffi_inner {
    /// Params for setting up Chronik
    #[derive(Debug)]
    pub struct SetupParams {
        /// Where the data of the blockchain is stored, dependent on network
        /// (mainnet, testnet, regtest)
        pub datadir_net: String,
        /// Host addresses where the Chronik HTTP endpoint will be served
        pub hosts: Vec<String>,
        /// Default port for `hosts` if only an IP address is given
        pub default_port: u16,
        /// Whether to clear the DB before proceeding, e.g. when reindexing
        pub wipe_db: bool,
        /// Whether pausing Chronik indexing is allowed
        pub is_pause_allowed: bool,
        /// Whether to output Chronik performance statistics into a perf/
        /// folder
        pub enable_perf_stats: bool,
        pub script_filter_variant: String,
        pub script_false_positive_rate: f32,
        pub script_expected_num_items: u32,
        /// Size of the script cache for tx history, in number of entries. Each
        /// entry is about 30B in size.
        pub script_num_txs_cache_size: usize,
    }

    extern "Rust" {
        type Chronik;
        fn setup_chronik(
            params: SetupParams,
            config: &Config,
            node: &NodeContext,
        ) -> bool;

        fn handle_tx_added_to_mempool(
            &self,
            ptx: &CTransaction,
            spent_coins: &CxxVector<CCoin>,
            time_first_seen: i64,
        );
        fn handle_tx_removed_from_mempool(&self, txid: [u8; 32]);
        fn handle_block_connected(&self, block: &CBlock, bindex: &CBlockIndex);
        fn handle_block_disconnected(
            &self,
            block: &CBlock,
            bindex: &CBlockIndex,
        );
        fn handle_block_finalized(&self, bindex: &CBlockIndex);
    }

    unsafe extern "C++" {
        include!("blockindex.h");
        include!("chronik-cpp/chronik_validationinterface.h");
        include!("coins.h");
        include!("config.h");
        include!("node/context.h");
        include!("primitives/block.h");
        include!("primitives/transaction.h");

        /// CBlockIndex from blockindex.h
        #[namespace = ""]
        type CBlockIndex = chronik_bridge::ffi::CBlockIndex;

        /// ::CBlock from primitives/block.h
        #[namespace = ""]
        type CBlock = chronik_bridge::ffi::CBlock;

        /// ::Coin from coins.h (renamed to CCoin to prevent a name clash)
        #[namespace = ""]
        #[cxx_name = "Coin"]
        type CCoin = chronik_bridge::ffi::CCoin;

        /// ::Config from config.h
        #[namespace = ""]
        type Config = chronik_bridge::ffi::Config;

        /// ::CTransaction from primitives/transaction.h
        #[namespace = ""]
        type CTransaction = chronik_bridge::ffi::CTransaction;

        /// NodeContext from node/context.h
        #[namespace = "node"]
        type NodeContext = chronik_bridge::ffi::NodeContext;

        /// Bridge to bitcoind to access the node
        type ChronikBridge = chronik_bridge::ffi::ChronikBridge;

        /// Register the Chronik instance as CValidationInterface to receive
        /// chain updates from the node.
        #[namespace = "chronik"]
        fn StartChronikValidationInterface(
            node: &NodeContext,
            chronik: Box<Chronik>,
        );
    }
}
