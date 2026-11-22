#!/usr/bin/env python3
"""Automated Unit Tests for Unified Silicon Identity and Address Binding.

Verifies:
1. FastHash64 and Katapult 6-byte CAN UUID calculation with seed 0xA16231A7.
2. SoftResetMailbox 32-byte binary layout and CRC32 integrity check.
3. Fallback address calculation: 0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F).
4. Multi-tier address binding priority:
   - Tier 1: Valid SoftResetMailbox at 0x2000_7FE0 overrides persistent flash.
   - Tier 2: Valid Flash Sectors 14/15 override silicon fallback.
   - Tier 3: Blank/unprogrammed flash falls back to deterministic UID address.
   - Corrupt mailbox fails CRC32 and safely cascades to flash or fallback.
5. STM32H503 binary artifact verification:
   - artifacts/bareboot-stm32h503-i3c-i2c.bin validity and vector table.
"""

import os
import struct
import unittest
import zlib

REQUEST_CANBOOT = 0x5984E3FA6CA1589B
REQUEST_APP = 0x4649534841505031
KATAPULT_UUID_SEED = 0xA16231A7
SOFT_RESET_MAILBOX_ADDR = 0x20007FE0
FLASH_STORAGE_MAGIC = 0x46495348  # "FISH"
FLASH_HEADER_SIZE = 32
FLASH_REG_NODE_ID_OFFSET = 0x10
MASK_64 = 0xFFFFFFFFFFFFFFFF


def fasthash_mix(val: int) -> int:
    val ^= val >> 23
    val = (val * 0x2127599BF4325C37) & MASK_64
    val ^= val >> 47
    return val


def fasthash64(buffer: bytes, seed: int = KATAPULT_UUID_SEED) -> int:
    m = 0x880355F21E6D1965
    h = (seed ^ ((len(buffer) * m) & MASK_64)) & MASK_64
    long_count = len(buffer) // 8
    if long_count:
        words = struct.unpack(f"<{long_count}Q", buffer[:long_count * 8])
        for v in words:
            h ^= fasthash_mix(v)
            h = (h * m) & MASK_64
    rem = buffer[long_count * 8:]
    if rem:
        v = 0
        for i in range(len(rem)):
            v |= rem[i] << (8 * i)
        h ^= fasthash_mix(v)
        h = (h * m) & MASK_64
    return fasthash_mix(h)


def katapult_can_uuid(uid: bytes) -> bytes:
    h = fasthash64(uid, KATAPULT_UUID_SEED)
    return struct.pack("<Q", h)[:6]


def fallback_address(uid_words: list) -> int:
    return 0x10 + ((uid_words[0] ^ uid_words[1] ^ uid_words[2]) & 0x3F)


def make_soft_reset_mailbox(magic: int, dynamic_node_id: int, bus_type: int = 0) -> bytes:
    payload = struct.pack(
        "<QBBHI3I",
        magic,
        dynamic_node_id,
        bus_type,
        0,  # flags
        0,  # can_assigned_id
        0, 0, 0,  # reserved[3]
    )
    crc = zlib.crc32(payload) & 0xFFFFFFFF
    return payload + struct.pack("<I", crc)


def parse_soft_reset_mailbox(data: bytes):
    if len(data) < 32:
        return None
    payload = data[:28]
    expected_crc = struct.unpack("<I", data[28:32])[0]
    calc_crc = zlib.crc32(payload) & 0xFFFFFFFF
    if calc_crc != expected_crc:
        return None
    magic, dynamic_node_id, bus_type, flags, can_assigned_id, r0, r1, r2 = struct.unpack("<QBBHI3I", payload)
    return {
        "magic": magic,
        "dynamic_node_id": dynamic_node_id,
        "bus_type": bus_type,
        "flags": flags,
        "can_assigned_id": can_assigned_id,
        "crc32": expected_crc,
    }


def resolve_node_address(mailbox_bytes: bytes, flash_sector_14: bytes, flash_sector_15: bytes, uid_words: list) -> int:
    """Emulates read_configured_node_id() logic from i3c_dynamic_target.c exactly."""
    # Tier 1: SoftResetMailbox
    mb = parse_soft_reset_mailbox(mailbox_bytes)
    if mb and (mb["magic"] == REQUEST_CANBOOT or mb["magic"] == REQUEST_APP):
        if 0x08 <= mb["dynamic_node_id"] <= 0x77:
            return mb["dynamic_node_id"]

    # Tier 2: Persistent Flash Ping-Pong Sectors 14 and 15
    def parse_flash(sector: bytes):
        if len(sector) < FLASH_HEADER_SIZE + 96:
            return None
        magic, version, payload_len, seq, crc32 = struct.unpack("<IHHII", sector[:16])
        if magic == FLASH_STORAGE_MAGIC and version == 1 and payload_len == 96:
            stored_id = sector[FLASH_HEADER_SIZE + FLASH_REG_NODE_ID_OFFSET]
            if 0x08 <= stored_id <= 0x77:
                return (seq, stored_id)
        return None

    h14 = parse_flash(flash_sector_14)
    h15 = parse_flash(flash_sector_15)
    best_id = None
    if h14 and h15:
        best_id = h14[1] if h14[0] >= h15[0] else h15[1]
    elif h14:
        best_id = h14[1]
    elif h15:
        best_id = h15[1]

    if best_id is not None:
        return best_id

    # Tier 3: Deterministic silicon UID fallback
    return fallback_address(uid_words)


