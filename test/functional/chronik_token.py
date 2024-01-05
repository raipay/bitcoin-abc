#!/usr/bin/env python3
# Copyright (c) 2023 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.
"""
Test Chronik's SLP + ALP integration.
"""

from typing import List, Optional

from test_framework.address import (
    ADDRESS_ECREG_P2SH_OP_TRUE,
    ADDRESS_ECREG_UNSPENDABLE,
    P2SH_OP_TRUE,
    SCRIPTSIG_OP_TRUE,
)
from test_framework.blocktools import (
    create_block,
    create_coinbase,
    make_conform_to_ctor,
)
from test_framework.messages import COutPoint, CTransaction, CTxIn, CTxOut
from test_framework.p2p import P2PDataStore
from test_framework.script import (
    OP_12,
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


def alp_genesis(
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


def alp_mint(
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


def alp_send(
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


def alp_burn(
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
    mint_baton_vout: Optional[int] = None,
    mint_vault_scripthash: Optional[int] = None,
    decimals: int = 0,
    initial_mint_amount: int = 0,
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
            bytes([mint_baton_vout]) if mint_baton_vout else mint_vault_scripthash,
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
            token_type,
            b"MINT",
            token_id,
            bytes([mint_baton_vout]) if mint_baton_vout else b"",
            mint_amount.to_bytes(8, "big"),
        ]
    )


def slp_mint_vault(
    token_id: bytes,
    mint_amounts: List[int],
) -> CScript:
    return CScript(
        [
            OP_RETURN,
            b"SLP\0",
            b"\x02",
            b"MINT",
            token_id,
        ]
        + [mint_amount.to_bytes(8, "big") for mint_amount in mint_amounts]
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
        token_type,
        b"SEND",
        token_id,
    ]
    for amount in amounts:
        ops.append(amount.to_bytes(8, "big"))
    return CScript(ops)


def alp_output(*sections: bytes) -> CTxOut:
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

        def alp_token(token_type=None, **kwargs) -> pb.Token:
            return pb.Token(
                token_type=token_type or pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                **kwargs,
            )

        node = self.nodes[0]
        chronik = ChronikClient("127.0.0.1", node.chronik_port)

        peer = node.add_p2p_connection(P2PDataStore())
        mocktime = 1300000000
        node.setmocktime(mocktime)

        coinblockhash = self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]
        coinblock = node.getblock(coinblockhash)
        cointx = coinblock["tx"][0]

        block_hashes = self.generatetoaddress(node, 100, ADDRESS_ECREG_UNSPENDABLE)

        coinvalue = 5000000000
        genesis_tx = CTransaction()
        genesis_tx.vin = [CTxIn(COutPoint(int(cointx, 16), 0), SCRIPTSIG_OP_TRUE)]
        genesis_tx.vout = [
            alp_output(
                alp_genesis(
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
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.GENESIS,
                actual_burn_amount="0",
            )
        ]
        genesis_slp_outputs = [
            pb.Token(),
            alp_token(token_id=genesis_txid, amount=10),
            alp_token(token_id=genesis_txid, amount=20),
            alp_token(token_id=genesis_txid, amount=30),
            pb.Token(),
            alp_token(token_id=genesis_txid, is_mint_baton=True),
            alp_token(token_id=genesis_txid, is_mint_baton=True),
        ]

        node.sendrawtransaction(genesis_tx.serialize().hex())

        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.token_entries), genesis_sections)
        assert_equal(
            [output.token for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_tx = CTransaction()
        mint_tx.vin = [
            CTxIn(
                COutPoint(int(genesis_txid, 16), 5),
                SCRIPTSIG_OP_TRUE,
            )
        ]
        mint_tx.vout = [
            alp_output(
                alp_mint(
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
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                actual_burn_amount="0",
            )
        ]
        mint_slp_outputs = [
            pb.Token(),
            alp_token(token_id=genesis_txid, amount=5),
            pb.Token(),
            alp_token(token_id=genesis_txid, is_mint_baton=True),
        ]

        node.sendrawtransaction(mint_tx.serialize().hex())

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.token_entries), mint_sections)
        assert_equal(
            [output.token for output in mint_proto.outputs],
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
            alp_output(
                alp_send(
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
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                actual_burn_amount="0",
            )
        ]
        send_slp_outputs = [
            pb.Token(),
            alp_token(token_id=genesis_txid, amount=3),
            alp_token(token_id=genesis_txid, amount=12),
        ]

        send_txid = node.sendrawtransaction(send_tx.serialize().hex())
        send_proto = chronik.tx(send_txid).ok()

        assert_equal(list(send_proto.token_entries), send_sections)
        assert_equal(list(send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in send_proto.outputs],
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
            alp_output(
                alp_genesis(
                    mint_amounts=[100],
                    num_batons=2,
                ),
            ),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(5000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 200000, P2SH_OP_TRUE),
        ]

        genesis2_txid = node.sendrawtransaction(genesis2_tx.serialize().hex())
        token_id2 = bytes.fromhex(genesis2_txid)[::-1]

        multi_tx = CTransaction()
        multi_tx.vin = [
            CTxIn(COutPoint(int(send_txid, 16), 1), SCRIPTSIG_OP_TRUE),
            CTxIn(
                COutPoint(int(genesis2_txid, 16), 2),
                SCRIPTSIG_OP_TRUE,
            ),
        ]
        multi_tx.vout = [
            alp_output(
                alp_genesis(
                    token_ticker=b"MULTI",
                    mint_amounts=[0xFFFF_FFFF_FFFF, 0],
                    num_batons=1,
                ),
                alp_mint(
                    token_id=token_id2,
                    mint_amounts=[0, 5],
                    num_batons=0,
                ),
                alp_burn(
                    token_id=token_id,
                    burn_amount=1,
                ),
                alp_send(
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
            pb.TokenEntry(
                token_id=multi_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.GENESIS,
                actual_burn_amount="0",
            ),
            pb.TokenEntry(
                token_id=genesis2_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                actual_burn_amount="0",
            ),
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                intentional_burn=1,
                actual_burn_amount="1",
            ),
        ]
        multi_slp_outputs = [
            pb.Token(),
            alp_token(token_id=multi_txid, amount=0xFFFF_FFFF_FFFF),
            alp_token(token_id=genesis2_txid, amount=5, entry_idx=1),
            alp_token(token_id=multi_txid, is_mint_baton=True),
            pb.Token(),
            alp_token(token_id=genesis_txid, amount=2, entry_idx=2),
            pb.Token(),
        ]

        node.sendrawtransaction(multi_tx.serialize().hex())

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.token_entries), multi_sections)
        assert_equal(list(multi_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in multi_proto.outputs],
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
            alp_output(
                # 0: success GENESIS
                alp_genesis(
                    token_ticker=b"ALL",
                    mint_amounts=[0, 7, 0, 0, 1],
                    num_batons=2,
                ),
                # 1: fail GENESIS: must be first
                alp_genesis(mint_amounts=[], num_batons=0),
                # 2: fail MINT: Too few outputs
                alp_mint(token_id, [0, 0, 0, 0, 0, 0, 0], 99),
                # 3: fail MINT: Overlapping amounts
                alp_mint(token_id, [0, 0xFFFF_FFFF_FFFF], 0),
                # 4: fail MINT: Overlapping batons
                alp_mint(token_id, [0], 1),
                # 5: success BURN: token ID 2
                alp_burn(token_id, 2),
                # 6: success MINT: token ID 3
                alp_mint(token_id2, [3, 0], 1),
                # 7: success MINT: token ID 2
                alp_mint(token_id, [0, 0, 0, 2, 0, 0, 0], 1),
                # 8: fail MINT: Duplicate token ID 2
                alp_mint(token_id, [], 0),
                # 9: fail BURN: Duplicate burn token ID 2
                alp_burn(token_id, 0),
                # 10: fail SEND: Too few outputs
                alp_send(multi_token_id, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123]),
                # 11: success SEND: token ID 4
                alp_send(
                    multi_token_id,
                    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFFFF_FFFF_FFFF],
                ),
                # 12: fail MINT: Duplicate token ID 4
                alp_mint(multi_token_id, [], 0),
                # 13: success UNKNOWN
                b"SLP2\x89",
                # 14: fail BURN: Descending token type
                alp_burn(multi_token_id, 0),
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
            pb.TokenEntry(
                token_id=all_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.GENESIS,
                actual_burn_amount="0",
                burn_summary="Invalid coloring at pushdata idx 1: GENESIS must be the first pushdata",
                failed_colorings=[
                    pb.TokenFailedColoring(
                        pushdata_idx=1,
                        error="GENESIS must be the first pushdata",
                    )
                ],
            ),
            pb.TokenEntry(
                token_id=genesis2_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                actual_burn_amount="0",
            ),
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                intentional_burn=2,
                actual_burn_amount="0",
                burn_summary=f"Invalid coloring at pushdata idx 2: Too few outputs, expected 107 but got 11. Invalid coloring at pushdata idx 3: Overlapping amount when trying to color 281474976710655 at index 2, output is already colored with 7 of {all_txid} (ALP STANDARD (V0)). Invalid coloring at pushdata idx 4: Overlapping mint baton when trying to color mint baton at index 2, output is already colored with 7 of {all_txid} (ALP STANDARD (V0)). Invalid coloring at pushdata idx 8: Duplicate token_id {genesis_txid}, found in section 2. Invalid coloring at pushdata idx 9: Duplicate intentional burn token_id {genesis_txid}, found in burn #0 and #1",
                failed_colorings=[
                    pb.TokenFailedColoring(
                        pushdata_idx=2,
                        error="Too few outputs, expected 107 but got 11",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=3,
                        error=f"Overlapping amount when trying to color 281474976710655 at index 2, output is already colored with 7 of {all_txid} (ALP STANDARD (V0))",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=4,
                        error=f"Overlapping mint baton when trying to color mint baton at index 2, output is already colored with 7 of {all_txid} (ALP STANDARD (V0))",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=8,
                        error=f"Duplicate token_id {genesis_txid}, found in section 2",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=9,
                        error=f"Duplicate intentional burn token_id {genesis_txid}, found in burn #0 and #1",
                    ),
                ],
            ),
            pb.TokenEntry(
                token_id=multi_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                actual_burn_amount="0",
                burn_summary=f"Invalid coloring at pushdata idx 10: Too few outputs, expected 13 but got 11. Invalid coloring at pushdata idx 12: Duplicate token_id {multi_txid}, found in section 3. Invalid coloring at pushdata idx 14: Descending token type: 137 > 0, token types must be in ascending order",
                failed_colorings=[
                    pb.TokenFailedColoring(
                        pushdata_idx=10,
                        error="Too few outputs, expected 13 but got 11",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=12,
                        error=f"Duplicate token_id {multi_txid}, found in section 3",
                    ),
                    pb.TokenFailedColoring(
                        pushdata_idx=14,
                        error="Descending token type: 137 > 0, token types must be in ascending order",
                    ),
                ],
            ),
            pb.TokenEntry(
                token_id="00" * 32,
                token_type=pb.TokenType(alp=0x89),
                tx_type=pb.UNKNOWN,
                actual_burn_amount="0",
            ),
            pb.TokenEntry(
                token_id="00" * 32,
                token_type=pb.TokenType(alp=0x9A),
                tx_type=pb.UNKNOWN,
                actual_burn_amount="0",
            ),
        ]
        all_slp_outputs = [
            pb.Token(),
            # success MINT: token ID 3
            alp_token(token_id=genesis2_txid, amount=3, entry_idx=1),
            # success GENESIS
            alp_token(token_id=all_txid, amount=7),
            # success MINT: token ID 3
            alp_token(token_id=genesis2_txid, is_mint_baton=True, entry_idx=1),
            # success MINT: token ID 2
            alp_token(token_id=genesis_txid, amount=2, entry_idx=2),
            # success GENESIS
            alp_token(token_id=all_txid, amount=1),
            # success GENESIS
            alp_token(token_id=all_txid, is_mint_baton=True),
            # success GENESIS
            alp_token(token_id=all_txid, is_mint_baton=True),
            # success MINT: token ID 2
            alp_token(token_id=genesis_txid, is_mint_baton=True, entry_idx=2),
            # success UNKNOWN
            alp_token(
                token_id="00" * 32, token_type=pb.TokenType(alp=0x89), entry_idx=4
            ),
            # success SEND: token ID 4
            alp_token(
                token_id=multi_txid,
                amount=0xFFFF_FFFF_FFFF,
                entry_idx=3,
            ),
        ]

        block_height = 102
        block = create_block(
            int(block_hashes[-1], 16),
            create_coinbase(block_height, b"\x03" * 33),
            1300000500,
        )
        block.vtx += [genesis_tx, send_tx, mint_tx, genesis2_tx, multi_tx, all_tx]
        make_conform_to_ctor(block)
        block.hashMerkleRoot = block.calc_merkle_root()
        block.solve()
        peer.send_blocks_and_test([block], node)

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.token_entries), all_sections)
        assert_equal(list(all_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in all_proto.outputs],
            all_slp_outputs,
        )

        non_slp_tx = CTransaction()
        non_slp_tx.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 5), SCRIPTSIG_OP_TRUE),
        ]
        pad_tx(non_slp_tx)

        non_slp_txid = node.sendrawtransaction(non_slp_tx.serialize().hex())
        non_slp_proto = chronik.tx(non_slp_txid).ok()

        non_slp_entries = [
            pb.TokenEntry(
                token_id=all_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                is_invalid=True,
                actual_burn_amount="1",
                burn_summary="Unexpected burn: Burns 1 base tokens",
            ),
        ]
        assert_equal(list(non_slp_proto.token_entries), non_slp_entries)
        assert_equal(list(non_slp_proto.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto.outputs], [pb.Token()])

        non_slp_tx2 = CTransaction()
        non_slp_tx2.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 6), SCRIPTSIG_OP_TRUE),
        ]
        pad_tx(non_slp_tx2)

        non_slp_entries2 = [
            pb.TokenEntry(
                token_id=all_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                is_invalid=True,
                actual_burn_amount="0",
                burns_mint_batons=True,
                burn_summary="Unexpected burn: Burns mint baton(s)",
            ),
        ]
        non_slp_txid2 = node.sendrawtransaction(non_slp_tx2.serialize().hex())
        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.token_entries), non_slp_entries2)
        assert_equal(list(non_slp_proto2.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto2.outputs], [pb.Token()])

        burn_tx = CTransaction()
        burn_tx.vin = [
            CTxIn(COutPoint(int(all_txid, 16), 4), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 3), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 1), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 2), SCRIPTSIG_OP_TRUE),
            CTxIn(COutPoint(int(all_txid, 16), 10), SCRIPTSIG_OP_TRUE),
        ]
        burn_tx.vout = [
            alp_output(
                alp_mint(token_id, [0, 2], 1),
                alp_burn(token_id, 1),
                alp_mint(token_id2, [4], 0),
                alp_send(all_token_id, [0, 0, 0, 8]),
                alp_send(multi_token_id, [0, 0, 0, 0, 1234]),
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
            pb.TokenEntry(
                token_id=genesis_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                is_invalid=True,
                burn_summary="Unexpected burn: Burns 2 base tokens, but intended to burn 1; burned 1 too many. Reason(s): Missing MINT baton",
                actual_burn_amount="2",
                intentional_burn=1,
            ),
            pb.TokenEntry(
                token_id=genesis2_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.MINT,
                actual_burn_amount="3",
                burn_summary="Unexpected burn: Burns 3 base tokens",
            ),
            pb.TokenEntry(
                token_id=all_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                is_invalid=True,
                actual_burn_amount="7",
                burn_summary="Unexpected burn: Burns 7 base tokens. Reason(s): Insufficient token input output sum: 7 < 8",
            ),
            pb.TokenEntry(
                token_id=multi_txid,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                burn_summary="Unexpected burn: Burns 281474976709421 base tokens",
                actual_burn_amount=str(0xFFFF_FFFF_FFFF - 1234),
            ),
        ]
        burn_slp_outputs = [
            pb.Token(),
            alp_token(token_id=genesis2_txid, amount=4, entry_idx=1),
            pb.Token(),
            pb.Token(),
            pb.Token(),
            alp_token(token_id=multi_txid, amount=1234, entry_idx=3),
        ]

        burn_block = create_block(
            int(block.hash, 16), create_coinbase(103, b"\x03" * 33), 1300000501
        )
        burn_block.vtx += [burn_tx]
        make_conform_to_ctor(burn_block)
        burn_block.hashMerkleRoot = burn_block.calc_merkle_root()
        burn_block.solve()
        peer.send_blocks_and_test([burn_block], node)

        burn_proto = chronik.tx(burn_txid).ok()
        assert_equal(list(burn_proto.token_entries), burn_sections)
        assert_equal(list(burn_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in burn_proto.outputs],
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
            alp_output(
                alp_burn(bytes([0x34] * 32), 1234),
                alp_send(bytes([0x56] * 32), [0, 0, 0]),
            ),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(546, P2SH_OP_TRUE),
            CTxOut(coinvalue - 300000, P2SH_OP_TRUE),
        ]
        no_tokens_tx.rehash()

        no_tokens_sections = [
            pb.TokenEntry(
                token_id="56" * 32,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.SEND,
                actual_burn_amount="0",
            ),
            pb.TokenEntry(
                token_id="34" * 32,
                token_type=pb.TokenType(alp=pb.ALP_TOKEN_TYPE_STANDARD),
                tx_type=pb.BURN,
                intentional_burn=1234,
                actual_burn_amount="0",
                burn_summary="Unexpected burn: Expected 1234 base tokens to be burned, but none found",
            ),
        ]
        no_tokens_slp_outputs = [
            pb.Token(),
            pb.Token(),
            pb.Token(),
            pb.Token(),
            pb.Token(),
        ]

        no_tokens_txid = node.sendrawtransaction(no_tokens_tx.serialize().hex())
        no_tokens_proto = chronik.tx(no_tokens_txid).ok()

        assert_equal(list(no_tokens_proto.token_entries), no_tokens_sections)
        assert_equal(list(no_tokens_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in no_tokens_proto.outputs],
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

        slp_genesis_outputs = [
            pb.Token(),
            pb.Token(
                token_id=slp_genesis_txid,
                amount=5000,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
            pb.Token(
                token_id=slp_genesis_txid,
                is_mint_baton=True,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
            pb.Token(),
        ]

        slp_genesis_sections = [
            pb.TokenEntry(
                token_id=slp_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                tx_type=pb.GENESIS,
                actual_burn_amount="0",
            ),
        ]

        node.sendrawtransaction(slp_genesis_tx.serialize().hex())

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(list(slp_genesis_proto.token_entries), slp_genesis_sections)
        assert_equal(list(slp_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_genesis_proto.outputs],
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

        slp_mint_sections = [
            pb.TokenEntry(
                token_id=slp_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                tx_type=pb.MINT,
                actual_burn_amount="0",
            ),
        ]
        slp_mint_outputs = [
            pb.Token(),
            pb.Token(
                token_id=slp_genesis_txid,
                amount=20,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
            pb.Token(),
            pb.Token(
                token_id=slp_genesis_txid,
                is_mint_baton=True,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
        ]

        slp_mint_txid = node.sendrawtransaction(slp_mint_tx.serialize().hex())
        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(list(slp_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_mint_proto.outputs],
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

        slp_send_sections = [
            pb.TokenEntry(
                token_id=slp_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
                tx_type=pb.SEND,
                actual_burn_amount="0",
            ),
        ]
        slp_send_outputs = [
            pb.Token(),
            pb.Token(
                token_id=slp_genesis_txid,
                amount=1000,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
            pb.Token(
                token_id=slp_genesis_txid,
                amount=4000,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_FUNGIBLE),
            ),
        ]

        slp_send_txid = node.sendrawtransaction(slp_send_tx.serialize().hex())
        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(list(slp_send_proto.token_entries), slp_send_sections)
        assert_equal(list(slp_send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_send_proto.outputs],
            slp_send_outputs,
        )

        mint_vault_script = CScript([OP_12])
        mint_vault_scripthash = hash160(mint_vault_script)
        vault_genesis_tx = CTransaction()
        vault_genesis_tx.vin = [
            CTxIn(COutPoint(int(slp_genesis_txid, 16), 3), SCRIPTSIG_OP_TRUE)
        ]
        vault_genesis_tx.vout = [
            CTxOut(
                0,
                slp_genesis(
                    token_type=b"\x02",
                    token_ticker=b"SLPVAULT",
                    token_name=b"0",
                    token_document_url=b"0",
                    token_document_hash=b"x" * 32,
                    mint_vault_scripthash=mint_vault_scripthash,
                    initial_mint_amount=1000,
                ),
            ),
            CTxOut(10000, P2SH_OP_TRUE),
            CTxOut(coinvalue - 500000, P2SH_OP_TRUE),
        ]
        vault_genesis_tx.rehash()
        vault_genesis_txid = vault_genesis_tx.hash
        vault_token_id = bytes.fromhex(vault_genesis_txid)

        vault_genesis_outputs = [
            pb.Token(),
            pb.Token(
                token_id=vault_genesis_txid,
                amount=1000,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_MINT_VAULT),
            ),
            pb.Token(),
        ]
        vault_genesis_sections = [
            pb.TokenEntry(
                token_id=vault_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_MINT_VAULT),
                tx_type=pb.GENESIS,
                actual_burn_amount="0",
            ),
        ]

        node.sendrawtransaction(vault_genesis_tx.serialize().hex())
        vault_genesis_proto = chronik.tx(vault_genesis_txid).ok()
        assert_equal(list(vault_genesis_proto.token_entries), vault_genesis_sections)
        assert_equal(list(vault_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in vault_genesis_proto.outputs],
            vault_genesis_outputs,
        )

        # Setup vault UTXO
        vault_setup_tx = CTransaction()
        vault_setup_tx.vin = [
            CTxIn(COutPoint(int(slp_mint_txid, 16), 2), SCRIPTSIG_OP_TRUE)
        ]
        vault_setup_tx.vout = [
            CTxOut(1000, CScript([OP_HASH160, mint_vault_scripthash, OP_EQUAL]))
        ]
        pad_tx(vault_setup_tx)
        vault_setup_txid = node.sendrawtransaction(vault_setup_tx.serialize().hex())
        vault_setup_proto = chronik.tx(vault_setup_txid).ok()
        assert_equal(
            vault_setup_proto.outputs[0].output_script,
            bytes(CScript([OP_HASH160, mint_vault_scripthash, OP_EQUAL])),
        )

        # MINT
        vault_mint_tx = CTransaction()
        vault_mint_tx.vin = [
            CTxIn(
                COutPoint(int(vault_setup_txid, 16), 0),
                CScript([bytes(CScript([OP_12]))]),
            )
        ]
        vault_mint_tx.vout = [
            CTxOut(
                0,
                slp_mint_vault(
                    token_id=vault_token_id,
                    mint_amounts=[6000000],
                ),
            ),
            CTxOut(546, P2SH_OP_TRUE),
        ]
        vault_mint_tx.rehash()
        vault_mint_txid = vault_mint_tx.hash

        # Must wait for 1 confirmation
        vault_mint_sections = [
            pb.TokenEntry(
                token_id=vault_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_MINT_VAULT),
                tx_type=pb.MINT,
                burn_summary="Missing MINT vault",
                actual_burn_amount="0",
            ),
        ]

        # Mine all txs and check again
        self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]

        vault_mint_outputs = [
            pb.Token(),
            pb.Token(
                token_id=vault_genesis_txid,
                amount=6000000,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_MINT_VAULT),
            ),
        ]
        vault_mint_sections = [
            pb.TokenEntry(
                token_id=vault_genesis_txid,
                token_type=pb.TokenType(slp=pb.SLP_TOKEN_TYPE_MINT_VAULT),
                tx_type=pb.MINT,
                actual_burn_amount="0",
            ),
        ]

        vault_mint_txid = node.sendrawtransaction(vault_mint_tx.serialize().hex())

        vault_mint_proto = chronik.tx(vault_mint_txid).ok()
        assert_equal(list(vault_mint_proto.token_entries), vault_mint_sections)
        assert_equal(list(vault_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in vault_mint_proto.outputs],
            vault_mint_outputs,
        )

        # Mine mint vault tx
        self.generatetoaddress(node, 1, ADDRESS_ECREG_P2SH_OP_TRUE)[0]

        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.token_entries), genesis_sections)
        assert_equal(list(genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.token_entries), mint_sections)
        assert_equal(list(mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in mint_proto.outputs],
            mint_slp_outputs,
        )

        send_proto = chronik.tx(send_txid).ok()
        assert_equal(list(send_proto.token_entries), send_sections)
        assert_equal(list(send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in send_proto.outputs],
            send_slp_outputs,
        )

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.token_entries), multi_sections)
        assert_equal(list(multi_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in multi_proto.outputs],
            multi_slp_outputs,
        )

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.token_entries), all_sections)
        assert_equal(list(all_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in all_proto.outputs],
            all_slp_outputs,
        )

        print("non_slp_txid =", non_slp_txid)
        non_slp_proto = chronik.tx(non_slp_txid).ok()
        assert_equal(list(non_slp_proto.token_entries), non_slp_entries)
        assert_equal(list(non_slp_proto.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto.outputs], [pb.Token()])

        print("non_slp_txid2 =", non_slp_txid2)
        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.token_entries), non_slp_entries2)
        assert_equal(list(non_slp_proto2.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto2.outputs], [pb.Token()])

        burn_proto = chronik.tx(burn_tx.hash).ok()
        assert_equal(list(burn_proto.token_entries), burn_sections)
        assert_equal(list(burn_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in burn_proto.outputs],
            burn_slp_outputs,
        )

        print("no_tokens_txid =", no_tokens_txid)
        no_tokens_proto = chronik.tx(no_tokens_txid).ok()
        assert_equal(list(no_tokens_proto.token_entries), no_tokens_sections)
        assert_equal(list(no_tokens_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(list(slp_genesis_proto.token_entries), slp_genesis_sections)
        assert_equal(list(slp_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(list(slp_mint_proto.token_entries), slp_mint_sections)
        assert_equal(list(slp_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(list(slp_send_proto.token_entries), slp_send_sections)
        assert_equal(list(slp_send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_send_proto.outputs],
            slp_send_outputs,
        )

        vault_genesis_proto = chronik.tx(vault_genesis_txid).ok()
        assert_equal(list(vault_genesis_proto.token_entries), vault_genesis_sections)
        assert_equal(list(vault_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in vault_genesis_proto.outputs],
            vault_genesis_outputs,
        )

        print("vault_mint_txid = ", vault_mint_txid)
        vault_mint_proto = chronik.tx(vault_mint_txid).ok()
        assert_equal(list(vault_mint_proto.token_entries), vault_mint_sections)
        assert_equal(list(vault_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in vault_mint_proto.outputs], vault_mint_outputs
        )

        # Undo 3 blocks, then mine
        node.invalidateblock(block.hash)

        reorg_height = 102
        reorg_timestamp = 1300000500
        reorg = create_block(
            int(block_hashes[-1], 16),
            create_coinbase(reorg_height, b"\x03" * 33),
            reorg_timestamp,
        )
        reorg.vtx += [
            genesis_tx,
            send_tx,
            mint_tx,
            genesis2_tx,
            multi_tx,
            all_tx,
            non_slp_tx,
            non_slp_tx2,
            burn_tx,
            no_tokens_tx,
            slp_genesis_tx,
            slp_mint_tx,
            slp_send_tx,
            vault_genesis_tx,
        ]
        make_conform_to_ctor(reorg)
        reorg.hashMerkleRoot = reorg.calc_merkle_root()
        reorg.solve()
        peer.send_blocks_and_test([reorg], node)

        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.token_entries), genesis_sections)
        assert_equal(list(genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.token_entries), mint_sections)
        assert_equal(list(mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in mint_proto.outputs],
            mint_slp_outputs,
        )

        send_proto = chronik.tx(send_txid).ok()
        assert_equal(list(send_proto.token_entries), send_sections)
        assert_equal(list(send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in send_proto.outputs],
            send_slp_outputs,
        )

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.token_entries), multi_sections)
        assert_equal(list(multi_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in multi_proto.outputs],
            multi_slp_outputs,
        )

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.token_entries), all_sections)
        assert_equal(list(all_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in all_proto.outputs],
            all_slp_outputs,
        )

        non_slp_proto = chronik.tx(non_slp_txid).ok()
        assert_equal(list(non_slp_proto.token_entries), non_slp_entries)
        assert_equal(list(non_slp_proto.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto.outputs], [pb.Token()])

        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.token_entries), non_slp_entries2)
        assert_equal(list(non_slp_proto2.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto2.outputs], [pb.Token()])

        burn_proto = chronik.tx(burn_tx.hash).ok()
        assert_equal(list(burn_proto.token_entries), burn_sections)
        assert_equal(list(burn_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in burn_proto.outputs],
            burn_slp_outputs,
        )

        no_tokens_proto = chronik.tx(no_tokens_txid).ok()
        assert_equal(list(no_tokens_proto.token_entries), no_tokens_sections)
        assert_equal(list(no_tokens_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(list(slp_genesis_proto.token_entries), slp_genesis_sections)
        assert_equal(list(slp_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(list(slp_mint_proto.token_entries), slp_mint_sections)
        assert_equal(list(slp_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(list(slp_send_proto.token_entries), slp_send_sections)
        assert_equal(list(slp_send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_send_proto.outputs],
            slp_send_outputs,
        )

        self.restart_node(0, ["-chronik"])

        genesis_proto = chronik.tx(genesis_txid).ok()
        assert_equal(list(genesis_proto.token_entries), genesis_sections)
        assert_equal(list(genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in genesis_proto.outputs], genesis_slp_outputs
        )

        mint_proto = chronik.tx(mint_txid).ok()
        assert_equal(list(mint_proto.token_entries), mint_sections)
        assert_equal(list(mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in mint_proto.outputs],
            mint_slp_outputs,
        )

        send_proto = chronik.tx(send_txid).ok()
        assert_equal(list(send_proto.token_entries), send_sections)
        assert_equal(list(send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in send_proto.outputs],
            send_slp_outputs,
        )

        multi_proto = chronik.tx(multi_txid).ok()
        assert_equal(list(multi_proto.token_entries), multi_sections)
        assert_equal(list(multi_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in multi_proto.outputs],
            multi_slp_outputs,
        )

        all_proto = chronik.tx(all_txid).ok()
        assert_equal(list(all_proto.token_entries), all_sections)
        assert_equal(list(all_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in all_proto.outputs],
            all_slp_outputs,
        )

        non_slp_proto = chronik.tx(non_slp_txid).ok()
        assert_equal(list(non_slp_proto.token_entries), non_slp_entries)
        assert_equal(list(non_slp_proto.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto.outputs], [pb.Token()])

        non_slp_proto2 = chronik.tx(non_slp_txid2).ok()
        assert_equal(list(non_slp_proto2.token_entries), non_slp_entries2)
        assert_equal(list(non_slp_proto2.token_failed_parsings), [])
        assert_equal([output.token for output in non_slp_proto2.outputs], [pb.Token()])

        burn_proto = chronik.tx(burn_tx.hash).ok()
        assert_equal(list(burn_proto.token_entries), burn_sections)
        assert_equal(list(burn_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in burn_proto.outputs],
            burn_slp_outputs,
        )

        no_tokens_proto = chronik.tx(no_tokens_txid).ok()
        assert_equal(list(no_tokens_proto.token_entries), no_tokens_sections)
        assert_equal(list(no_tokens_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in no_tokens_proto.outputs],
            no_tokens_slp_outputs,
        )

        slp_genesis_proto = chronik.tx(slp_genesis_txid).ok()
        assert_equal(list(slp_genesis_proto.token_entries), slp_genesis_sections)
        assert_equal(list(slp_genesis_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_genesis_proto.outputs],
            slp_genesis_outputs,
        )

        slp_mint_proto = chronik.tx(slp_mint_txid).ok()
        assert_equal(list(slp_mint_proto.token_entries), slp_mint_sections)
        assert_equal(list(slp_mint_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_mint_proto.outputs],
            slp_mint_outputs,
        )

        slp_send_proto = chronik.tx(slp_send_txid).ok()
        assert_equal(list(slp_send_proto.token_entries), slp_send_sections)
        assert_equal(list(slp_send_proto.token_failed_parsings), [])
        assert_equal(
            [output.token for output in slp_send_proto.outputs],
            slp_send_outputs,
        )


if __name__ == "__main__":
    ChronikSlp().main()
