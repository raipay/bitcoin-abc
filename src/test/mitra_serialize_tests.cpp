// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include <mitra/serialize.h>
#include <streams.h>
#include <util/strencodings.h>

#include <test/util/setup_common.h>

#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(mitra_serialize_tests, BasicTestingSetup)

BOOST_AUTO_TEST_CASE(example_ruck_deserialize) {
    CDataStream ss(SER_DISK, 0);

    ss.write("\x20", 1);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0x20);

    ss.write("\x7f", 1);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0x7f);

    ss.write("\x80\xfe", 2);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfe);

    ss.write("\x8f\xed", 2);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfed);

    ss.write("\x9f\xed\xcb", 3);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcb);

    ss.write("\xaf\xed\xcb\xa9", 4);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcba9);

    ss.write("\xbf\xed\xcb\xa9\x87", 5);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcba987);

    ss.write("\xcf\xed\xcb\xa9\x87\x65", 6);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcba98765);

    ss.write("\xdf\xed\xcb\xa9\x87\x65\x43", 7);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcba9876543);

    ss.write("\xef\xed\xcb\xa9\x87\x65\x43\x21", 8);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedcba987654321);

    ss.write("\xf0\xfe\xdc\xba\x98\x76\x54\x32\x10", 9);
    BOOST_CHECK_EQUAL(ReadRuckInt(ss), 0xfedc'ba98'7654'3210);
}

BOOST_AUTO_TEST_CASE(example_ruck_serialize) {
    CDataStream ss(SER_DISK, 0);

    WriteRuckInt(ss, 0x20);
    BOOST_CHECK_EQUAL(HexStr(ss), "20");
    ss.clear();

    WriteRuckInt(ss, 0x7f);
    BOOST_CHECK_EQUAL(HexStr(ss), "7f");
    ss.clear();

    WriteRuckInt(ss, 0xfe);
    BOOST_CHECK_EQUAL(HexStr(ss), "80fe");
    ss.clear();

    
    WriteRuckInt(ss, 0x0000'0000'0000'007f);
    BOOST_CHECK_EQUAL(HexStr(ss), "7f");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'00ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "80ff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'01ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "81ff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'03ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "83ff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'07ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "87ff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'0fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "8fff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'1fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "901fff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0000'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "90ffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'000f'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "9fffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'00ff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "a0ffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'0fff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "afffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0000'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "b0ffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'000f'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "bfffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'00ff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c0ffffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'0fff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "cfffffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0000'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "d0ffffffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x000f'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "dfffffffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x00ff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e0ffffffffffffff");
    ss.clear();
    
    WriteRuckInt(ss, 0x0fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "efffffffffffffff");
    ss.clear();

    WriteRuckInt(ss, 0x7fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f07fffffffffffffff");
    ss.clear();

    WriteRuckInt(ss, 0x8000'0000'0000'0000);
    BOOST_CHECK_EQUAL(HexStr(ss), "f08000000000000000");
    ss.clear();
    
    WriteRuckInt(ss, 0xffff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f0ffffffffffffffff");
    ss.clear();
}

BOOST_AUTO_TEST_CASE(example_sechet_deserialize) {
    CDataStream ss(SER_DISK, 0);

    ss.write("\x20", 1);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x20);
    ss.clear();

    ss.write("\xbf\xff", 2);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x3fff);
    ss.clear();

    ss.write("\xdf\xff\xff", 3);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x1fffff);
    ss.clear();

    ss.write("\xef\xff\xff\xff", 4);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x0fffffff);
    ss.clear();

    ss.write("\xf7\xff\xff\xff\xff", 5);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x07ffffffff);
    ss.clear();

    ss.write("\xfb\xff\xff\xff\xff\xff", 6);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x03ffffffffff);
    ss.clear();

    ss.write("\xfd\xff\xff\xff\xff\xff\xff", 7);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x01ffffffffffff);
    ss.clear();

    ss.write("\xfe\xff\xff\xff\xff\xff\xff\xff", 8);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x00ffffffffffffff);
    ss.clear();

    ss.write("\xff\xff\xff\xff\xff\xff\xff\xff\xff", 9);
    BOOST_CHECK_EQUAL(ReadSechetInt(ss), 0x00ffffffffffffffff);
}

BOOST_AUTO_TEST_CASE(example_sechet_serialize) {
    CDataStream ss(SER_DISK, 0);

    WriteSechetInt(ss, 0x20);
    BOOST_CHECK_EQUAL(HexStr(ss), "20");
    ss.clear();

    WriteSechetInt(ss, 0x7f);
    BOOST_CHECK_EQUAL(HexStr(ss), "7f");
    ss.clear();

    WriteSechetInt(ss, 0xfe);
    BOOST_CHECK_EQUAL(HexStr(ss), "80fe");
    ss.clear();

    WriteSechetInt(ss, 0x0000'0000'0000'007f);
    BOOST_CHECK_EQUAL(HexStr(ss), "7f");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'00ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "80ff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'01ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "81ff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'03ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "83ff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'07ff);
    BOOST_CHECK_EQUAL(HexStr(ss), "87ff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'0fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "8fff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'1fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "9fff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'3fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "bfff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'7fff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c07fff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0000'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c0ffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0001'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c1ffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0003'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c3ffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0007'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "c7ffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'000f'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "cfffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'001f'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "dfffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'003f'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e03fffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'007f'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e07fffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'00ff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e0ffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'01ff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e1ffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'03ff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e3ffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'07ff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "e7ffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'0fff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "efffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'1fff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f01fffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'3fff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f03fffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'7fff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f07fffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0000'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f0ffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0001'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f1ffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0003'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f3ffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0007'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f7ffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'000f'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f80fffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'001f'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f81fffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'003f'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f83fffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'007f'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f87fffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'00ff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f8ffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'01ff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "f9ffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'03ff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fbffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'07ff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fc07ffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'0fff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fc0fffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'1fff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fc1fffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'3fff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fc3fffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'7fff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fc7fffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0000'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fcffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0001'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fdffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0003'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe03ffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0007'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe07ffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x000f'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe0fffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x001f'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe1fffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x003f'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe3fffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x007f'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "fe7fffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x00ff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "feffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x01ff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff01ffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x03ff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff03ffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x07ff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff07ffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x0fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff0fffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x1fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff1fffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x3fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff3fffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0x7fff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ff7fffffffffffffff");
    ss.clear();
    
    WriteSechetInt(ss, 0xffff'ffff'ffff'ffff);
    BOOST_CHECK_EQUAL(HexStr(ss), "ffffffffffffffffff");
    ss.clear();
}

BOOST_AUTO_TEST_SUITE_END()
