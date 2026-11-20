#!/usr/bin/env python3
"""Simulation Test Harness for Multi-Drop In-Band Bootloader Updates.

Tests multi-node configuration parsing, pre-flight bus inventory discovery,
selective handsfree 0xF0 bootloader entry, sequential block flashing,
and readback verification across a simulated shared multidrop bus.
"""

import hashlib
import os
import sys
import tempfile
import unittest

# Import the multidrop flashtool implementation
from multidrop_flashtool import (
    BaseBusTransport,
    ENTRY_STROBE_BYTE,
    MultidropConfig,
    MultidropFlasher,
    NodeConfig,
    crc16,
    make_frame,
    parse_frame,
)


class MockDeviceNode:
    """Simulates an individual MCU node on a shared multidrop bus."""

    STATE_APP = 1
    STATE_BOOTLOADER = 2

    def __init__(self, address: int, app_base: int = 0x08002000, block_size: int = 64, mcu: str = "stm32h503xx"):
        self.address = address
        self.app_base = app_base
        self.block_size = block_size
        self.mcu = mcu
        self.state = self.STATE_APP
        self.ram_mailbox = 0
        self.flash_memory = bytearray(32 * 1024)  # 32 KB simulated flash
        self.received_blocks = 0
        self.is_completed = False
        self.online = True
        self.corrupt_readback = False

    def handle_write(self, data: bytes) -> bool:
        if not self.online:
            return False

        # In-band entry strobe: single byte 0xF0
        if data == ENTRY_STROBE_BYTE:
            self.ram_mailbox = 0x5984E3FA6CA1589B
            self.state = self.STATE_BOOTLOADER
            return True

        if self.state == self.STATE_APP:
            # Application ignores Katapult frames unless already in bootloader
            return False

        # Parse Katapult frame
        frame = parse_frame(data)
        if not frame:
            return False

        cmd = frame["cmd"]
        payload = frame["payload"]

        if cmd == 0x11:  # CONNECT
            self.ram_mailbox = 0
            return True

        elif cmd == 0x12:  # RX_BLOCK
            if len(payload) < 4 + self.block_size:
                return False
            addr = int.from_bytes(payload[0:4], "little")
            offset = addr - self.app_base
            block_bytes = payload[4:4 + self.block_size]
            if 0 <= offset + self.block_size <= len(self.flash_memory):
                self.flash_memory[offset:offset + self.block_size] = block_bytes
                self.received_blocks += 1
                return True
            return False

        elif cmd == 0x13:  # RX_EOF
            return True

        elif cmd == 0x14:  # REQ_BLOCK
            return True

        elif cmd == 0x15:  # COMPLETE
            self.is_completed = True
            self.state = self.STATE_APP  # Boot into new app
            return True

        return False

    def handle_read(self, length: int, last_cmd: Optional[int], last_payload: bytes) -> bytes:
        if not self.online:
            return b""

        if self.state != self.STATE_BOOTLOADER and last_cmd != 0x15:
            return b"\xFF" * length

        if last_cmd == 0x11:  # ACK to CONNECT
            mcu_bytes = self.mcu.encode("utf-8") + b"\x00"
            mcu_pad = (4 - (len(mcu_bytes) % 4)) % 4
            mcu_payload = mcu_bytes + b"\x00" * mcu_pad

            version_bytes = b"v0.0.1\x00"
            ver_pad = (4 - (len(version_bytes) % 4)) % 4
            ver_payload = version_bytes + b"\x00" * ver_pad

            payload = (
                (0x11).to_bytes(4, "little") +
                (1).to_bytes(4, "little") +
                self.app_base.to_bytes(4, "little") +
                self.block_size.to_bytes(4, "little") +
                mcu_payload +
                ver_payload
            )
            return make_frame(0xA0, payload)

        elif last_cmd == 0x12:  # ACK to RX_BLOCK
            addr = int.from_bytes(last_payload[0:4], "little")
            payload = (0x12).to_bytes(4, "little") + addr.to_bytes(4, "little")
            return make_frame(0xA0, payload)

        elif last_cmd == 0x13:  # ACK to RX_EOF
            payload = (0x13).to_bytes(4, "little") + (self.received_blocks).to_bytes(4, "little")
            return make_frame(0xA0, payload)

        elif last_cmd == 0x14:  # ACK to REQ_BLOCK
            addr = int.from_bytes(last_payload[0:4], "little")
            offset = addr - self.app_base
            block_data = bytes(self.flash_memory[offset:offset + self.block_size])
            if self.corrupt_readback:
                # Corrupt one byte to simulate flash write error
                block_data = bytes([block_data[0] ^ 0xFF]) + block_data[1:]
            payload = (0x14).to_bytes(4, "little") + addr.to_bytes(4, "little") + block_data
            return make_frame(0xA0, payload)

        elif last_cmd == 0x15:  # ACK to COMPLETE
            payload = (0x15).to_bytes(4, "little")
            return make_frame(0xA0, payload)

        return b"\xFF" * length


