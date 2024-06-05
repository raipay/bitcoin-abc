// Copyright (c) 2009-2010 Satoshi Nakamoto
// Copyright (c) 2009-2016 The Bitcoin Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include <primitives/block.h>

#include <hash.h>
#include <tinyformat.h>
#include <crypto/scrypt.h>
#include <hash.h>
#include <streams.h>

BlockHash CBlockHeader::GetHash() const {
    return BlockHash(SerializeHash(*this));
}

BlockHash CBlockHeader::GetPoWHash() const {
    CDataStream ss(SER_NETWORK, PROTOCOL_VERSION);
    ss << nVersion;
    ss << hashPrevBlock;
    ss << hashMerkleRoot;
    ss << nTime;
    ss << nBits;
    ss << nNonce;
    uint256 thash;
    scrypt_1024_1_1_256((const char *)ss.data(), (char *)(&thash));
    return BlockHash(thash);
}

std::string CBlock::ToString() const {
    std::stringstream s;
    s << strprintf("CBlock(hash=%s, ver=0x%08x, hashPrevBlock=%s, "
                   "hashMerkleRoot=%s, nTime=%u, nBits=%08x, nNonce=%u, "
                   "vtx=%u)\n",
                   GetHash().ToString(), nVersion, hashPrevBlock.ToString(),
                   hashMerkleRoot.ToString(), nTime, nBits, nNonce, vtx.size());
    for (const auto &tx : vtx) {
        s << "  " << tx->ToString() << "\n";
    }
    return s.str();
}