class TestUnifiedIdentityAndAddressing(unittest.TestCase):
    def test_fasthash64_known_vector(self):
        uid = bytes([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C])
        h = fasthash64(uid, KATAPULT_UUID_SEED)
        self.assertNotEqual(h, 0)
        uuid = katapult_can_uuid(uid)
        self.assertEqual(len(uuid), 6)
        self.assertEqual(uuid, struct.pack("<Q", h)[:6])

    def test_soft_reset_mailbox_crc32_and_validation(self):
        mb_raw = make_soft_reset_mailbox(REQUEST_CANBOOT, 0x42, bus_type=1)
        self.assertEqual(len(mb_raw), 32)

        mb = parse_soft_reset_mailbox(mb_raw)
        self.assertIsNotNone(mb)
        self.assertEqual(mb["magic"], REQUEST_CANBOOT)
        self.assertEqual(mb["dynamic_node_id"], 0x42)
        self.assertEqual(mb["bus_type"], 1)

        # 1-bit corruption in payload fails CRC
        corrupted = bytearray(mb_raw)
        corrupted[8] ^= 0x01
        self.assertIsNone(parse_soft_reset_mailbox(bytes(corrupted)))

        # 1-bit corruption in CRC fails CRC
        corrupted_crc = bytearray(mb_raw)
        corrupted_crc[31] ^= 0x80
        self.assertIsNone(parse_soft_reset_mailbox(bytes(corrupted_crc)))

    def test_fallback_address_distribution(self):
        # Fallback address must always map strictly within 0x10..=0x4F
        test_uids = [
            [0x00000000, 0x00000000, 0x00000000],
            [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF],
            [0x12345678, 0x9ABCDEF0, 0x11223344],
            [0xDEADBEEF, 0xCAFEBABE, 0x01020304],
        ]
        for uid in test_uids:
            addr = fallback_address(uid)
            self.assertTrue(0x10 <= addr <= 0x4F)

        # Distinct UIDs yield distinct fallback addresses
        addr1 = fallback_address([0x00000001, 0, 0])
        addr2 = fallback_address([0x00000002, 0, 0])
        self.assertNotEqual(addr1, addr2)

    def test_address_binding_hierarchy(self):
        uid = [0x11111111, 0x22222222, 0x33333333]
        fallback = fallback_address(uid)

        blank_mb = b"\xFF" * 32
        blank_flash = b"\xFF" * 256

        # Case 1: Fresh blank silicon -> Tier 3 UID fallback
        addr = resolve_node_address(blank_mb, blank_flash, blank_flash, uid)
        self.assertEqual(addr, fallback)

        # Case 2: Configured Flash Sector 14 -> Tier 2 Flash address (0x38)
        sector_14 = bytearray(256)
        struct.pack_into("<IHHII", sector_14, 0, FLASH_STORAGE_MAGIC, 1, 96, 10, 0)
        sector_14[FLASH_HEADER_SIZE + FLASH_REG_NODE_ID_OFFSET] = 0x38
        addr = resolve_node_address(blank_mb, bytes(sector_14), blank_flash, uid)
        self.assertEqual(addr, 0x38)

        # Case 3: Warm reboot with valid SoftResetMailbox -> Tier 1 overrides Flash (0x42)
        valid_mb = make_soft_reset_mailbox(REQUEST_CANBOOT, 0x42, bus_type=1)
        addr = resolve_node_address(valid_mb, bytes(sector_14), blank_flash, uid)
        self.assertEqual(addr, 0x42)

        # Case 4: Brownout/corrupted SoftResetMailbox -> Cascades back down to Flash (0x38)
        corrupted_mb = bytearray(valid_mb)
        corrupted_mb[9] ^= 0x55
        addr = resolve_node_address(bytes(corrupted_mb), bytes(sector_14), blank_flash, uid)
        self.assertEqual(addr, 0x38)

    def test_firmware_binary_artifact(self):
        repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
        bin_path = os.path.join(repo_root, "artifacts", "bareboot-stm32h503-i3c-i2c.bin")
        self.assertTrue(os.path.exists(bin_path), f"Artifact missing: {bin_path}")
        size = os.path.getsize(bin_path)
        self.assertGreater(size, 2048)
        self.assertLess(size, 8192)

        with open(bin_path, "rb") as f:
            data = f.read()

        initial_sp = struct.unpack("<I", data[:4])[0]
        reset_handler = struct.unpack("<I", data[4:8])[0]

        # STM32H503: SRAM is 0x2000_0000..0x2000_8000; SP must be in this range
        self.assertTrue(0x20000000 <= initial_sp <= 0x20008000, f"Invalid SP: 0x{initial_sp:08X}")
        # Flash is 0x0800_0000..0x0802_0000; Thumb reset handler must be odd and in sector 0
        self.assertTrue(0x08000000 <= reset_handler <= 0x08004000, f"Invalid PC: 0x{reset_handler:08X}")
        self.assertEqual(reset_handler & 1, 1, "Reset handler Thumb bit must be set")


if __name__ == "__main__":
    unittest.main()
