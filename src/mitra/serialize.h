// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include <compat/byteswap.h>
#include <crypto/common.h>
#include <logging.h>
#include <primitives/block.h>
#include <primitives/transaction.h>
#include <serialize.h>

#include <ios>
#include <iostream>
#include <limits>

/**
 * I think I would do it the following way:
    0xxxxxxx -> number is simply the byte itself if <0x80
    1sssxxxx xxxxxxxx [xxxxxxxx …] -> three size bits, which tell us how many
 extra bytes follow (big endian)
        1000xxxx xxxxxxxx
        1001xxxx xxxxxxxx xxxxxxxx
        1010xxxx xxxxxxxx xxxxxxxx xxxxxxxx
    etc.
    This way it’s still very easy to read the number for a human in hex:
    Less than 80 -> it’s the number itself
    8xxx -> it’s xxx
    9xxxxx -> it’s xxxxx
    axxxxxxx -> it’s xxxxxxx
    bxxxxxxxxx -> it’s xxxxxxxxx
    etc.

    And the code is dead simple. Don’t even need to shift any bits really
 (except by multiples of 8).
 */

template <typename Stream> void WriteRuckInt(Stream &os, uint64_t value) {
    if (value < 0x80) {
        ser_writedata8(os, value);
        return;
    }
    // 0x0000'0000'0000'007f -> 7f
    // 0x0000'0000'0000'00ff -> 80ff
    // 0x0000'0000'0000'0fff -> 8fff
    // 0x0000'0000'0000'ffff -> 90'ffff
    // 0x0000'0000'000f'ffff -> 9f'ffff
    // 0x0000'0000'00ff'ffff -> a0ff'ffff
    // 0x0000'0000'0fff'ffff -> afff'ffff
    // 0x0000'0000'ffff'ffff -> b0'ffff'ffff
    // 0x0000'000f'ffff'ffff -> bf'ffff'ffff
    // 0x0000'00ff'ffff'ffff -> c0ff'ffff'ffff
    // 0x0000'0fff'ffff'ffff -> cfff'ffff'ffff
    // 0x0000'ffff'ffff'ffff -> d0'ffff'ffff'ffff
    // 0x000f'ffff'ffff'ffff -> df'ffff'ffff'ffff
    // 0x00ff'ffff'ffff'ffff -> e0ff'ffff'ffff'ffff
    // 0x0fff'ffff'ffff'ffff -> efff'ffff'ffff'ffff
    // 0xffff'ffff'ffff'ffff -> f0'ffff'ffff'ffff'ffff
    const uint64_t numBits = CountBits(value);
    const uint64_t numBytes = (numBits + 3) / 8;
    const uint64_t headerVal = numBytes < 8 ? value >> (numBytes * 8) : 0;
    const uint64_t header = 0x80 | ((numBytes - 1) << 4) | headerVal;
    ser_writedata8(os, header);

    uint64_t n = value << ((8 - numBytes) * 8);

    for (size_t i = 0; i < numBytes; ++i) {
        ser_writedata8(os, n >> 56);
        n <<= 8;
    }
}

template <typename Stream>
uint64_t ReadRuckInt(Stream &is, bool range_check = true) {
    uint8_t header = ser_readdata8(is);
    // LogPrintf("size = %02x\n", size);
    if ((header & 0x80) == 0) {
        return header;
    }
    const size_t numBytes = ((header & 0x70) >> 4) + 1;
    uint64_t result = header & 0x0f;
    for (size_t i = 0; i < numBytes; ++i) {
        uint8_t nextByte = ser_readdata8(is);
        /*if (result == 0 && nextByte > 0x80) {
            throw std::ios_base::failure("non-canonical ReadRuckInt()");
        }*/
        result <<= 8;
        result |= nextByte;
    }
    return result;
}

template <typename Stream> void WriteSechetInt(Stream &os, uint64_t value) {
    if (value < 0x80) {
        ser_writedata8(os, value);
        return;
    }

    // 0x0000'0000'0000'007f (7) -> "7f"
    // 0x0000'0000'0000'3fff (14) -> "bfff"
    // 0x0000'0000'001f'ffff (21) -> "dfffff"
    // 0x0000'0000'0fff'ffff (28) -> "efffffff"
    // 0x0000'0007'ffff'ffff (35) -> "f7ffffffff"
    // 0x0000'03ff'ffff'ffff (42) -> "fbffffffffff"
    // 0x0001'ffff'ffff'ffff (49) -> "fdffffffffffff"
    // 0x00ff'ffff'ffff'ffff (56) -> "feffffffffffffff"
    // 0xffff'ffff'ffff'ffff (63) -> "ffffffffffffffffff"

    const uint64_t numBits = CountBits(value);
    uint64_t numBytes = (numBits + 6) / 7 - 1;
    if (numBytes > 8) {
        numBytes = 8;
    }
    const uint64_t headerType = ~((1 << (8 - numBytes)) - 1) & 0xff;
    const uint64_t headerVal = numBytes < 7 ? value >> (numBytes * 8) : 0;
    ser_writedata8(os, headerType | headerVal);

    uint64_t n = value << ((8 - numBytes) * 8);
    for (size_t i = 0; i < numBytes; ++i) {
        ser_writedata8(os, n >> 56);
        n <<= 8;
    }
}

