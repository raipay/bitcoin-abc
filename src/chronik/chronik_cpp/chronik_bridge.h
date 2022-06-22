#include <chainparams.h>
#include <config.h>
#include <node/context.h>

#include <rust/cxx.h>

namespace chronik_bridge {

struct Block;
struct MempoolTx;
struct ChronikIndexer;

class ChronikBridge {
private:
    const Config &m_config;
    const Consensus::Params &m_consensus;
    NodeContext &m_node;

public:
    ChronikBridge(const Config &config, const Consensus::Params &consensus,
                  NodeContext &node)
        : m_config(config), m_consensus(consensus), m_node(node) {}

    rust::Vec<::chronik_bridge::Block>
    get_block_range(int32_t start_height, uint32_t num_blocks) const;
    rust::Vec<uint8_t> get_block_slice(uint32_t file_num, uint32_t data_pos,
                                       uint32_t num_bytes) const;
    rust::Vec<uint8_t> get_undo_slice(uint32_t file_num, uint32_t undo_pos,
                                      uint32_t num_bytes) const;
    rust::String run_rpc_command(rust::Str command,
                                 rust::Slice<const rust::Str> params) const;
};

std::array<uint8_t, 65>
serialize_pubkey_uncompressed(std::array<uint8_t, 33> pubkey);

} // namespace chronik_bridge

void StartChronikValidationInterface(
    const NodeContext &node,
    rust::Box<chronik_bridge::ChronikIndexer> chronik_indexer);

void StopChronikValidationInterface();
