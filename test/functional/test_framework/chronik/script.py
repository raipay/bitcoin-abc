#!/usr/bin/env python3
# Copyright (c) 2024 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

import unittest

from test_framework.script import CScript


class SlpScript(CScript):
    """
    SLP requires us to encode Scripts differently than CScript currently does.
    SLP forbids encoding the empty bytestring with OP_0, but CScript encodes it this
    way. Therefore, we add SlpScript, which encodes b"" as b"\x4c\x00".
    """

    @classmethod
    def _coerce_instance(cls, other):
        if isinstance(other, (bytes, bytearray, int)):
            if not other:
                return b"\x4c\x00"
        return super()._coerce_instance(other)


class TestFrameworkSlpScript(unittest.TestCase):
    def test_slp_script(self):
        # SlpScript encodes b"" as b"\4c\x00"
        self.assertEqual(SlpScript([b"abc"]), b"\x03abc")
        self.assertEqual(SlpScript([b""]), b"\x4c\x00")
        self.assertEqual(SlpScript([b"abc", b""]), b"\x03abc\x4c\x00")

        # CScript encodes it as OP_0
        self.assertEqual(CScript([b""]), b"\x00")
        self.assertEqual(CScript([b"abc", b""]), b"\x03abc\x00")