class MockBusTransport(BaseBusTransport):
    """Simulates a shared physical multidrop bus with connected MockDeviceNodes."""

    def __init__(self):
        self.nodes: Dict[int, MockDeviceNode] = {}
        self.last_cmd: Dict[int, Optional[int]] = {}
        self.last_payload: Dict[int, bytes] = {}

    def attach_node(self, node: MockDeviceNode):
        self.nodes[node.address] = node
        self.last_cmd[node.address] = None
        self.last_payload[node.address] = b""

    def probe(self, addr: int) -> bool:
        node = self.nodes.get(addr)
        return node is not None and node.online

    def write_raw(self, addr: int, data: bytes) -> bool:
        node = self.nodes.get(addr)
        if not node:
            return False

        if len(data) >= 8 and data[0] == 0x01 and data[1] == 0x88:
            parsed = parse_frame(data)
            if parsed:
                self.last_cmd[addr] = parsed["cmd"]
                self.last_payload[addr] = parsed["payload"]
        else:
            self.last_cmd[addr] = None
            self.last_payload[addr] = b""

        return node.handle_write(data)

    def read_raw(self, addr: int, length: int) -> bytes:
        node = self.nodes.get(addr)
        if not node:
            return b""
        return node.handle_read(length, self.last_cmd.get(addr), self.last_payload.get(addr, b""))


