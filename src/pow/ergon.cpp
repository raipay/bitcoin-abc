// Copyright (c) 2009-2010 Satoshi Nakamoto
// Copyright (c) 2009-2016 The Bitcoin Core developers
// Copyright (c) 2017-2020 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include <pow/pow.h>

#include <arith_uint256.h>
#include <chain.h>
#include <chainparams.h>
#include <consensus/activation.h>
#include <consensus/params.h>
#include <pow/aserti32d.h>
#include <pow/daa.h>
#include <pow/eda.h>
#include <pow/grasberg.h>
#include <primitives/blockhash.h>
#include <util/system.h>

/**
 * To reduce the impact of timestamp manipulation, we select the block we are
 * basing our computation on via a median of 3.
 */
static const CBlockIndex *GetSuitableBlock(const CBlockIndex *pindex) {
    assert(pindex->nHeight >= 3);

    /**
     * In order to avoid a block with a very skewed timestamp having too much
     * influence, we select the median of the 3 top most blocks as a starting
     * point.
     */
    const CBlockIndex *blocks[3];
    blocks[2] = pindex;
    blocks[1] = pindex->pprev;
    blocks[0] = blocks[1]->pprev;

    // Sorting network.
    if (blocks[0]->nTime > blocks[2]->nTime) {
        std::swap(blocks[0], blocks[2]);
    }

    if (blocks[0]->nTime > blocks[1]->nTime) {
        std::swap(blocks[0], blocks[1]);
    }

    if (blocks[1]->nTime > blocks[2]->nTime) {
        std::swap(blocks[1], blocks[2]);
    }

    // We should have our candidate in the middle now.
    return blocks[1];
}

static arith_uint256 ComputeEmaTarget(const CBlockIndex *pindexPrev,
                                      const Consensus::Params &params) {
    arith_uint256 work = pindexPrev->nChainWork - pindexPrev->pprev->nChainWork;
    // arith_uint256 work =GetBlockProof(pindexPrev)
    const CBlockIndex *p1 = GetSuitableBlock(pindexPrev);
    const CBlockIndex *p0 = GetSuitableBlock(pindexPrev->pprev);

    int64_t t1 = p1->nTime;
    int64_t t0 = p0->nTime;
    int64_t t = t1 - t0;

    int64_t resistance = 1000;

    /*
        Next block difficulty will be calculated with the quadratic
        approximation of the formula:
        diff' = diff / (1 + (t/T-1) / r)
        where t is the recent solve time difference guarded by 3 block median,
        T is the target spacing and r is resistance, namely
        diff' = diff[ 1 - (t/T-1) / r + (t/T-1)^2 / r^2 ]
        The function has a minimum around t_0/T = r/2 - 1 and then starts
        growing. For resistance = 1000 t_0 is around 3.5 days. In the event
        of a massive hashrate drop, the the difficulty would go up instead
        of going down. To prevent that we flatten the parabola
        after the minimum.
    */

    int minimum = resistance / 2 - 1;
    int normalized_time = t / params.nPowTargetSpacing;

    if (normalized_time > minimum) {
        work -= (work * minimum) / resistance - work / resistance -
                (work * minimum * minimum) / (resistance * resistance) +
                2 * (work * minimum) / (resistance * resistance) -
                work / (resistance * resistance);
    } else if (t >= 0) {
        work -= (work * t / params.nPowTargetSpacing) / resistance -
                work / resistance -
                (work * t * t /
                 (params.nPowTargetSpacing * params.nPowTargetSpacing)) /
                    (resistance * resistance) +
                2 * (work * t / params.nPowTargetSpacing) /
                    (resistance * resistance) -
                work / (resistance * resistance);
    } else {
        // Multiplication of arith uint by a negative is unimplemented, for
        // negative sign we pull the minus out manually
        t = -t;
        work += (work * t / params.nPowTargetSpacing) / resistance +
                work / resistance +
                (work * t * t /
                 (params.nPowTargetSpacing * params.nPowTargetSpacing)) /
                    (resistance * resistance) +
                2 * (work * t / params.nPowTargetSpacing) /
                    (resistance * resistance) +
                work / (resistance * resistance);
    }
    /**
     * We need to compute T = (2^256 / W) - 1 but 2^256 doesn't fit in 256 bits.
     * By expressing 1 as W / W, we get (2^256 - W) / W, and we can compute
     * 2^256 - W as the complement of W.
     */
    return (-work) / work;
}

