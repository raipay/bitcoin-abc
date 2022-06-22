#include <array>
#include <optional>

#include <blockdb.h>
#include <chainparams.h>
#include <consensus/validation.h>
#include <logging.h>
#include <node/blockstorage.h>
#include <node/coin.h>
#include <node/context.h>
#include <node/ui_interface.h>
#include <pubkey.h>
#include <rpc/request.h>
#include <rpc/server.h>
#include <timedata.h>
#include <undo.h>
#include <util/translation.h>
#include <validation.h>
#include <validationinterface.h>

#include <chronik_cxxbridge/src/lib.rs.h>

template <typename T, typename C> rust::Vec<T> ToRustVec(const C &container) {
    rust::Vec<uint8_t> vec;
    vec.reserve(container.size());
    std::copy(container.begin(), container.end(), std::back_inserter(vec));
    return vec;
}

std::array<uint8_t, 32> HashToArray(const uint256 &hash) {
    std::array<uint8_t, 32> array;
    std::copy_n(hash.begin(), 32, array.begin());
    return array;
}

chronik_bridge::BlockHeader BridgeBlockHeader(const CBlockHeader &header) {
    CDataStream raw_header(SER_NETWORK, PROTOCOL_VERSION);
    raw_header << header;
    return {
        .raw = ToRustVec<uint8_t>(raw_header),
        .hash = HashToArray(header.GetHash()),
        .prev_hash = HashToArray(header.hashPrevBlock),
        .n_bits = header.nBits,
        .timestamp = header.GetBlockTime(),
    };
}

chronik_bridge::TxOutput BridgeTxOutput(const CTxOut &output) {
    return {
        .value = output.nValue / Amount::satoshi(),
        .script = ToRustVec<uint8_t>(output.scriptPubKey),
    };
}

chronik_bridge::Coin BridgeCoin(const Coin &coin) {
    const int32_t nHeight =
        coin.GetHeight() == 0x7fff'ffff ? -1 : coin.GetHeight();
    return {
        .tx_output = BridgeTxOutput(coin.GetTxOut()),
        .is_coinbase = coin.IsCoinBase(),
        .height = nHeight,
    };
}

rust::Vec<chronik_bridge::Coin>
BridgeSpentCoins(std::optional<const std::vector<Coin> *> spent_coins,
                 size_t &nUndoPos) {
    if (!spent_coins) {
        return {};
    }
    rust::Vec<chronik_bridge::Coin> bridge_coins;
    for (const Coin &coin : **spent_coins) {
        nUndoPos +=
            GetSerializeSize(Using<TxInUndoFormatter>(coin), PROTOCOL_VERSION);
        bridge_coins.push_back(BridgeCoin(coin));
    }
    nUndoPos += GetSizeOfCompactSize(bridge_coins.size());
    return bridge_coins;
}

chronik_bridge::Tx BridgeTxMempool(const CTransactionRef &tx,
                                   const std::vector<Coin> &spent_coins) {
    size_t nUndoPos;
    CDataStream raw_tx(SER_NETWORK, PROTOCOL_VERSION);
    raw_tx << tx;
    return {.txid = HashToArray(tx->GetId()),
            .raw = ToRustVec<uint8_t>(raw_tx),
            .spent_coins =
                BridgeSpentCoins(std::optional(&spent_coins), nUndoPos)};
}

chronik_bridge::BlockTx
BridgeBlockTx(const CTransactionRef &tx,
              std::optional<const std::vector<Coin> *> spent_coins,
              size_t &nDataPos, size_t &nUndoPos) {
    CDataStream raw_tx(SER_NETWORK, PROTOCOL_VERSION);
    raw_tx << tx;
    const size_t data_pos = nDataPos;
    const size_t undo_pos = spent_coins ? nUndoPos : 0;
    nDataPos += raw_tx.size();
    rust::Vec<chronik_bridge::Coin> bridge_spent_coins =
        BridgeSpentCoins(spent_coins, nUndoPos);
    return {.tx =
                {
                    .txid = HashToArray(tx->GetId()),
                    .raw = ToRustVec<uint8_t>(raw_tx),
                    .spent_coins = bridge_spent_coins,
                },
            .data_pos = uint32_t(data_pos),
            .undo_pos = uint32_t(undo_pos),
            .undo_size = spent_coins ? uint32_t(nUndoPos - undo_pos) : 0};
}