class MultidropFlasherSimulationTest(unittest.TestCase):
    """Unit and simulation test suite for multidrop flashing."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()

        # Create three dummy firmware binaries
        self.fw_yaw = os.path.join(self.temp_dir.name, "yaw_fw.bin")
        self.fw_pitch = os.path.join(self.temp_dir.name, "pitch_fw.bin")
        self.fw_roll = os.path.join(self.temp_dir.name, "roll_fw.bin")

        with open(self.fw_yaw, "wb") as f:
            f.write(bytes([(i * 3 + 1) & 0xFF for i in range(256)]))
        with open(self.fw_pitch, "wb") as f:
            f.write(bytes([(i * 5 + 7) & 0xFF for i in range(256)]))
        with open(self.fw_roll, "wb") as f:
            f.write(bytes([(i * 7 + 11) & 0xFF for i in range(256)]))

        self.node_configs = [
            NodeConfig("joint_yaw", 0x40, self.fw_yaw),
            NodeConfig("joint_pitch", 0x41, self.fw_pitch),
            NodeConfig("joint_roll", 0x42, self.fw_roll),
        ]
        self.config = MultidropConfig(
            bus_type="i2c",
            speed_khz=400,
            reset_delay=0.01,
            connect_retries=5,
            nodes=self.node_configs,
        )

        self.bus = MockBusTransport()
        self.node_yaw = MockDeviceNode(0x40)
        self.node_pitch = MockDeviceNode(0x41)
        self.node_roll = MockDeviceNode(0x42)
        self.bus.attach_node(self.node_yaw)
        self.bus.attach_node(self.node_pitch)
        self.bus.attach_node(self.node_roll)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_crc16_and_framing(self):
        data = b"\x11\x00"
        c = crc16(data)
        self.assertIsInstance(c, int)
        f = make_frame(0x11, b"")
        parsed = parse_frame(f)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["cmd"], 0x11)
        self.assertEqual(parsed["wc"], 0)

    def test_selective_inband_entry(self):
        """Verifies that 0xF0 strobe only transitions the addressed node."""
        self.assertEqual(self.node_yaw.state, MockDeviceNode.STATE_APP)
        self.assertEqual(self.node_pitch.state, MockDeviceNode.STATE_APP)
        self.assertEqual(self.node_roll.state, MockDeviceNode.STATE_APP)

        # Send strobe targeting 0x41 (pitch) only
        ok = self.bus.write_raw(0x41, ENTRY_STROBE_BYTE)
        self.assertTrue(ok)

        # Only node 0x41 should be in bootloader; 0x40 and 0x42 must remain in APP
        self.assertEqual(self.node_yaw.state, MockDeviceNode.STATE_APP)
        self.assertEqual(self.node_pitch.state, MockDeviceNode.STATE_BOOTLOADER)
        self.assertEqual(self.node_roll.state, MockDeviceNode.STATE_APP)
        self.assertEqual(self.node_pitch.ram_mailbox, 0x5984E3FA6CA1589B)

    def test_preflight_inventory_success(self):
        """Pre-flight succeeds when all nodes are responsive."""
        flasher = MultidropFlasher(self.bus, self.config)
        inv = flasher.preflight_inventory(self.config.nodes)
        self.assertEqual(inv, {"joint_yaw": True, "joint_pitch": True, "joint_roll": True})

    def test_preflight_inventory_aborts_on_missing_node(self):
        """Pre-flight fails when a node is absent, preventing partial updates."""
        self.node_roll.online = False
        flasher = MultidropFlasher(self.bus, self.config)
        inv = flasher.preflight_inventory(self.config.nodes)
        self.assertFalse(inv["joint_roll"])
        self.assertTrue(inv["joint_yaw"])

        # Running flasher must abort at step 1
        res = flasher.run()
        self.assertFalse(res)
        # Node yaw must NOT have been touched
        self.assertEqual(self.node_yaw.state, MockDeviceNode.STATE_APP)
        self.assertEqual(self.node_yaw.received_blocks, 0)

    def test_sequential_multi_node_flash_success(self):
        """Verifies full sequential update across all three nodes."""
        flasher = MultidropFlasher(self.bus, self.config)
        res = flasher.run()
        self.assertTrue(res)

        # Verify all nodes received their binary, readback matched, and marked complete
        for node, fw_path in [(self.node_yaw, self.fw_yaw), (self.node_pitch, self.fw_pitch), (self.node_roll, self.fw_roll)]:
            self.assertEqual(node.received_blocks, 4)  # 256 bytes / 64 bytes
            self.assertTrue(node.is_completed)
            self.assertEqual(node.state, MockDeviceNode.STATE_APP)
            with open(fw_path, "rb") as f:
                expected = f.read()
            actual = bytes(node.flash_memory[0:256])
            self.assertEqual(actual, expected)

    def test_readback_corruption_detection(self):
        """Verifies that readback mismatch causes abort without completing update."""
        self.node_pitch.corrupt_readback = True
        flasher = MultidropFlasher(self.bus, self.config)
        res = flasher.run()
        self.assertFalse(res)

        # Node yaw succeeded
        self.assertTrue(self.node_yaw.is_completed)
        # Node pitch failed verification, never completed
        self.assertFalse(self.node_pitch.is_completed)
        # Node roll was never started
        self.assertEqual(self.node_roll.received_blocks, 0)

    def test_selective_single_node_filter(self):
        """Verifies updating a single node using --node filter."""
        flasher = MultidropFlasher(self.bus, self.config)
        res = flasher.run(node_filter="joint_pitch")
        self.assertTrue(res)

        # Only joint_pitch was flashed
        self.assertEqual(self.node_yaw.received_blocks, 0)
        self.assertEqual(self.node_pitch.received_blocks, 4)
        self.assertTrue(self.node_pitch.is_completed)
        self.assertEqual(self.node_roll.received_blocks, 0)


if __name__ == "__main__":
    unittest.main()
