// Copyright (c) 2022 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef BITCOIN_CHRONIK_CPP_CHRONIK_H
#define BITCOIN_CHRONIK_CPP_CHRONIK_H

#include <string>
#include <vector>

class Config;
namespace node {
struct NodeContext;
} // namespace node

namespace chronik {

static const std::vector<std::string> DEFAULT_BINDS = {"127.0.0.1", "::1"};

// How many blocks of txid -> tx num maps are cached in-memory.
// Caution against setting this too high, there is an optimum which lies around
// 100; using e.g. 1000 is slower and also requires more memory.
// Memory usage will be around 40B per tx in the cache, so 100 blocks with 2000
// tx/block would require around 8MB of RAM.
static const size_t DEFAULT_TX_NUM_CACHE_DEPTH = 100;

// Registers Chronik indexer as ValidationInterface, listens to HTTP queries
bool Start(const Config &config, const node::NodeContext &node, bool fWipe);

// Unregisters Chronik indexer as ValidationInterface, stops the HTTP server
void Stop();

} // namespace chronik

#endif // BITCOIN_CHRONIK_CPP_CHRONIK_H