static arith_uint256 ComputeExpTarget(const CBlockIndex *pindexPrev,
                                      const Consensus::Params &params) {
    arith_uint256 work = pindexPrev->nChainWork - pindexPrev->pprev->nChainWork;
    // arith_uint256 work =GetBlockProof(pindexPrev)
    const CBlockIndex *p1 = GetSuitableBlock(pindexPrev);
    const CBlockIndex *p0 = GetSuitableBlock(pindexPrev->pprev);

    int64_t t = p1->nTime - p0->nTime;

    int64_t resistance = 1000;

    /*
        Next block difficulty will be calculated with the quadratic
        approximation of the formula:
        diff' = diff / (1 + (t/T-1) / r)
        where t is the recent solve time difference guarded by 3 block median,
        T is the target spacing and r is resistance, namely
        diff' = diff[ 1 - (t/T-1) / r + (t/T-1)^2 / r^2 ]
        The function has a minimum around t_0/T = r/2 - 1 and then starts
        growing. For resistance = 1000 t_0 is around 3.5 days. In the event
        of a massive hashrate drop, the the difficulty would go up instead
        of going down. To prevent that we flatten the parabola
        after the minimum.
    */

    int minimum = resistance / 2 - 1;
    int normalized_time = t / params.nPowTargetSpacing;

    if (normalized_time > minimum) {
        work -= (work * minimum) / resistance - work / resistance -
                (work * minimum * minimum) / (resistance * resistance) +
                2 * (work * minimum) / (resistance * resistance) -
                work / (resistance * resistance);
    } else {
        work -= (work * t / params.nPowTargetSpacing) / resistance -
                work / resistance -
                (work * (t * t) /
                 (params.nPowTargetSpacing * params.nPowTargetSpacing)) /
                    (resistance * resistance) +
                2 * (work * t / params.nPowTargetSpacing) /
                    (resistance * resistance) -
                work / (resistance * resistance);
    }

    /**
     * We need to compute T = (2^256 / W) - 1 but 2^256 doesn't fit in 256 bits.
     * By expressing 1 as W / W, we get (2^256 - W) / W, and we can compute
     * 2^256 - W as the complement of W.
     */
    return (-work) / work;
}

uint32_t GetNextExpWorkRequired(const CBlockIndex *pindexPrev,
                                const CBlockHeader *pblock,
                                const Consensus::Params &params) {
    // This cannot handle the genesis block and early blocks in general.
    assert(pindexPrev);
    if (pindexPrev->nHeight < 4) {
        return 0x1a04b500; // should be about right for two s9
    }
    arith_uint256 nextTarget = ComputeExpTarget(pindexPrev, params);
    if (!IsErgonEMAEnabled(params, pindexPrev)) {
        nextTarget = ComputeExpTarget(pindexPrev, params);
    } else if (IsErgonEMAEnabled(params, pindexPrev) &&
               !IsErgonEMAEnabled(params, pindexPrev->pprev)) {
        // Due to attack on Jun 24 2022 the difficulty was driven to powLimit,
        // we need to jumpstart it back on the fork.
        return 0x1a04b500;
        // return 0x1b03c53c; //Should result in a non-zero reward.
    } else {
        nextTarget = ComputeEmaTarget(pindexPrev, params);
    }
    const arith_uint256 powLimit = UintToArith256(params.powLimit);
    if (nextTarget > powLimit) {
        return powLimit.GetCompact();
    }

    return nextTarget.GetCompact();
}