size_t GetFirstBlockTxOffset(const CBlock &block, const CBlockIndex *pindex) {
    return pindex->nDataPos + ::GetSerializeSize(CBlockHeader()) +
           GetSizeOfCompactSize(block.vtx.size());
}

size_t GetFirstUndoOffset(const CBlock &block, const CBlockIndex *pindex) {
    return pindex->nUndoPos + GetSizeOfCompactSize(block.vtx.size() - 1);
}

chronik_bridge::Block BridgeBlock(const CBlock &block,
                                  const CBlockIndex *pindex) {
    size_t nDataPos = GetFirstBlockTxOffset(block, pindex);
    size_t nUndoPos = 0;
    CBlockUndo block_undo;
    if (pindex->nHeight) { // Genesis block doesn't have undo data
        nUndoPos = GetFirstUndoOffset(block, pindex);
        if (!UndoReadFromDisk(block_undo, pindex)) {
            throw std::runtime_error("Reading block undo data failed");
        }
    }
    rust::Vec<chronik_bridge::BlockTx> bridge_txs;
    for (size_t tx_idx = 0; tx_idx < block.vtx.size(); ++tx_idx) {
        std::optional<std::vector<Coin> *> spent_coins =
            tx_idx != 0
                ? std::optional(&block_undo.vtxundo[tx_idx - 1].vprevout)
                : std::nullopt;
        bridge_txs.push_back(
            BridgeBlockTx(block.vtx[tx_idx], spent_coins, nDataPos, nUndoPos));
    }
    return {.header = BridgeBlockHeader(block.GetBlockHeader()),
            .txs = bridge_txs,
            .file_num = uint32_t(pindex->nFile),
            .data_pos = pindex->nDataPos,
            .undo_pos = pindex->nUndoPos};
}

namespace chronik_bridge {

rust::Vec<chronik_bridge::Block>
ChronikBridge::get_block_range(int32_t start_height,
                               uint32_t num_blocks) const {
    LOCK(cs_main);
    const int32_t chain_height = ::ChainActive().Height();
    int32_t end_height = start_height + num_blocks - 1;
    if (end_height > chain_height) {
        end_height = chain_height;
        num_blocks = end_height - start_height + 1;
    }

    CBlockIndex *pindex = nullptr;
    if (start_height >= 0) {
        pindex = ::ChainActive().Tip()->GetAncestor(end_height);
    } else {
        num_blocks = 0;
    }
    rust::Vec<chronik_bridge::Block> bridge_blocks;
    bridge_blocks.reserve(num_blocks);
    for (size_t i = 0; i < num_blocks; ++i) {
        if (pindex == nullptr) {
            break;
        }
        CBlock block;
        if (!ReadBlockFromDisk(block, pindex, m_consensus)) {
            throw std::runtime_error("Reading block data failed");
        }
        bridge_blocks.push_back(BridgeBlock(block, pindex));
        pindex = pindex->pprev;
    }
    std::reverse(bridge_blocks.begin(), bridge_blocks.end());
    return bridge_blocks;
}

rust::Vec<uint8_t> ChronikBridge::get_block_slice(uint32_t file_num,
                                                  uint32_t data_pos,
                                                  uint32_t num_bytes) const {
    const FlatFilePos filePos(file_num, data_pos);
    CAutoFile file(OpenBlockFile(filePos, true), SER_DISK, CLIENT_VERSION);
    // equivalent to std::vector<uint8_t> data(num_bytes)
    rust::Vec<uint8_t> data;
    data.reserve(num_bytes);
    for (size_t i = 0; i < num_bytes; ++i) {
        data.push_back(0);
    }
    try {
        file.read((char *)data.data(), num_bytes);
    } catch (const std::exception &e) {
        throw std::runtime_error("Invalid block slice");
    }
    return data;
}

rust::Vec<uint8_t> ChronikBridge::get_undo_slice(uint32_t file_num,
                                                 uint32_t undo_pos,
                                                 uint32_t num_bytes) const {
    const FlatFilePos filePos(file_num, undo_pos);
    CAutoFile file(OpenUndoFile(filePos, true), SER_DISK, CLIENT_VERSION);
    // equivalent to std::vector<uint8_t> data(num_bytes)
    rust::Vec<uint8_t> data;
    data.reserve(num_bytes);
    for (size_t i = 0; i < num_bytes; ++i) {
        data.push_back(0);
    }
    try {
        file.read((char *)data.data(), num_bytes);
    } catch (const std::exception &e) {
        throw std::runtime_error("Invalid undo slice");
    }
    return data;
}

rust::String
ChronikBridge::run_rpc_command(rust::Str command,
                               rust::Slice<const rust::Str> params) const {
    // Parse and build JSON params
    UniValue json_params(UniValue::VARR);
    json_params.reserve(params.size());
    for (const rust::Str &param : params) {
        UniValue json_param;
        if (!json_param.read(std::string(param))) {
            throw std::runtime_error(
                JSONRPCError(RPC_INVALID_REQUEST, "Invalid params").write());
        }
        json_params.push_back(json_param);
    }
    // Build JSONRPC request
    JSONRPCRequest req;
    req.context = (NodeContext *)&m_node;
    req.params = json_params;
    req.strMethod = std::string(command);
    req.URI = "";
    // Execute and run
    try {
        const UniValue jsonResult = ::tableRPC.execute(m_config, req);
        return jsonResult.write();
    } catch (UniValue &ex) {
        throw std::runtime_error(ex.write());
    }
}

std::array<uint8_t, 65>
serialize_pubkey_uncompressed(std::array<uint8_t, 33> pubkey_compressed) {
    std::array<uint8_t, 65> result;
    CPubKey pubkey(pubkey_compressed.begin(), pubkey_compressed.end());
    if (!pubkey.Decompress()) {
        return result;
    }
    std::copy(pubkey.begin(), pubkey.end(), result.begin());
    return result;
}

} // namespace chronik_bridge

