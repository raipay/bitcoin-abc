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

static const bool DEFAULT_SCRIPT_HISTORY_BLOOM_IS_ENABLED = true;
static const double DEFAULT_SCRIPT_HISTORY_BLOOM_FALSE_POSITIVE_RATE = 0.90;
static const size_t DEFAULT_SCRIPT_HISTORY_BLOOM_EXPECTED_NUM_ITEMS =
    100'000'000;

// Registers Chronik indexer as ValidationInterface, listens to HTTP queries
bool Start(const Config &config, const node::NodeContext &node, bool fWipe);

// Unregisters Chronik indexer as ValidationInterface, stops the HTTP server
void Stop();

} // namespace chronik

#endif // BITCOIN_CHRONIK_CPP_CHRONIK_H
