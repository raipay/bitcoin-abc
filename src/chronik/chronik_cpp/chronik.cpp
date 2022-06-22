#include <chronik_cxxbridge/src/lib.rs.h>
#include <logging.h>
#include <util/system.h>

void StartChronik(const Config &config, const Consensus::Params &consensus,
                  NodeContext &node) {
    LogPrintf("Starting Chronik!\n");

    std::unique_ptr<chronik_bridge::ChronikBridge> bridge =
        std::make_unique<chronik_bridge::ChronikBridge>(config, consensus,
                                                        node);

    rust::Box<chronik_bridge::ChronikIndexer> chronik_indexer =
        chronik_bridge::setup_indexer({.bridge = std::move(bridge),
                                       .datadir = GetDataDir().u8string(),
                                       .chronik_host = "127.0.0.1:7654"});

    StartChronikValidationInterface(node, std::move(chronik_indexer));
}

void StopChronik() {
    StopChronikValidationInterface();
}