class ChronikValidationInterface final : public CValidationInterface {
public:
    ChronikValidationInterface(
        const NodeContext &node,
        rust::Box<chronik_bridge::ChronikIndexer> chronik_indexer)
        : m_node(node), m_chronik_indexer(std::move(chronik_indexer)) {}

    void Register() { RegisterValidationInterface(this); }

    void Unregister() { UnregisterValidationInterface(this); }

private:
    const NodeContext &m_node;
    rust::Box<chronik_bridge::ChronikIndexer> m_chronik_indexer;

    void TransactionAddedToMempool(const CTransactionRef &ptx,
                                   uint64_t mempool_sequence) override {
        std::map<COutPoint, ::Coin> spent_coins_map;
        for (const CTxIn &input : ptx->vin) {
            spent_coins_map[input.prevout] = ::Coin();
        }
        FindCoins(m_node, spent_coins_map);
        std::vector<::Coin> spent_coins;
        spent_coins.reserve(spent_coins_map.size());
        for (const CTxIn &input : ptx->vin) {
            spent_coins.push_back(spent_coins_map[input.prevout]);
        }
        chronik_bridge::MempoolTx mempool_tx = {
            .tx = BridgeTxMempool(ptx, spent_coins),
            .time = GetAdjustedTime(),
        };
        m_chronik_indexer->handle_tx_added_to_mempool(mempool_tx);
    }

    void TransactionRemovedFromMempool(const CTransactionRef &ptx,
                                       MemPoolRemovalReason reason,
                                       uint64_t mempool_sequence) override {
        m_chronik_indexer->handle_tx_removed_from_mempool(
            HashToArray(ptx->GetId()));
    }

    void BlockConnected(const std::shared_ptr<const CBlock> &block,
                        const CBlockIndex *pindex) override {
        m_chronik_indexer->handle_block_connected(BridgeBlock(*block, pindex));
    }

    void BlockDisconnected(const std::shared_ptr<const CBlock> &block,
                           const CBlockIndex *pindex) override {
        m_chronik_indexer->handle_block_disconnected(
            BridgeBlock(*block, pindex));
    }
};

std::unique_ptr<ChronikValidationInterface> g_chronik_validation_interface;

void StartChronikValidationInterface(
    const NodeContext &node,
    rust::Box<chronik_bridge::ChronikIndexer> chronik_indexer) {
    g_chronik_validation_interface =
        std::make_unique<ChronikValidationInterface>(node, std::move(chronik_indexer));
    g_chronik_validation_interface->Register();
}

void StopChronikValidationInterface() {
    g_chronik_validation_interface->Unregister();
    g_chronik_validation_interface.reset();
}
