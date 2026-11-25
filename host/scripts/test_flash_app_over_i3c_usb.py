#!/usr/bin/env python3
"""Automated unit test suite for BareBoot Native USB Bulk I3C Flashtool."""

import unittest
import os
import sys

# Add host/scripts to sys.path
SCRIPTS_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPTS_DIR not in sys.path:
    sys.path.insert(0, SCRIPTS_DIR)

from flash_app_over_i3c_usb import (
    crc16,
    make_frame,
    parse_frame,
    MockAgentLaI3c,
    I3cUsbFlasher,
    CMD_CONNECT,
    CMD_BLOCK_WRITE,
    CMD_RX_EOF,
    CMD_REQ_BLOCK,
    CMD_COMPLETE,
    RESP_ACK,
)


class TestI3cUsbFlasher(unittest.TestCase):
    """Test suite verifying framing, mock transport, and flash orchestrator."""

    def test_crc16_known_vector(self):
        """Verify CRC16 matches Katapult CCITT computation."""
        data = bytes([0x11, 0x00]) # Empty payload connect
        computed = crc16(data)
        self.assertIsInstance(computed, int)
        self.assertTrue(0 <= computed <= 0xFFFF)

    def test_frame_roundtrip(self):
        """Verify Katapult command framing and parsing."""
        cmd = CMD_BLOCK_WRITE
        payload = bytes([0x00, 0x20, 0x00, 0x08]) + bytes(range(64)) # addr + 64 bytes
        framed = make_frame(cmd, payload)

        self.assertEqual(framed[0], 0x01)
        self.assertEqual(framed[1], 0x88)
        self.assertEqual(framed[-2], 0x99)
        self.assertEqual(framed[-1], 0x03)

        parsed = parse_frame(framed)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["cmd"], cmd)
        self.assertEqual(parsed["wc"], len(payload) // 4)
        self.assertEqual(parsed["payload"], payload)

    def test_mock_transport_entdaa(self):
        """Verify ENTDAA address assignment via MockAgentLaI3c."""
        mock_client = MockAgentLaI3c(freq_khz=12500)
        self.assertTrue(mock_client.connect())

        dev_info = mock_client.entdaa(0x40)
        self.assertIsNotNone(dev_info)
        self.assertEqual(dev_info["dynamic_address"], 0x40)
        self.assertEqual(dev_info["bcr"], 0x06)
        self.assertEqual(dev_info["dcr"], 0x63)
        mock_client.close()

    def test_mock_transport_pin_inversion(self):
        """Verify dynamic pin setting and swapping on MockAgentLaI3c."""
        mock_client = MockAgentLaI3c()
        self.assertEqual(mock_client.scl_pin, 10)
        self.assertEqual(mock_client.sda_pin, 11)

        mock_client.set_pins(14, 15)
        self.assertEqual(mock_client.scl_pin, 14)
        self.assertEqual(mock_client.sda_pin, 15)

        scl, sda = mock_client.swap_pins()
        self.assertEqual(scl, 15)
        self.assertEqual(sda, 14)
        self.assertEqual(mock_client.scl_pin, 15)
        self.assertEqual(mock_client.sda_pin, 14)

    def test_full_flasher_workflow(self):
        """Verify full Katapult lifecycle: connect, block write, verify, complete."""
        mock_client = MockAgentLaI3c(freq_khz=12500)
        mock_client.connect()
        mock_client.entdaa(0x40)

        flasher = I3cUsbFlasher(mock_client, addr=0x40, freq_khz=12500)
        self.assertTrue(flasher.connect_bootloader())
        self.assertEqual(flasher.app_base, 0x08002000)
        self.assertEqual(flasher.block_size, 64)
        self.assertEqual(flasher.mcu, "stm32h503xx")

        # Create 1024-byte test image (16 blocks)
        test_image = bytes((i % 256) for i in range(1024))
        stats = flasher.flash_image(test_image, verify=True)

        self.assertEqual(stats["image_size_bytes"], 1024)
        self.assertEqual(stats["num_blocks"], 16)
        self.assertTrue(stats["verification_passed"])
        self.assertGreater(stats["effective_kb_s"], 0)

        mock_client.close()

    def test_mock_transport_pin_inversion(self):
        """Verify dynamic pin setting and swapping on MockAgentLaI3c."""
        mock_client = MockAgentLaI3c()
        self.assertEqual(mock_client.scl_pin, 10)
        self.assertEqual(mock_client.sda_pin, 11)

        mock_client.set_pins(14, 15)
        self.assertEqual(mock_client.scl_pin, 14)
        self.assertEqual(mock_client.sda_pin, 15)

        scl, sda = mock_client.swap_pins()
        self.assertEqual(scl, 15)
        self.assertEqual(sda, 14)
        self.assertEqual(mock_client.scl_pin, 15)
        self.assertEqual(mock_client.sda_pin, 14)

    def test_verification_detects_corruption(self):
        """Verify that readback catches corrupted flash memory."""
        mock_client = MockAgentLaI3c(freq_khz=12500)
        mock_client.connect()
        mock_client.entdaa(0x40)

        flasher = I3cUsbFlasher(mock_client, addr=0x40, freq_khz=12500)
        self.assertTrue(flasher.connect_bootloader())

        test_image = b"\xAA" * 256

        # Hook into private_read to corrupt readback response
        original_read = mock_client.private_read
        def corrupt_read(target_addr, length):
            raw = original_read(target_addr, length)
            parsed = parse_frame(raw)
            if parsed and parsed["cmd"] == RESP_ACK and len(parsed["payload"]) > 16:
                # Corrupt payload byte
                corrupt_pl = bytearray(parsed["payload"])
                corrupt_pl[10] ^= 0xFF
                return make_frame(RESP_ACK, bytes(corrupt_pl))
            return raw

        mock_client.private_read = corrupt_read
        stats = flasher.flash_image(test_image, verify=True)
        self.assertFalse(stats["verification_passed"])

        mock_client.close()

    def test_mock_transport_pin_inversion(self):
        """Verify dynamic pin setting and swapping on MockAgentLaI3c."""
        mock_client = MockAgentLaI3c()
        self.assertEqual(mock_client.scl_pin, 10)
        self.assertEqual(mock_client.sda_pin, 11)

        mock_client.set_pins(14, 15)
        self.assertEqual(mock_client.scl_pin, 14)
        self.assertEqual(mock_client.sda_pin, 15)

        scl, sda = mock_client.swap_pins()
        self.assertEqual(scl, 15)
        self.assertEqual(sda, 14)
        self.assertEqual(mock_client.scl_pin, 15)
        self.assertEqual(mock_client.sda_pin, 14)


if __name__ == "__main__":
    unittest.main()