template <typename Stream>
uint64_t ReadSechetInt(Stream &is, bool range_check = true) {
    const uint8_t header = ser_readdata8(is);
    if ((header & 0x80) == 0) {
        return header;
    }
    const uint64_t numHeaderBits = CountBits(uint8_t(~header));
    const uint64_t leadingOnes = 8 - numHeaderBits;
    const uint8_t mask = (1 << numHeaderBits) - 1;
    uint64_t result = header & mask;
    for (size_t i = 0; i < leadingOnes; ++i) {
        uint8_t nextByte = ser_readdata8(is);
        result <<= 8;
        result |= nextByte;
    }
    return result;
}

const uint64_t DEN_OFFSETS[] = {
    0x0000'0000'0000'0000,
    0x0000'0000'0000'0080,
    0x0000'0000'0000'4080,
    0x0000'0000'0020'4080,
    0x0000'0000'1020'4080,
    0x0000'0008'1020'4080,
    0x0000'0408'1020'4080,
    0x0002'0408'1020'4080,
    0x0102'0408'1020'4080,
};

template <typename Stream> void WriteDenInt(Stream &os, uint64_t value) {
    if (value < 0x80) {
        ser_writedata8(os, value);
        return;
    }
    
    uint64_t numBytes;
    uint64_t offset = DEN_OFFSETS[8];
    for (numBytes = 0; numBytes < 8; ++numBytes) {
        if (value < DEN_OFFSETS[numBytes + 1]) {
            offset = DEN_OFFSETS[numBytes];
            break;
        }
    }

    const uint64_t offsetValue = value - offset;
    const uint64_t headerType = ~((1 << (8 - numBytes)) - 1) & 0xff;
    const uint64_t headerVal = numBytes < 7 ? offsetValue >> (numBytes * 8) : 0;
    ser_writedata8(os, headerType | headerVal);

    uint64_t n = offsetValue << ((8 - numBytes) * 8);
    for (size_t i = 0; i < numBytes; ++i) {
        ser_writedata8(os, n >> 56);
        n <<= 8;
    }
}

template <typename Stream>
uint64_t ReadDenInt(Stream &is, bool range_check = true) {
    const uint8_t header = ser_readdata8(is);
    if ((header & 0x80) == 0) {
        return header;
    }
    const uint64_t numHeaderBits = CountBits(uint8_t(~header));
    const uint64_t leadingOnes = 8 - numHeaderBits;
    const uint8_t mask = (1 << numHeaderBits) - 1;
    uint64_t result = header & mask;
    for (size_t i = 0; i < leadingOnes; ++i) {
        uint8_t nextByte = ser_readdata8(is);
        result <<= 8;
        result |= nextByte;
    }
    return result + DEN_OFFSETS[leadingOnes];
}

struct RuckIntFormatter {
    template <typename Stream, typename I> void Unser(Stream &s, I &v) {
        uint64_t n = ReadRuckInt<Stream>(s, true);
        if (n < std::numeric_limits<I>::min() ||
            n > std::numeric_limits<I>::max()) {
            throw std::ios_base::failure("RuckInt exceeds limit of type");
        }
        v = n;
    }

    template <typename Stream, typename I> void Ser(Stream &s, I v) {
        static_assert(std::is_unsigned<I>::value,
                      "RuckInt only supported for unsigned integers");
        static_assert(std::numeric_limits<I>::max() <=
                          std::numeric_limits<uint64_t>::max(),
                      "RuckInt only supports 64-bit integers and below");

        WriteRuckInt<Stream>(s, v);
    }
};

struct SechetIntFormatter {
    template <typename Stream, typename I> void Unser(Stream &s, I &v) {
        uint64_t n = ReadSechetInt<Stream>(s, true);
        if (n < std::numeric_limits<I>::min() ||
            n > std::numeric_limits<I>::max()) {
            throw std::ios_base::failure("SechetInt exceeds limit of type");
        }
        v = n;
    }

    template <typename Stream, typename I> void Ser(Stream &s, I v) {
        static_assert(std::is_unsigned<I>::value,
                      "SechetInt only supported for unsigned integers");
        static_assert(std::numeric_limits<I>::max() <=
                          std::numeric_limits<uint64_t>::max(),
                      "SechetInt only supports 64-bit integers and below");

        WriteSechetInt<Stream>(s, v);
    }
};

struct DenIntFormatter {
    template <typename Stream, typename I> void Unser(Stream &s, I &v) {
        uint64_t n = ReadDenInt<Stream>(s, true);
        if (n < std::numeric_limits<I>::min() ||
            n > std::numeric_limits<I>::max()) {
            throw std::ios_base::failure("SechetInt exceeds limit of type");
        }
        v = n;
    }

