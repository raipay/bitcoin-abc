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

// Size of the script cache for tx history, in number of entries. Each entry is
// about 30B in size, so the default cache fits ~3MB.
static const size_t DEFAULT_SCRIPT_NUM_TXS_CACHE_SIZE = 100'000;

// Registers Chronik indexer as ValidationInterface, listens to HTTP queries
bool Start(const Config &config, const node::NodeContext &node, bool fWipe);

// Unregisters Chronik indexer as ValidationInterface, stops the HTTP server
void Stop();

} // namespace chronik

#endif // BITCOIN_CHRONIK_CPP_CHRONIK_H
