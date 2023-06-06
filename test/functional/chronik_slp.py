#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's SLPv2 integration.
"""

from typing import List, Optional

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import (
    GENESIS_CB_TXID,
    create_block,
    create_coinbase,
    make_conform_to_ctor,
)
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import (
    OP_EQUAL,
    OP_HASH160,
    OP_RESERVED,
    OP_RETURN,
    CScript,
    hash160,
)
from test_framework.test_framework import BitcoinTestFramework
from test_framework.txtools import pad_tx
from test_framework.util import assert_equal


def slpv2_genesis(
    token_ticker: bytes = b"",
    token_name: bytes = b"",
    url: bytes = b"",
    data: bytes = b"",
    auth_pubkey: bytes = b"",
    decimals: int = 0,
    *,
    mint_amounts: List[int],
    num_batons: int,
) -> bytes:
    result = bytearray()
    result.extend(b"SLP2")
    result.append(0)

    result.append(len(b"GENESIS"))
    result.extend(b"GENESIS")

    result.append(len(token_ticker))
    result.extend(token_ticker)

    result.append(len(token_name))
    result.extend(token_name)

    result.append(len(url))
    result.extend(url)

    result.append(len(data))
    result.extend(data)

    result.append(len(auth_pubkey))
    result.extend(auth_pubkey)

    result.append(decimals)

    result.append(len(mint_amounts))
    for amount in mint_amounts:
        result.extend(amount.to_bytes(6, "little"))

    result.append(num_batons)
    return result


def slpv2_mint(
    token_id: bytes,
    mint_amounts: List[int],
    num_batons: int,
) -> bytes:
    result = bytearray()
    result.extend(b"SLP2")
    result.append(0)

    result.append(len(b"MINT"))
    result.extend(b"MINT")

    result.extend(token_id)

    result.append(len(mint_amounts))
    for amount in mint_amounts:
        result.extend(amount.to_bytes(6, "little"))

    result.append(num_batons)

    return result


def slpv2_send(
    token_id: bytes,
    output_amounts: List[int],
) -> bytes:
    result = bytearray()
    result.extend(b"SLP2")
    result.append(0)

    result.append(len(b"SEND"))
    result.extend(b"SEND")

    result.extend(token_id)

    result.append(len(output_amounts))
    for amount in output_amounts:
        result.extend(amount.to_bytes(6, "little"))

    return result


def slpv2_burn(
    token_id: bytes,
    burn_amount: int,
) -> bytes:
    result = bytearray()
    result.extend(b"SLP2")
    result.append(0)

    result.append(len(b"BURN"))
    result.extend(b"BURN")

    result.extend(token_id)
    result.extend(burn_amount.to_bytes(6, "little"))

    return result


def slp_genesis(
    token_type: bytes = b"\x01",
    token_ticker: bytes = b"",
    token_name: bytes = b"",
    token_document_url: bytes = b"",
    token_document_hash: bytes = b"",
    decimals: int = 0,
    *,
    mint_baton_vout: Optional[int],
    initial_mint_amount: int,
) -> CScript:
    return CScript(
        [
            OP_RETURN,
            b"SLP\0",
            token_type,
            b"GENESIS",
            token_ticker,
            token_name,
            token_document_url,
            token_document_hash,
            bytes([decimals]),
            bytes([mint_baton_vout]) if mint_baton_vout else b"",
            initial_mint_amount.to_bytes(8, "big"),
        ]
    )


def slp_mint(
    token_type: bytes = b"\x01",
    *,
    token_id: bytes,
    mint_baton_vout: Optional[int],
    mint_amount: int,
) -> CScript:
    return CScript(
        [
            OP_RETURN,
            b"SLP\0",
            b"\x01",
            b"MINT",
            token_id,
            bytes([mint_baton_vout]) if mint_baton_vout else b"",
            mint_amount.to_bytes(8, "big"),
        ]
    )


def slp_send(
    token_type: bytes = b"\x01",
    *,
    token_id: bytes,
    amounts: List[int],
) -> CScript:
    ops = [
        OP_RETURN,
        b"SLP\0",
        b"\x01",
        b"SEND",
        token_id,
    ]
    for amount in amounts:
        ops.append(amount.to_bytes(8, "big"))
    return CScript(ops)


def slpv2_output(*sections: bytes) -> CTxOut:
    return CTxOut(0, CScript([OP_RETURN, OP_RESERVED] + list(sections)))


class ChronikSlp(BitcoinTestFramework):
    def set_test_params(self):
        self.setup_clean_chain = True
        self.num_nodes = 1
        self.extra_args = [["-chronik"]]

    def skip_test_if_missing_module(self):
        self.skip_if_no_chronik()

    def run_test(self):
        from test_framework.chronik.client import ChronikClient, pb
        from test_framework.chronik.test_data import genesis_cb_tx

        def slpv2_token(**kwargs) -> pb.SlpToken:
            return pb.SlpToken(token_protocol=pb.TOKEN_PROTOCOL_SLPV2, **kwargs)

        node = self.nodes[0]
        chronik = ChronikClient("127.0.0.1", node.chronik_port)

        peer = node.add_p2p_connection(P2PDataStore())
        node.setmocktime(1300000000)

        coinblockhash = self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        coinblock = node.getblock(coinblockhash)
        cointx = coinblock["tx"][0]

        block_hashes = self.generatetoaddress(node, 100, ADDRESS_ECREG_UNSPENDABLE)

        coinvalue = 5000000000
        genesis_tx = CTransaction()
        genesis_tx.vin = [CTxIn(COutPoint(int(cointx, 16), 0), SCRIPTSIG_OP_TRUE)]
        genesis_tx.vout = [
            slpv2_output(
                slpv2_genesis(
                    token_ticker=b"TEST",
                    token_name=b"Test Token",
                    url=b"http://example.com",
                    data=b"Token Data",
                    auth_pubkey=b"Token Pubkey",
                    decimals=4,
                    mint_amounts=[10, 20, 30, 0],
                    num_batons=2,
                )
            ),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(coinvalue - 100000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        genesis_tx.rehash()
        genesis_txid = genesis_tx.hash
        token_id = bytes.fromhex(genesis_txid)[::-1]
        genesis_sections = [
            pb.Slpv2Section(
                token_id=token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.GENESIS,
            )
        ]
        genesis_slp_outputs = [
            pb.SlpToken(),
            slpv2_token(token_id=token_id, amount=10),
            slpv2_token(token_id=token_id, amount=20),
            slpv2_token(token_id=token_id, amount=30),
            pb.SlpToken(),
            slpv2_token(token_id=token_id, is_mint_baton=True),
            slpv2_token(token_id=token_id, is_mint_baton=True),
        ]

        validated_genesis_proto = chronik.validate_tx(genesis_tx.serialize()).ok()
        assert_equal(list(validated_genesis_proto.slpv2_sections), genesis_sections)
        assert_equal(list(validated_genesis_proto.slp_errors), [])
        assert_equal(list(validated_genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_genesis_proto.outputs],
            genesis_slp_outputs,
        )

        chronik.broadcast_tx(genesis_tx.serialize()).ok()

        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.slpv2_sections), genesis_sections)
        assert_equal(list(genesis_proto.slp_errors), [])
        assert_equal(list(genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_tx = CTransaction()
        mint_tx.vin = [
            CTxIn(
                COutPoint(int(genesis_txid, 16), 5),
                SCRIPTSIG_OP_TRUE,
            )
        ]
        mint_tx.vout = [
            slpv2_output(
                slpv2_mint(
                    token_id=token_id,
                    mint_amounts=[5, 0],
                    num_batons=1,
                ),
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        mint_tx.rehash()
        mint_txid = mint_tx.hash

        mint_sections = [
            pb.Slpv2Section(
                token_id=token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.MINT,
            )
        ]
        mint_slp_outputs = [
            pb.SlpToken(),
            slpv2_token(token_id=token_id, amount=5),
            pb.SlpToken(),
            slpv2_token(token_id=token_id, is_mint_baton=True),
        ]

        validated_mint_proto = chronik.validate_tx(mint_tx.serialize()).ok()
        assert_equal(list(validated_mint_proto.slpv2_sections), mint_sections)
        assert_equal(list(validated_mint_proto.slp_errors), [])
        assert_equal(list(validated_mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_mint_proto.outputs], mint_slp_outputs
        )

        chronik.broadcast_tx(mint_tx.serialize()).ok()

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.slpv2_sections), mint_sections)
        assert_equal(list(mint_proto.slp_errors), [])
        assert_equal(list(mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in mint_proto.outputs],
            mint_slp_outputs,
        )

        send_tx = CTransaction()
        send_tx.vin = [
            CTxIn(
                COutPoint(int(genesis_txid, 16), 1),
                SCRIPTSIG_OP_TRUE,
            ),
            CTxIn(COutPoint(int(mint_txid, 16), 1), SCRIPTSIG_OP_TRUE),
        ]
        send_tx.vout = [
            slpv2_output(
                slpv2_send(
                    token_id=token_id,
                    output_amounts=[3, 12],
                ),
            ),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        send_tx.rehash()
        send_txid = send_tx.hash

        send_sections = [
            pb.Slpv2Section(
                token_id=token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.SEND,
            )
        ]
        send_slp_outputs = [
            pb.SlpToken(),
            slpv2_token(token_id=token_id, amount=3),
            slpv2_token(token_id=token_id, amount=12),
        ]

        validated_send_proto = chronik.validate_tx(send_tx.serialize()).ok()
        assert_equal(list(validated_send_proto.slpv2_sections), send_sections)
        assert_equal(list(validated_send_proto.slp_errors), [])
        assert_equal(list(validated_send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_send_proto.outputs], send_slp_outputs
        )

        send_txid = chronik.broadcast_tx(send_tx.serialize()).ok().txid[::-1].hex()
        send_proto = chronik.tx(send_txid).ok()

        assert_equal(list(send_proto.slpv2_sections), send_sections)
        assert_equal(list(send_proto.slp_errors), [])
        assert_equal(list(send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in send_proto.outputs],
            send_slp_outputs,
        )

        genesis2_tx = CTransaction()
        genesis2_tx.vin = [
            CTxIn(
                COutPoint(int(genesis_txid, 16), 4),
                SCRIPTSIG_OP_TRUE,
            )
        ]
        genesis2_tx.vout = [
            slpv2_output(
                slpv2_genesis(
                    mint_amounts=[100],
                    num_batons=2,
                ),
            ),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 200000, P2SH_OP_TRUE),
        ]

        token_id2 = chronik.broadcast_tx(genesis2_tx.serialize()).ok().txid
        genesis2_txid = token_id2[::-1].hex()

        multi_tx = CTransaction()
        multi_tx.vin = [
            CTxIn(COutPoint(int(send_txid, 16), 1), SCRIPTSIG_OP_TRUE),
            CTxIn(
                COutPoint(int(genesis2_txid, 16), 2),
                SCRIPTSIG_OP_TRUE,
            ),
        ]
        multi_tx.vout = [
            slpv2_output(
                slpv2_genesis(
                    mint_amounts=[0xFFFF_FFFF_FFFF, 0],
                    num_batons=1,
                ),
                slpv2_mint(
                    token_id=token_id2,
                    mint_amounts=[0, 5],
                    num_batons=0,
                ),
                slpv2_burn(
                    token_id=token_id,
                    burn_amount=1,
                ),
                slpv2_send(
                    token_id=token_id,
                    output_amounts=[0, 0, 0, 0, 2],
                ),
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        multi_tx.rehash()
        multi_txid = multi_tx.hash
        multi_token_id = bytes.fromhex(multi_txid)[::-1]

        multi_sections = [
            pb.Slpv2Section(
                token_id=multi_token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.GENESIS,
            ),
            pb.Slpv2Section(
                token_id=token_id2,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.MINT,
            ),
            pb.Slpv2Section(
                token_id=token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.SEND,
            ),
        ]
        multi_burns = [
            pb.SlpBurn(
                token_id=token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_intentional_burn=1,
                slpv2_actual_burn=1,
            )
        ]
        multi_slp_outputs = [
            pb.SlpToken(),
            slpv2_token(token_id=multi_token_id, amount=0xFFFF_FFFF_FFFF),
            slpv2_token(token_id=token_id2, amount=5, slpv2_section_idx=1),
            slpv2_token(token_id=multi_token_id, is_mint_baton=True),
            pb.SlpToken(),
            slpv2_token(token_id=token_id, amount=2, slpv2_section_idx=2),
            pb.SlpToken(),
        ]

        validated_mint_proto = chronik.validate_tx(mint_tx.serialize()).ok()
        assert_equal(list(validated_mint_proto.slpv2_sections), mint_sections)
        assert_equal(list(validated_mint_proto.slp_errors), [])
        assert_equal(list(validated_mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_mint_proto.outputs], mint_slp_outputs
        )

        chronik.broadcast_tx(multi_tx.serialize()).ok()

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.slpv2_sections), multi_sections)
        assert_equal(list(multi_proto.slp_errors), [])
        assert_equal(list(multi_proto.slp_burns), multi_burns)
        assert_equal(
            [output.slp for output in multi_proto.outputs],
            multi_slp_outputs,
        )

        all_tx = CTransaction()
        all_tx.vin = [
            CTxIn(
                COutPoint(int(genesis2_txid, 16), 3),
                SCRIPTSIG_OP_TRUE,
            ),
            CTxIn(
                COutPoint(int(genesis_txid, 16), 6),
                SCRIPTSIG_OP_TRUE,
            ),
            CTxIn(COutPoint(int(multi_txid, 16), 1), SCRIPTSIG_OP_TRUE),
        ]
        all_tx.vout = [
            slpv2_output(
                # 0: success GENESIS
                slpv2_genesis(mint_amounts=[0, 7, 0, 0, 1], num_batons=2),
                # 1: fail GENESIS: must be first
                slpv2_genesis(mint_amounts=[], num_batons=0),
                # 2: fail MINT: Too few outputs
                slpv2_mint(token_id, [0, 0, 0, 0, 0, 0, 0], 99),
                # 3: fail MINT: Overlapping amounts
                slpv2_mint(token_id, [0, 0xFFFF_FFFF_FFFF], 0),
                # 4: fail MINT: Overlapping batons
                slpv2_mint(token_id, [0], 1),
                # 5: success BURN: token ID 2
                slpv2_burn(token_id, 2),
                # 6: success MINT: token ID 3
                slpv2_mint(token_id2, [3, 0], 1),
                # 7: success MINT: token ID 2
                slpv2_mint(token_id, [0, 0, 0, 2, 0, 0, 0], 1),
                # 8: fail MINT: Duplicate token ID 2
                slpv2_mint(token_id, [], 0),
                # 9: fail BURN: Duplicate burn token ID 2
                slpv2_burn(token_id, 0),
                # 10: fail SEND: Too few outputs
                slpv2_send(multi_token_id, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                # 11: success SEND: token ID 4
                slpv2_send(
                    multi_token_id,
                    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFFFF_FFFF_FFFF],
                ),
                # 12: fail MINT: Duplicate token ID 4
                slpv2_mint(multi_token_id, [], 0),
                # 13: success UNKNOWN
                b"SLP2\x89",
                # 14: fail BURN: Descending token type
                slpv2_burn(multi_token_id, 0),
                # 15: success UNKNOWN
                b"SLP2\x9a",
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(1000, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        all_tx.rehash()
        all_txid = all_tx.hash
        all_token_id = bytes.fromhex(all_txid)[::-1]

        all_sections = [
            pb.Slpv2Section(
                token_id=all_token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.GENESIS,
            ),
            pb.Slpv2Section(
                token_id=token_id2,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.MINT,
            ),
            pb.Slpv2Section(
                token_id=token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.MINT,
            ),
            pb.Slpv2Section(
                token_id=multi_token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.SEND,
            ),
            pb.Slpv2Section(
                token_id=bytes(32),
                token_type=0x89,
                section_type=pb.UNKNOWN,
            ),
            pb.Slpv2Section(
                token_id=bytes(32),
                token_type=0x9A,
                section_type=pb.UNKNOWN,
            ),
        ]
        all_burns = [
            pb.SlpBurn(
                token_id=token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_intentional_burn=2,
                slpv2_actual_burn=0,
            )
        ]
        all_slp_errors = [
            "Error at pushdata index 1: GENESIS must be the first pushdata",
            "Error at pushdata index 2: Too few outputs, expected 107 but got 11",
            "Error at pushdata index 3: Overlapping amount",
            "Error at pushdata index 4: Overlapping mint baton",
            (
                f"Error at pushdata index 8: Duplicate token_id {genesis_txid},"
                " found in section 2"
            ),
            (
                "Error at pushdata index 9: Duplicate intentional burn token_id"
                f" {genesis_txid}, found in burn #0 and #1"
            ),
            "Error at pushdata index 10: Too few outputs, expected 13 but got 11",
            (
                f"Error at pushdata index 12: Duplicate token_id {multi_txid},"
                " found in section 3"
            ),
            (
                f"Error at pushdata index 14: Descending token type: {0x89} > 0,"
                " token types must be in ascending order"
            ),
        ]
        all_slp_outputs = [
            pb.SlpToken(),
            # success MINT: token ID 3
            slpv2_token(token_id=token_id2, amount=3, slpv2_section_idx=1),
            # success GENESIS
            slpv2_token(token_id=all_token_id, amount=7),
            # success MINT: token ID 3
            slpv2_token(token_id=token_id2, is_mint_baton=True, slpv2_section_idx=1),
            # success MINT: token ID 2
            slpv2_token(token_id=token_id, amount=2, slpv2_section_idx=2),
            # success GENESIS
            slpv2_token(token_id=all_token_id, amount=1),
            # success GENESIS
            slpv2_token(token_id=all_token_id, is_mint_baton=True),
            # success GENESIS
            slpv2_token(token_id=all_token_id, is_mint_baton=True),
            # success MINT: token ID 2
            slpv2_token(token_id=token_id, is_mint_baton=True, slpv2_section_idx=2),
            # success UNKNOWN
            slpv2_token(token_id=bytes(32), slpv2_token_type=0x89, slpv2_section_idx=4),
            # success SEND: token ID 4
            slpv2_token(
                token_id=multi_token_id,
                amount=0xFFFF_FFFF_FFFF,
                slpv2_section_idx=3,
            ),
        ]

        validated_all_proto = chronik.validate_tx(all_tx.serialize()).ok()
        assert_equal(list(validated_all_proto.slpv2_sections), all_sections)
        assert_equal(list(validated_all_proto.slp_errors), all_slp_errors)
        assert_equal(list(validated_all_proto.slp_burns), all_burns)
        assert_equal(
            [output.slp for output in validated_all_proto.outputs], all_slp_outputs
        )

        block = create_block(
            int(block_hashes[-1], 16), create_coinbase(102, b"\x03" * 33), 1300000500
        )
        block.vtx += [genesis_tx, send_tx, mint_tx, genesis2_tx, multi_tx, all_tx]
        make_conform_to_ctor(block)
        block.hashMerkleRoot = block.calc_merkle_root()
        block.solve()
        peer.send_blocks_and_test([block], node)

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.slpv2_sections), all_sections)
        assert_equal(list(all_proto.slp_errors), all_slp_errors)
        assert_equal(list(all_proto.slp_burns), all_burns)
        assert_equal(
            [output.slp for output in all_proto.outputs],
            all_slp_outputs,
        )

        non_slp_tx = CTransaction()
        non_slp_tx.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 5), SCRIPTSIG_OP_TRUE),
        ]
        pad_tx(non_slp_tx)

        non_slp_burns = [
            pb.SlpBurn(
                token_id=all_token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_actual_burn=1,
            )
        ]
        non_slp_errors = [
            "eMPP parse failed: Missing OP_RESERVED, but got [unrecognized opcode]"
        ]

        validated_non_slp_proto = chronik.validate_tx(non_slp_tx.serialize()).ok()
        assert_equal(list(validated_non_slp_proto.slpv2_sections), [])
        assert_equal(list(validated_non_slp_proto.slp_errors), non_slp_errors)
        assert_equal(list(validated_non_slp_proto.slp_burns), non_slp_burns)
        assert_equal(
            [output.slp for output in validated_non_slp_proto.outputs], [pb.SlpToken()]
        )

        assert_equal(
            chronik.broadcast_tx(non_slp_tx.serialize()).err(400).msg,
            (
                f"400: Tx {non_slp_tx.hash} failed SLP checks: Unexpected burn of SLPv2"
                f" token {all_txid}: burns 1 base tokens."
            ),
        )
        non_slp_txid = node.sendrawtransaction(non_slp_tx.serialize().hex())
        non_slp_proto = chronik.tx(non_slp_txid).ok()

        assert_equal(list(non_slp_proto.slpv2_sections), [])
        assert_equal(list(non_slp_proto.slp_burns), non_slp_burns)
        assert_equal(list(non_slp_proto.slp_errors), non_slp_errors)
        assert_equal([output.slp for output in non_slp_proto.outputs], [pb.SlpToken()])

        non_slp_tx2 = CTransaction()
        non_slp_tx2.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 6), SCRIPTSIG_OP_TRUE),
        ]
        pad_tx(non_slp_tx2)

        non_slp2_burns = [
            pb.SlpBurn(
                token_id=all_token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                burn_mint_batons=True,
            )
        ]
        non_slp2_errors = [
            "eMPP parse failed: Missing OP_RESERVED, but got [unrecognized opcode]"
        ]

        validated_non_slp2_proto = chronik.validate_tx(non_slp_tx2.serialize()).ok()
        assert_equal(list(validated_non_slp2_proto.slpv2_sections), [])
        assert_equal(list(validated_non_slp2_proto.slp_errors), non_slp2_errors)
        assert_equal(list(validated_non_slp2_proto.slp_burns), non_slp2_burns)
        assert_equal(
            [output.slp for output in validated_non_slp2_proto.outputs], [pb.SlpToken()]
        )

        assert_equal(
            chronik.broadcast_tx(non_slp_tx2.serialize()).err(400).msg,
            (
                f"400: Tx {non_slp_tx2.hash} failed SLP checks: Unexpected burn of"
                f" SLPv2 token {all_txid}: burns mint baton(s)."
            ),
        )
        non_slp_txid2 = node.sendrawtransaction(non_slp_tx2.serialize().hex())
        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.slpv2_sections), [])
        assert_equal(list(non_slp_proto2.slp_burns), non_slp2_burns)
        assert_equal(list(non_slp_proto2.slp_errors), non_slp2_errors)
        assert_equal([output.slp for output in non_slp_proto.outputs], [pb.SlpToken()])

        burn_tx = CTransaction()
        burn_tx.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 4), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 3), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 1), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 2), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 10), SCRIPTSIG_OP_TRUE),
        ]
        burn_tx.vout = [
            slpv2_output(
                slpv2_mint(token_id, [0, 2], 1),
                slpv2_burn(token_id, 1),
                slpv2_mint(token_id2, [4], 0),
                slpv2_send(all_token_id, [0, 0, 0, 8]),
                slpv2_send(multi_token_id, [0, 0, 0, 0, 1234]),
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        burn_tx.rehash()
        burn_txid = burn_tx.hash

        burn_sections = [
            pb.Slpv2Section(
                token_id=token_id2,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.MINT,
            ),
            pb.Slpv2Section(
                token_id=multi_token_id,
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.SEND,
            ),
        ]
        burn_burns = [
            pb.SlpBurn(
                token_id=token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_intentional_burn=1,
                slpv2_actual_burn=2,
                burn_error="Missing MINT baton",
            ),
            pb.SlpBurn(
                token_id=token_id2,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_actual_burn=3,
            ),
            pb.SlpBurn(
                token_id=all_token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_actual_burn=7,
                burn_error="Insufficient token input output sum",
            ),
            pb.SlpBurn(
                token_id=multi_token_id,
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_actual_burn=0xFFFF_FFFF_FFFF - 1234,
            ),
        ]
        burn_slp_outputs = [
            pb.SlpToken(),
            slpv2_token(token_id=token_id2, amount=4),
            pb.SlpToken(),
            pb.SlpToken(),
            pb.SlpToken(),
            slpv2_token(token_id=multi_token_id, amount=1234, slpv2_section_idx=1),
        ]

        validated_burn_proto = chronik.validate_tx(burn_tx.serialize()).ok()
        assert_equal(list(validated_burn_proto.slpv2_sections), burn_sections)
        assert_equal(list(validated_burn_proto.slp_errors), [])
        assert_equal(list(validated_burn_proto.slp_burns), burn_burns)
        assert_equal(
            [output.slp for output in validated_burn_proto.outputs], burn_slp_outputs
        )

        burn_block = create_block(
            int(block.hash, 16), create_coinbase(103, b"\x03" * 33), 1300000501
        )
        burn_block.vtx += [burn_tx]
        make_conform_to_ctor(burn_block)
        burn_block.hashMerkleRoot = burn_block.calc_merkle_root()
        burn_block.solve()
        peer.send_blocks_and_test([burn_block], node)

        burn_proto = chronik.tx(burn_txid).ok()
        assert_equal(list(burn_proto.slpv2_sections), burn_sections)
        assert_equal(list(burn_proto.slp_burns), burn_burns)
        assert_equal(list(burn_proto.slp_errors), [])
        assert_equal(
            [output.slp for output in burn_proto.outputs],
            burn_slp_outputs,
        )

        no_tokens_tx = CTransaction()
        no_tokens_tx.vin = [
            CTxIn(
                COutPoint(int(genesis2_txid, 16), 4),
                SCRIPTSIG_OP_TRUE,
            ),
        ]
        no_tokens_tx.vout = [
            slpv2_output(
                slpv2_burn(bytes([34] * 32), 1234),
                slpv2_send(bytes([56] * 32), [0, 0, 0]),
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(coinvalue - 300000, P2SH_OP_TRUE),
        ]
        no_tokens_tx.rehash()

        no_tokens_sections = [
            pb.Slpv2Section(
                token_id=bytes([56] * 32),
                token_type=pb.SLPV2_TOKEN_TYPE_STANDARD,
                section_type=pb.SEND,
            )
        ]
        no_tokens_burns = [
            pb.SlpBurn(
                token_id=bytes([34] * 32),
                token_protocol=pb.TOKEN_PROTOCOL_SLPV2,
                slpv2_intentional_burn=1234,
            )
        ]
        no_tokens_slp_outputs = [
            pb.SlpToken(),
            pb.SlpToken(),
            pb.SlpToken(),
            pb.SlpToken(),
            pb.SlpToken(),
        ]

        validated_no_tokens_proto = chronik.validate_tx(no_tokens_tx.serialize()).ok()
        assert_equal(list(validated_no_tokens_proto.slpv2_sections), no_tokens_sections)
        assert_equal(list(validated_no_tokens_proto.slp_errors), [])
        assert_equal(list(validated_no_tokens_proto.slp_burns), no_tokens_burns)
        assert_equal(
            [output.slp for output in validated_no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        assert_equal(
            chronik.broadcast_tx(no_tokens_tx.serialize()).err(400).msg,
            (
                f"400: Tx {no_tokens_tx.hash} failed SLP checks: Unexpected burn of"
                f" SLPv2 token {bytes([34] * 32).hex()}: Expected 1234 base tokens to"
                " be burned, but none found."
            ),
        )
        no_tokens_txid = node.sendrawtransaction(no_tokens_tx.serialize().hex())
        no_tokens_proto = chronik.tx(no_tokens_txid).ok()

        assert_equal(list(no_tokens_proto.slpv2_sections), no_tokens_sections)
        assert_equal(list(no_tokens_proto.slp_burns), no_tokens_burns)
        assert_equal(list(no_tokens_proto.slp_errors), [])
        assert_equal(
            [output.slp for output in no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        slp_genesis_tx = CTransaction()
        slp_genesis_tx.vin = [
            CTxIn(COutPoint(int(no_tokens_txid, 16), 4), SCRIPTSIG_OP_TRUE)
        ]
        slp_genesis_tx.vout = [
            CTxOut(
                0,
                slp_genesis(
                    token_ticker=b"SLPTEST",
                    token_name=b"Test SLP Token 3",
                    token_document_url=b"http://example/slp",
                    token_document_hash=b"x" * 32,
                    decimals=4,
                    mint_baton_vout=2,
                    initial_mint_amount=5000,
                ),
            ),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 400000, P2SH_OP_TRUE),
        ]
        slp_genesis_tx.rehash()
        slp_genesis_txid = slp_genesis_tx.hash
        slp_token_id = bytes.fromhex(slp_genesis_txid)

        slp_genesis_tx_data = pb.Slpv1TxData(
            token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            tx_type=pb.GENESIS,
            token_id=slp_token_id,
        )
        slp_genesis_outputs = [
            pb.SlpToken(),
            pb.SlpToken(
                token_id=slp_token_id,
                amount=5000,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
            pb.SlpToken(
                token_id=slp_token_id,
                is_mint_baton=True,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
            pb.SlpToken(),
        ]

        validated_slp_genesis_proto = chronik.validate_tx(
            slp_genesis_tx.serialize()
        ).ok()
        assert_equal(validated_slp_genesis_proto.slpv1_data, slp_genesis_tx_data)
        assert_equal(list(validated_slp_genesis_proto.slpv2_sections), [])
        assert_equal(list(validated_slp_genesis_proto.slp_errors), [])
        assert_equal(list(validated_slp_genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        chronik.broadcast_tx(slp_genesis_tx.serialize()).ok()

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(slp_genesis_proto.slpv1_data, slp_genesis_tx_data)
        assert_equal(list(slp_genesis_proto.slp_errors), [])
        assert_equal(list(slp_genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        slp_mint_tx = CTransaction()
        slp_mint_tx.vin = [
            CTxIn(COutPoint(int(slp_genesis_txid, 16), 2), SCRIPTSIG_OP_TRUE)
        ]
        slp_mint_tx.vout = [
            CTxOut(
                0, slp_mint(token_id=slp_token_id, mint_baton_vout=3, mint_amount=20)
            ),
            CTxOut(2000, P2SH_OP_TRUE),
            CTxOut(2000, P2SH_OP_TRUE),
            CTxOut(2000, P2SH_OP_TRUE),
        ]

        slp_mint_tx_data = pb.Slpv1TxData(
            token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            tx_type=pb.MINT,
            token_id=slp_token_id,
        )
        slp_mint_outputs = [
            pb.SlpToken(),
            pb.SlpToken(
                token_id=slp_token_id,
                amount=20,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
            pb.SlpToken(),
            pb.SlpToken(
                token_id=slp_token_id,
                is_mint_baton=True,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
        ]

        validated_slp_mint_proto = chronik.validate_tx(slp_mint_tx.serialize()).ok()
        assert_equal(validated_slp_mint_proto.slpv1_data, slp_mint_tx_data)
        assert_equal(list(validated_slp_mint_proto.slpv2_sections), [])
        assert_equal(list(validated_slp_mint_proto.slp_errors), [])
        assert_equal(list(validated_slp_mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_mint_txid = (
            chronik.broadcast_tx(slp_mint_tx.serialize()).ok().txid[::-1].hex()
        )
        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(slp_mint_proto.slpv1_data, slp_mint_tx_data)
        assert_equal(list(slp_mint_proto.slp_errors), [])
        assert_equal(list(slp_mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_send_tx = CTransaction()
        slp_send_tx.vin = [
            CTxIn(COutPoint(int(slp_genesis_txid, 16), 1), SCRIPTSIG_OP_TRUE)
        ]
        slp_send_tx.vout = [
            CTxOut(0, slp_send(token_id=slp_token_id, amounts=[1000, 4000])),
            CTxOut(4000, P2SH_OP_TRUE),
            CTxOut(4000, P2SH_OP_TRUE),
        ]

        slp_send_tx_data = pb.Slpv1TxData(
            token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            tx_type=pb.SEND,
            token_id=slp_token_id,
        )
        slp_send_outputs = [
            pb.SlpToken(),
            pb.SlpToken(
                token_id=slp_token_id,
                amount=1000,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
            pb.SlpToken(
                token_id=slp_token_id,
                amount=4000,
                slpv1_token_type=pb.SLPV1_TOKEN_TYPE_FUNGIBLE,
            ),
        ]

        validated_slp_send_proto = chronik.validate_tx(slp_send_tx.serialize()).ok()
        assert_equal(validated_slp_send_proto.slpv1_data, slp_send_tx_data)
        assert_equal(list(validated_slp_send_proto.slpv2_sections), [])
        assert_equal(list(validated_slp_send_proto.slp_errors), [])
        assert_equal(list(validated_slp_send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in validated_slp_send_proto.outputs],
            slp_send_outputs,
        )

        slp_send_txid = (
            chronik.broadcast_tx(slp_send_tx.serialize()).ok().txid[::-1].hex()
        )
        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(slp_send_proto.slpv1_data, slp_send_tx_data)
        assert_equal(list(slp_send_proto.slp_errors), [])
        assert_equal(list(slp_send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_send_proto.outputs],
            slp_send_outputs,
        )

        # Mine all txs and check again
        self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.slpv2_sections), genesis_sections)
        assert_equal(list(genesis_proto.slp_errors), [])
        assert_equal(list(genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.slpv2_sections), mint_sections)
        assert_equal(list(mint_proto.slp_errors), [])
        assert_equal(list(mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in mint_proto.outputs],
            mint_slp_outputs,
        )

        send_proto = chronik.tx(send_txid).ok()
        assert_equal(list(send_proto.slpv2_sections), send_sections)
        assert_equal(list(send_proto.slp_errors), [])
        assert_equal(list(send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in send_proto.outputs],
            send_slp_outputs,
        )

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.slpv2_sections), multi_sections)
        assert_equal(list(multi_proto.slp_errors), [])
        assert_equal(list(multi_proto.slp_burns), multi_burns)
        assert_equal(
            [output.slp for output in multi_proto.outputs],
            multi_slp_outputs,
        )

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.slpv2_sections), all_sections)
        assert_equal(list(all_proto.slp_errors), all_slp_errors)
        assert_equal(list(all_proto.slp_burns), all_burns)
        assert_equal(
            [output.slp for output in all_proto.outputs],
            all_slp_outputs,
        )

        non_slp_proto = chronik.tx(non_slp_txid).ok()
        assert_equal(list(non_slp_proto.slpv2_sections), [])
        assert_equal(list(non_slp_proto.slp_burns), non_slp_burns)
        assert_equal(list(non_slp_proto.slp_errors), non_slp_errors)
        assert_equal([output.slp for output in non_slp_proto.outputs], [pb.SlpToken()])

        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.slpv2_sections), [])
        assert_equal(list(non_slp_proto2.slp_burns), non_slp2_burns)
        assert_equal(list(non_slp_proto2.slp_errors), non_slp2_errors)
        assert_equal([output.slp for output in non_slp_proto2.outputs], [pb.SlpToken()])

        burn_proto = chronik.tx(burn_tx.hash).ok()
        assert_equal(list(burn_proto.slpv2_sections), burn_sections)
        assert_equal(list(burn_proto.slp_burns), burn_burns)
        assert_equal(list(burn_proto.slp_errors), [])
        assert_equal(
            [output.slp for output in burn_proto.outputs],
            burn_slp_outputs,
        )

        no_tokens_proto = chronik.tx(no_tokens_txid).ok()
        assert_equal(list(no_tokens_proto.slpv2_sections), no_tokens_sections)
        assert_equal(list(no_tokens_proto.slp_burns), no_tokens_burns)
        assert_equal(list(no_tokens_proto.slp_errors), [])
        assert_equal(
            [output.slp for output in no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(slp_genesis_proto.slpv1_data, slp_genesis_tx_data)
        assert_equal(list(slp_genesis_proto.slp_errors), [])
        assert_equal(list(slp_genesis_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(slp_mint_proto.slpv1_data, slp_mint_tx_data)
        assert_equal(list(slp_mint_proto.slp_errors), [])
        assert_equal(list(slp_mint_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(slp_send_proto.slpv1_data, slp_send_tx_data)
        assert_equal(list(slp_send_proto.slp_errors), [])
        assert_equal(list(slp_send_proto.slp_burns), [])
        assert_equal(
            [output.slp for output in slp_send_proto.outputs],
            slp_send_outputs,
        )


if __name__ == "__main__":
    ChronikSlp().main()