    template <typename Stream, typename I> void Ser(Stream &s, I v) {
        static_assert(std::is_unsigned<I>::value,
                      "SechetInt only supported for unsigned integers");
        static_assert(std::numeric_limits<I>::max() <=
                          std::numeric_limits<uint64_t>::max(),
                      "SechetInt only supports 64-bit integers and below");

        WriteDenInt<Stream>(s, v);
    }
};

template <class Formatter, typename NumFmt> struct MitraVectorFormatter {
    template <typename Stream, typename V> void Ser(Stream &s, const V &v) {
        Formatter formatter;
        NumFmt numFmt;
        numFmt.Ser(s, v.size());
        for (const typename V::value_type &elem : v) {
            formatter.Ser(s, elem);
        }
    }

    template <typename Stream, typename V> void Unser(Stream &s, V &v) {
        Formatter formatter;
        NumFmt numFmt;
        v.clear();
        size_t size;
        numFmt.Unser(s, size);
        size_t allocated = 0;
        while (allocated < size) {
            // For DoS prevention, do not blindly allocate as much as the stream
            // claims to contain. Instead, allocate in 5MiB batches, so that an
            // attacker actually needs to provide X MiB of data to make us
            // allocate X+5 Mib.
            static_assert(sizeof(typename V::value_type) <= MAX_VECTOR_ALLOCATE,
                          "Vector element size too large");
            allocated =
                std::min(size, allocated + MAX_VECTOR_ALLOCATE /
                                               sizeof(typename V::value_type));
            v.reserve(allocated);
            while (v.size() < allocated) {
                v.emplace_back();
                formatter.Unser(s, v.back());
            }
        }
    };
};

template <typename NumFmt> struct MitraAmountFormatter {
    template <typename Stream> void Ser(Stream &s, const Amount &amount) const {
        NumFmt numFmt;
        int64_t amountSats = amount / SATOSHI;
        if (amountSats < 0) {
            throw std::ios_base::failure("Cannot format negative amount");
        }
        numFmt.Ser(s, uint64_t(amountSats));
    }

    template <typename Stream> void Unser(Stream &s, Amount &amount) {
        NumFmt numFmt;
        uint64_t amountSats;
        numFmt.Unser(s, amountSats);
        amount = int64_t(amountSats) * SATOSHI;
    }
};

template <typename NumFmt> struct MitraCOutPointFormatter {
    template <typename Stream>
    void Ser(Stream &s, const COutPoint &outpoint) const {
        s << outpoint.GetTxId();
        s << Using<NumFmt>(outpoint.GetN());
    }

    template <typename Stream> void Unser(Stream &s, COutPoint &outpoint) {
        TxId txid;
        s >> txid;
        uint32_t n;
        s >> Using<NumFmt>(n);
        outpoint = COutPoint(txid, n);
    }
};

template <typename NumFmt> struct MitraCTxInFormatter {
    FORMATTER_METHODS(CTxIn, obj) {
        READWRITE(Using<MitraCOutPointFormatter<NumFmt>>(obj.prevout),
                  Using<MitraVectorFormatter<DefaultFormatter, NumFmt>>(
                      obj.scriptSig),
                  Using<NumFmt>(obj.nSequence));
    }
};

template <typename NumFmt> struct MitraCTxOutFormatter {
    FORMATTER_METHODS(CTxOut, obj) {
        READWRITE(Using<MitraAmountFormatter<NumFmt>>(obj.nValue),
                  Using<MitraVectorFormatter<DefaultFormatter, NumFmt>>(
                      obj.scriptPubKey));
    }
};

template <class TxType, typename NumFmt> struct MitraCTxFormatter {
    FORMATTER_METHODS(TxType, obj) {
        READWRITE(
            obj.nVersion,
            Using<MitraVectorFormatter<MitraCTxInFormatter<NumFmt>, NumFmt>>(
                obj.vin),
            Using<MitraVectorFormatter<MitraCTxOutFormatter<NumFmt>, NumFmt>>(
                obj.vout),
            Using<NumFmt>(obj.nLockTime));
    }
};

template <typename NumFmt> struct MitraCTxRefFormatter {
    FORMATTER_METHODS(CTransactionRef, obj) {
        READWRITE(
            obj->nVersion,
            Using<MitraVectorFormatter<MitraCTxInFormatter<NumFmt>, NumFmt>>(
                obj->vin),
            Using<MitraVectorFormatter<MitraCTxOutFormatter<NumFmt>, NumFmt>>(
                obj->vout),
            Using<NumFmt>(obj->nLockTime));
    }
};

template <typename NumFmt> struct MitraCBlockFormatter {
    FORMATTER_METHODS(CBlock, obj) {
        READWRITEAS(CBlockHeader, obj);
        READWRITE(
            Using<VectorFormatter<MitraCTxRefFormatter<NumFmt>>>(obj.vtx));
    }
};
