#!/usr/bin/env python3
"""Automated Production Lockout and Recovery Verification Suite for BareBoot.

Verifies all production lockout, fail-safe, and recovery scenarios strictly over
the 2-wire serial bus (I2C/I3C on PB3/PB4) without SWD access.

Scenarios Verified:
1. Scenario 1: Normal Handsfree OTA over I3C/I2C
   - Target running application -> Trigger handsfree entry (0xF0) -> Katapult connect
     -> Block flash -> Bit-exact readback verification -> Complete and launch application.
2. Scenario 2: Application Without BareBoot Re-Entry Logic (Boot Window Catch)
   - Target running uncooperative firmware lacking 0xF0 / RSTACT re-entry hook.
   - Hardware power cycle / reset opens BareBoot 5.0-second boot window.
   - Flashtool connects within window, cancels timeout (bootentry_cancel_timeout),
     and flashes target without prior application cooperation.
3. Scenario 3: Interrupted / Torn / Corrupted OTA Recovery
   - Mid-stream abort leaving erased / corrupted vector table.
   - Target reboots: !application_check_valid() causes launch timer to be disabled
     and traps permanently in bootloader recovery mode.
   - Serial bus flashtool connects to trapped recovery target, re-flashes bit-exact
     firmware, restoring operation with zero SWD intervention.
4. Scenario 4: NVM Calibration Retention
   - Verifies Sectors 14 and 15 (0x0801_C000..0x0802_0000, H5FlashStorage) remain
     bit-exact with identical CRC32 and SHA-256 digests across all flash operations.
"""

import hashlib
import os
import struct
import sys
import time
import unittest
import zlib
from typing import Any, Dict, List, Optional, Tuple

# Katapult / BareBoot Protocol Command Codes
CMD_CONNECT     = 0x11
CMD_BLOCK_WRITE = 0x12
CMD_RX_EOF      = 0x13
CMD_REQ_BLOCK   = 0x14
CMD_COMPLETE    = 0x15
RESP_ACK        = 0xA0
RESP_NACK       = 0xA1

# STM32H503 Memory Map Constants
FLASH_BASE          = 0x08000000
BOOTLOADER_BASE     = 0x08000000
BOOTLOADER_SIZE     = 0x00002000  # 8 KiB (Sector 0)
APP_BASE            = 0x08002000  # Sector 1 start
APP_MAX_SIZE        = 0x0001A000  # 104 KiB (Sectors 1..13)
APP_LIMIT           = APP_BASE + APP_MAX_SIZE  # 0x0801C000
NVM_CAL_BASE        = 0x0801C000  # Sector 14 start (H5FlashStorage Bank 2)
NVM_CAL_SIZE        = 0x00004000  # 16 KiB (Sectors 14 and 15)
FLASH_TOTAL_SIZE    = 0x00020000  # 128 KiB
SECTOR_SIZE         = 8192        # 8 KiB per sector on STM32H503
BLOCK_SIZE          = 64          # Katapult transfer block size

RAM_START           = 0x20000000
RAM_SIZE            = 0x00008000  # 32 KiB
RAM_LIMIT           = RAM_START + RAM_SIZE
MAILBOX_ADDR        = RAM_LIMIT - 8  # 0x20007FF8

REQUEST_BOOTLOADER  = 0x5984E3FA6CA1589B
INBAND_ENTRY_BYTE   = bytes([0xF0])
BOOT_WINDOW_SECONDS = 5.0


def crc16(data: bytes) -> int:
    """CRC16-CCITT implementation compatible with Katapult bootloader."""
    crc = 0xFFFF
    for byte in data:
        tmp = byte ^ (crc & 0xFF)
        tmp = (tmp ^ ((tmp << 4) & 0xFF)) & 0xFF
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp >> 4) ^ (tmp << 3)) & 0xFFFF
    return crc


def make_frame(cmd: int, payload: bytes = b"") -> bytes:
    """Frame Katapult command: [0x01, 0x88, cmd, word_count, payload..., crc_lo, crc_hi, 0x99, 0x03]."""
    assert len(payload) % 4 == 0, "Payload length must be a multiple of 4 bytes"
    wc = len(payload) // 4
    core = bytes([cmd, wc]) + payload
    c = crc16(core)
    return bytes([0x01, 0x88]) + core + bytes([c & 0xFF, (c >> 8) & 0xFF, 0x99, 0x03])


def parse_frame(data: bytes) -> Optional[Dict[str, Any]]:
    """Scan and parse a Katapult protocol frame from raw received bytes."""
    for i in range(len(data) - 1):
        if data[i] == 0x01 and data[i + 1] == 0x88 and len(data) - i >= 8:
            cmd = data[i + 2]
            wc = data[i + 3]
            frame_len = 4 + wc * 4 + 4
            if len(data) - i < frame_len:
                continue
            cand = data[i:i + frame_len]
            if cand[-2] != 0x99 or cand[-1] != 0x03:
                continue
            exp_crc = cand[-4] | (cand[-3] << 8)
            if crc16(cand[2:4 + wc * 4]) != exp_crc:
                continue
            return {
                "cmd": cand[2],
                "wc": wc,
                "payload": cand[4:4 + wc * 4],
            }
    return None


class SimulatedFlashMemory:
    """Simulates STM32H503 NOR flash with sector erase and quad-word programming."""

    def __init__(self, size: int = FLASH_TOTAL_SIZE):
        self.size = size
        self.memory = bytearray([0xFF] * size)
        self.erased_sectors = set(range(size // SECTOR_SIZE))

    def erase_sector(self, sector_index: int):
        offset = sector_index * SECTOR_SIZE
        self.memory[offset:offset + SECTOR_SIZE] = b"\xFF" * SECTOR_SIZE
        self.erased_sectors.add(sector_index)

    def write_block(self, address: int, data: bytes) -> bool:
        if address < FLASH_BASE or address + len(data) > FLASH_BASE + self.size:
            return False
        offset = address - FLASH_BASE
        sector_idx = offset // SECTOR_SIZE
        # Sector must have been erased or cells must support programming
        for i, b in enumerate(data):
            current = self.memory[offset + i]
            # In NOR flash, programming can only change 1s to 0s
            if (current & b) != b:
                return False
            self.memory[offset + i] = current & b
        return True

    def read_range(self, address: int, length: int) -> bytes:
        if address < FLASH_BASE or address + length > FLASH_BASE + self.size:
            return b""
        offset = address - FLASH_BASE
        return bytes(self.memory[offset:offset + length])


class SimulatedFishyServoNode:
    """High-fidelity STM32H503 target node running BareBoot and FishyServo application.

    Models:
    - Sector 0 (0x0800_0000..0x0800_2000): BareBoot
    - Sector 1..13 (0x0800_2000..0x0801_C000): FishyServo Application slot
    - Sector 14..15 (0x0801_C000..0x0802_0000): NVM Calibration (H5FlashStorage)
    - RAM mailbox at 0x2000_7FF8 for handsfree re-entry token
    - BareBoot bootentry_check logic with 5.0-second boot window
    - application_check_valid SP and PC Thumb-mode validation
    - Cancellation of boot window timeout upon Katapult commands
    - Permanent trapping in recovery mode on corrupt/invalid application
    """

    STATE_APP_RUNNING = "APP_RUNNING"
    STATE_BOOT_WINDOW = "BOOT_WINDOW"
    STATE_BOOT_TRAPPED = "BOOT_TRAPPED"

    def __init__(self, i2c_addr: int = 0x40, supports_reentry_hook: bool = True):
        self.i2c_addr = i2c_addr
        self.supports_reentry_hook = supports_reentry_hook
        self.flash = SimulatedFlashMemory()
        self.ram_mailbox: int = 0
        self.state: str = self.STATE_APP_RUNNING
        self.boot_window_expires: float = 0.0
        self.last_parsed_cmd: Optional[Dict[str, Any]] = None
        self.mcu = "stm32h503xx"
        self.received_blocks: int = 0
        self.pending_jump: bool = False
        self.commit_state: str = "VALID"
        self.erased_in_transfer: set = set()

        # Mark bootloader sector as programmed
        self.flash.write_block(BOOTLOADER_BASE, b"\x00\x80\x00\x20\x09\x00\x00\x08" + b"\x55" * 56)

    def application_check_valid(self) -> bool:
        """Evaluate application vector table validity at APP_BASE (0x0800_2000)."""
        vec_bytes = self.flash.read_range(APP_BASE, 16)
        if len(vec_bytes) < 16:
            return False
        sp, pc, nmi, hardfault = struct.unpack("<IIII", vec_bytes)

        # 1. Stack Pointer (SP) must be within valid SRAM boundaries
        if sp <= RAM_START or sp > RAM_LIMIT:
            return False

        # 2. Reset Vector (PC) must have Thumb-bit set (bit 0 == 1)
        if (pc & 1) == 0:
            return False

        # 3. PC must lie within flash application region
        if pc < APP_BASE or pc >= (FLASH_BASE + FLASH_TOTAL_SIZE - 4):
            return False

        # 4. Multi-Vector Verification: HardFault_Handler (app[3])
        if hardfault == 0xFFFFFFFF or hardfault == 0 or (hardfault & 1) == 0:
            return False
        if hardfault < APP_BASE or hardfault >= (FLASH_BASE + FLASH_TOTAL_SIZE):
            return False

        # 5. Entry Opcode Sanity: Dereference first instruction opcode at pc & ~1
        entry_addr = pc & ~1
        entry_bytes = self.flash.read_range(entry_addr, 4)
        if len(entry_bytes) < 4:
            return False
        entry_code = struct.unpack("<I", entry_bytes)[0]
        if entry_code == 0xFFFFFFFF or entry_code == 0:
            return False

        # 6. Monotonic commit state verification
        if self.commit_state != "VALID":
            return False

        return True

    def bootentry_check(self, current_time: float):
        """Execute BareBoot bootentry_check logic upon power cycle or reset."""
        bootup_code = self.ram_mailbox
        is_valid = self.application_check_valid()

        if bootup_code == REQUEST_BOOTLOADER or not is_valid:
            # Trapped indefinitely in bootloader recovery loop
            self.ram_mailbox = 0
            self.state = self.STATE_BOOT_TRAPPED
            self.boot_window_expires = 0.0
        else:
            # Valid application present with no bootloader request token:
            # Open 5.0-second boot window
            self.state = self.STATE_BOOT_WINDOW
            self.boot_window_expires = current_time + BOOT_WINDOW_SECONDS

    def bootentry_cancel_timeout(self):
        """Cancel the 5.0-second boot window timeout upon receiving Katapult commands."""
        if self.state == self.STATE_BOOT_WINDOW:
            self.state = self.STATE_BOOT_TRAPPED
            self.boot_window_expires = 0.0

    def update_time(self, current_time: float):
        """Advance time; handles boot window timeout transition into application."""
        if self.state == self.STATE_BOOT_WINDOW:
            if current_time >= self.boot_window_expires:
                self.state = self.STATE_APP_RUNNING
                self.boot_window_expires = 0.0

    def power_cycle(self, current_time: float):
        """Simulate physical power cycle / POR: RAM is cleared, BareBoot boots."""
        self.ram_mailbox = 0
        self.received_blocks = 0
        self.erased_in_transfer.clear()
        self.bootentry_check(current_time)

    def reset_pulse(self, current_time: float):
        """Simulate reset line pulse / NVIC_SystemReset: RAM mailbox preserved."""
        self.received_blocks = 0
        self.erased_in_transfer.clear()
        self.bootentry_check(current_time)

    def is_in_bootloader(self) -> bool:
        return self.state in (self.STATE_BOOT_WINDOW, self.STATE_BOOT_TRAPPED)

    def handle_bus_write(self, addr: int, data: bytes, current_time: float) -> bool:
        if addr != self.i2c_addr:
            return False

        # In-band reboot strobe (single byte 0xF0)
        if data == INBAND_ENTRY_BYTE:
            if self.state == self.STATE_APP_RUNNING:
                if self.supports_reentry_hook:
                    # Application acknowledges 0xF0, sets token, and triggers reset
                    self.ram_mailbox = REQUEST_BOOTLOADER
                    self.reset_pulse(current_time)
                    return True
                else:
                    # Uncooperative application ignores 0xF0
                    return False
            else:
                # Already in bootloader
                return True

        if not self.is_in_bootloader():
            # Running application ignores Katapult framing
            return False

        # Any command received in bootloader cancels the launch timeout
        self.bootentry_cancel_timeout()

        frame = parse_frame(data)
        if not frame:
            return False

        self.last_parsed_cmd = frame
        cmd = frame["cmd"]
        payload = frame["payload"]

        if cmd == CMD_CONNECT:
            return True

        elif cmd == CMD_BLOCK_WRITE:
            if len(payload) < 4 + BLOCK_SIZE:
                return False
            dest_addr = struct.unpack("<I", payload[0:4])[0]
            block_data = payload[4:4 + BLOCK_SIZE]

            # Enforce flash boundary protection:
            # Writes must be within application slot: APP_BASE <= dest_addr < APP_LIMIT
            if dest_addr < APP_BASE or (dest_addr + BLOCK_SIZE) > APP_LIMIT:
                return False

            if self.received_blocks == 0:
                self.commit_state = "UPDATE_IN_PROGRESS"

            offset = dest_addr - FLASH_BASE
            sector_idx = offset // SECTOR_SIZE
            if sector_idx not in self.erased_in_transfer:
                self.flash.erase_sector(sector_idx)
                self.erased_in_transfer.add(sector_idx)

            ok = self.flash.write_block(dest_addr, block_data)
            if ok:
                self.received_blocks += 1
            return ok

        elif cmd == CMD_RX_EOF:
            return True

        elif cmd == CMD_REQ_BLOCK:
            return True

        elif cmd == CMD_COMPLETE:
            self.commit_state = "VALID"
            self.erased_in_transfer.clear()
            self.pending_jump = True
            return True

        return False

    def handle_bus_read(self, addr: int, length: int) -> bytes:
        if addr != self.i2c_addr:
            return b"\xFF" * length

        if not self.is_in_bootloader() and not self.pending_jump:
            return b"\xFF" * length

        if not self.last_parsed_cmd:
            return b"\xFF" * length

        cmd = self.last_parsed_cmd["cmd"]
        payload = self.last_parsed_cmd["payload"]

        if cmd == CMD_CONNECT:
            mcu_bytes = self.mcu.encode("utf-8") + b"\x00"
            mcu_pad = (4 - (len(mcu_bytes) % 4)) % 4
            mcu_pl = mcu_bytes + b"\x00" * mcu_pad

            resp_pl = (
                struct.pack("<I", 0x11)
                + struct.pack("<I", 1)  # Katapult protocol version
                + struct.pack("<I", APP_BASE)
                + struct.pack("<I", BLOCK_SIZE)
                + mcu_pl
            )
            return make_frame(RESP_ACK, resp_pl)

        elif cmd == CMD_BLOCK_WRITE:
            dest_addr = struct.unpack("<I", payload[0:4])[0]
            resp_pl = struct.pack("<I", dest_addr)
            return make_frame(RESP_ACK, resp_pl)

        elif cmd == CMD_RX_EOF:
            resp_pl = struct.pack("<I", self.received_blocks)
            return make_frame(RESP_ACK, resp_pl)

        elif cmd == CMD_REQ_BLOCK:
            req_addr = struct.unpack("<I", payload[0:4])[0]
            block_data = self.flash.read_range(req_addr, BLOCK_SIZE)
            resp_pl = struct.pack("<I", req_addr) + block_data
            return make_frame(RESP_ACK, resp_pl)

        elif cmd == CMD_COMPLETE:
            if self.pending_jump:
                self.pending_jump = False
                if self.application_check_valid():
                    self.state = self.STATE_APP_RUNNING
                else:
                    self.state = self.STATE_BOOT_TRAPPED
            return make_frame(RESP_ACK)

        return make_frame(RESP_NACK)


class SerialBusController:
    """Simulates 2-wire serial bus (I2C/I3C on PB3/PB4) master."""

    def __init__(self, node: SimulatedFishyServoNode):
        self.node = node
        self.current_time = 100.0

    def advance_time(self, seconds: float):
        self.current_time += seconds
        self.node.update_time(self.current_time)

    def write_bus(self, addr: int, data: bytes) -> bool:
        return self.node.handle_bus_write(addr, data, self.current_time)

    def read_bus(self, addr: int, length: int) -> bytes:
        return self.node.handle_bus_read(addr, length)

    def exchange(self, addr: int, cmd: int, payload: bytes = b"", read_len: int = 64) -> Tuple[Optional[Dict[str, Any]], bytes]:
        tx_data = make_frame(cmd, payload)
        if not self.write_bus(addr, tx_data):
            return None, b""
        rx_data = self.read_bus(addr, read_len)
        parsed = parse_frame(rx_data)
        return parsed, rx_data


class ProductionBusFlasher:
    """Flashtool driver executing over the 2-wire serial bus (I2C/I3C)."""

    def __init__(self, bus: SerialBusController, target_addr: int = 0x40):
        self.bus = bus
        self.addr = target_addr
        self.app_base = APP_BASE
        self.block_size = BLOCK_SIZE

    def trigger_handsfree_reboot(self) -> bool:
        """Send in-band 0xF0 reboot strobe to transition running app to BareBoot."""
        return self.bus.write_bus(self.addr, INBAND_ENTRY_BYTE)

    def connect(self) -> bool:
        """Handshake with BareBoot via CMD_CONNECT, canceling any active boot window."""
        parsed, _ = self.bus.exchange(self.addr, CMD_CONNECT, read_len=64)
        if parsed and parsed["cmd"] == RESP_ACK:
            pl = parsed["payload"]
            self.app_base = struct.unpack("<I", pl[8:12])[0]
            self.block_size = struct.unpack("<I", pl[12:16])[0]
            return True
        return False

    def flash_image(self, image_bytes: bytes, verify: bool = True, abort_after_blocks: Optional[int] = None) -> bool:
        """Stream blocks with inverted ordering (block 0 last), verify readback, and complete transfer."""
        num_blocks = (len(image_bytes) + self.block_size - 1) // self.block_size
        padded_len = num_blocks * self.block_size
        padded_data = image_bytes + b"\xFF" * (padded_len - len(image_bytes))

        # Inverted Flashtool Block Ordering:
        # Write payload blocks (1..num_blocks-1) first. Block 0 (the vector table)
        # is programmed strictly after all payload blocks have been transmitted and verified.
        if num_blocks <= 1:
            payload_indices = []
            block_0_needed = True
        else:
            payload_indices = list(range(1, num_blocks))
            block_0_needed = True

        blocks_transmitted = 0

        # 1. Stream payload blocks (1..num_blocks-1)
        for i in payload_indices:
            if abort_after_blocks is not None and blocks_transmitted >= abort_after_blocks:
                return False

            target_addr = self.app_base + i * self.block_size
            blk_data = padded_data[i * self.block_size:(i + 1) * self.block_size]
            payload = struct.pack("<I", target_addr) + blk_data

            parsed, _ = self.bus.exchange(self.addr, CMD_BLOCK_WRITE, payload, read_len=32)
            if not parsed or parsed["cmd"] != RESP_ACK:
                return False
            blocks_transmitted += 1

        # 2. Readback verification of payload blocks
        if verify and payload_indices:
            for bi in payload_indices:
                target_addr = self.app_base + bi * self.block_size
                payload = struct.pack("<I", target_addr)
                parsed_rb, _ = self.bus.exchange(self.addr, CMD_REQ_BLOCK, payload, read_len=96)
                if not parsed_rb or parsed_rb["cmd"] != RESP_ACK:
                    return False
                got = parsed_rb["payload"][4:4 + self.block_size]
                exp = padded_data[bi * self.block_size:(bi + 1) * self.block_size]
                if got != exp:
                    return False

        # 3. Write Block 0 (the vector table) last, after all payload blocks have been transmitted and verified!
        if block_0_needed:
            if abort_after_blocks is not None and blocks_transmitted >= abort_after_blocks:
                return False

            target_addr = self.app_base
            blk_data = padded_data[:self.block_size]
            payload = struct.pack("<I", target_addr) + blk_data

            parsed, _ = self.bus.exchange(self.addr, CMD_BLOCK_WRITE, payload, read_len=32)
            if not parsed or parsed["cmd"] != RESP_ACK:
                return False
            blocks_transmitted += 1

            if verify:
                payload = struct.pack("<I", target_addr)
                parsed_rb, _ = self.bus.exchange(self.addr, CMD_REQ_BLOCK, payload, read_len=96)
                if not parsed_rb or parsed_rb["cmd"] != RESP_ACK:
                    return False
                got = parsed_rb["payload"][4:4 + self.block_size]
                if got != blk_data:
                    return False

        # End-of-file flush
        parsed_eof, _ = self.bus.exchange(self.addr, CMD_RX_EOF, read_len=32)
        if not parsed_eof or parsed_eof["cmd"] != RESP_ACK:
            return False

        # Complete and trigger launch
        parsed_comp, _ = self.bus.exchange(self.addr, CMD_COMPLETE, read_len=32)
        if not parsed_comp or parsed_comp["cmd"] != RESP_ACK:
            return False

        return True


def make_valid_firmware_image(size: int = 1024) -> bytes:
    """Generate a valid STM32H503 binary with valid SP, Thumb PC reset vector, HardFault vector, and entry opcodes."""
    valid_sp = 0x20004000  # Within 0x2000_0000..0x2000_8000
    valid_pc = 0x08002041  # Thumb mode (bit 0 == 1) within app slot
    valid_nmi = 0x08002051
    valid_hardfault = 0x08002061  # Valid HardFault vector
    vectors = [valid_sp, valid_pc, valid_nmi, valid_hardfault] + [0x08002071] * 12
    vec_bytes = struct.pack("<16I", *vectors)
    entry_opcode = struct.pack("<I", 0xB580B082)  # sub sp, #8; push {r7, lr}
    rem_size = size - 64 - 4
    payload = bytes(((i * 7 + 13) & 0xFF) for i in range(rem_size))
    return vec_bytes + entry_opcode + payload


def make_torn_entry_opcode_firmware_image(size: int = 1024) -> bytes:
    """Generate an image with valid SP, PC, and vectors, but erased entry code (0xFFFFFFFF)."""
    valid_sp = 0x20004000
    valid_pc = 0x08002041
    valid_nmi = 0x08002051
    valid_hardfault = 0x08002061
    vectors = [valid_sp, valid_pc, valid_nmi, valid_hardfault] + [0x08002071] * 12
    vec_bytes = struct.pack("<16I", *vectors)
    erased_opcode = b"\xFF\xFF\xFF\xFF"
    rem_size = size - 64 - 4
    payload = b"\xFF" * rem_size
    return vec_bytes + erased_opcode + payload


def generate_nvm_calibration_payload() -> bytes:
    """Generate mock H5FlashStorage calibration data for Sectors 14 and 15."""
    # 16 KiB total
    records = []
    # Calibration Header: Magic 'CALB', Version 1, Node ID 0x40, Zero Offset 1842
    cal_hdr = struct.pack("<4sHHHHI", b"CALB", 1, 0x40, 1842, 3600, 0x12345678)
    records.append(cal_hdr)
    # Synthetic sensor LUT
    for i in range(256):
        records.append(struct.pack("<H", (i * 256 + 17) & 0xFFFF))
    data = b"".join(records)
    crc = zlib.crc32(data)
    data += struct.pack("<I", crc)
    padded = data + b"\xFF" * (NVM_CAL_SIZE - len(data))
    return padded


class TestProductionLockoutAndRecovery(unittest.TestCase):
    """Test suite verifying all lockout, fail-safe, and recovery scenarios strictly over bus."""

    def setUp(self):
        self.node = SimulatedFishyServoNode(i2c_addr=0x40, supports_reentry_hook=True)
        self.bus = SerialBusController(self.node)
        self.flasher = ProductionBusFlasher(self.bus, target_addr=0x40)

        # Pre-populate Sectors 14 and 15 with calibrated NVM data
        self.nvm_data = generate_nvm_calibration_payload()
        self.node.flash.write_block(NVM_CAL_BASE, self.nvm_data[:BLOCK_SIZE])
        for offset in range(0, NVM_CAL_SIZE, BLOCK_SIZE):
            blk = self.nvm_data[offset:offset + BLOCK_SIZE]
            self.node.flash.write_block(NVM_CAL_BASE + offset, blk)

        self.initial_nvm_crc = zlib.crc32(self.node.flash.read_range(NVM_CAL_BASE, NVM_CAL_SIZE))
        self.initial_nvm_sha256 = hashlib.sha256(self.node.flash.read_range(NVM_CAL_BASE, NVM_CAL_SIZE)).hexdigest()

        # Install initial valid running firmware into application slot
        init_fw = make_valid_firmware_image(size=2048)
        self.node.state = SimulatedFishyServoNode.STATE_BOOT_TRAPPED
        self.flasher.flash_image(init_fw, verify=True)
        self.node.state = SimulatedFishyServoNode.STATE_APP_RUNNING

    def test_scenario_1_normal_handsfree_ota(self):
        """Scenario 1: Normal Handsfree OTA over I3C/I2C.
        Target running app -> 0xF0 strobe -> Katapult connect -> Flash -> Verify -> Launch.
        """
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING)

        # 1. Trigger handsfree entry via 0xF0 strobe over bus
        strobe_ok = self.flasher.trigger_handsfree_reboot()
        self.assertTrue(strobe_ok, "In-band 0xF0 reboot strobe must be accepted")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Node must enter bootloader trapped mode upon receiving 0xF0 token")

        # 2. Katapult connect over bus
        connected = self.flasher.connect()
        self.assertTrue(connected, "Flashtool must successfully connect to BareBoot over serial bus")
        self.assertEqual(self.flasher.app_base, APP_BASE)
        self.assertEqual(self.flasher.block_size, BLOCK_SIZE)

        # 3. Flash updated firmware binary and verify bit-exact readback
        new_fw = make_valid_firmware_image(size=4096)
        flash_ok = self.flasher.flash_image(new_fw, verify=True)
        self.assertTrue(flash_ok, "Flashtool must complete block flashing and bit-exact readback")

        # 4. Verify target rebooted into valid running application
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "Target must launch new application upon CMD_COMPLETE")
        self.assertTrue(self.node.application_check_valid(), "Application vector table must be valid")

    def test_scenario_2_uncooperative_app_boot_window_catch(self):
        """Scenario 2: Application Without BareBoot Re-Entry Logic (Boot Window Catch).
        Uncooperative app ignores 0xF0. Hardware reset opens 5.0-second boot window.
        Flashtool connects within window, cancels timeout, and flashes target without app cooperation.
        """
        # Configure target with uncooperative firmware lacking 0xF0 re-entry hook
        self.node.supports_reentry_hook = False
        self.node.state = SimulatedFishyServoNode.STATE_APP_RUNNING

        # 1. In-band 0xF0 strobe is ignored by uncooperative application
        strobe_ok = self.flasher.trigger_handsfree_reboot()
        self.assertFalse(strobe_ok, "Uncooperative firmware must ignore 0xF0 strobe")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING)

        # 2. Hardware power cycle / reset occurs (e.g. power cycle or external reset pulse)
        self.node.power_cycle(self.bus.current_time)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_WINDOW,
                         "BareBoot must open 5.0-second boot window upon power cycle with valid app")
        self.assertEqual(self.node.boot_window_expires, self.bus.current_time + BOOT_WINDOW_SECONDS)

        # 3. Advance time by 2.0 seconds (within the 5.0-second window)
        self.bus.advance_time(2.0)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_WINDOW,
                         "Target must remain in boot window before 5.0 seconds elapse")

        # 4. Flashtool connects over bus within the window
        connected = self.flasher.connect()
        self.assertTrue(connected, "Flashtool must connect within boot window")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Connecting must cancel boot window timeout (bootentry_cancel_timeout)")
        self.assertEqual(self.node.boot_window_expires, 0.0, "Timeout must be deactivated")

        # 5. Flash new firmware and verify application launch
        restored_fw = make_valid_firmware_image(size=3072)
        flash_ok = self.flasher.flash_image(restored_fw, verify=True)
        self.assertTrue(flash_ok, "Flashing must succeed over serial bus")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "Target must boot restored application")
        self.node.supports_reentry_hook = True

    def test_scenario_2_boot_window_timeout_without_flashtool(self):
        """Scenario 2 auxiliary: Verify that without flashtool, BareBoot boots app after 5.0s."""
        self.node.power_cycle(self.bus.current_time)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_WINDOW)

        # Advance beyond 5.0 seconds
        self.bus.advance_time(5.1)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "BareBoot must automatically launch application after 5.0s without connection")

    def test_scenario_3_interrupted_torn_ota_recovery(self):
        """Scenario 3: Interrupted / Torn / Corrupted OTA Recovery.
        Intentionally abort mid-stream -> corrupted vector table -> !application_check_valid()
        -> Trapped permanently in bootloader recovery -> Bus flashtool connects and restores.
        """
        self.node.supports_reentry_hook = True
        # Enter bootloader
        strobe_ok = self.flasher.trigger_handsfree_reboot()
        self.assertTrue(strobe_ok, "Reboot strobe must succeed")
        connected = self.flasher.connect()
        self.assertTrue(connected, "Must connect to bootloader")

        # Intentionally abort flash operation after writing only 2 blocks (torn transfer)
        partial_fw = bytes([0x00] * 128)  # Zeroed out vector table
        self.flasher.flash_image(partial_fw, abort_after_blocks=2)

        # Simulate hardware reboot after torn transfer
        self.node.power_cycle(self.bus.current_time)

        # Verify application_check_valid() evaluates to FALSE
        self.assertFalse(self.node.application_check_valid(),
                         "Torn / zeroed vector table must fail application_check_valid()")

        # Verify BareBoot disables launch timer and traps permanently in bootloader recovery mode
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "BareBoot must trap indefinitely in recovery mode when application is invalid")
        self.assertEqual(self.node.boot_window_expires, 0.0,
                         "Application launch timer must be disabled")

        # Advance time significantly (e.g. 60 seconds); target must remain trapped and NEVER launch app
        self.bus.advance_time(60.0)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Target must not jump to application even after arbitrary time in recovery mode")

        # Flashtool connects over serial bus from trapped recovery state with ZERO SWD intervention
        connected = self.flasher.connect()
        self.assertTrue(connected, "Flashtool must connect to trapped recovery node over serial bus")

        # Re-flash a complete valid image
        valid_fw = make_valid_firmware_image(size=4096)
        recovered = self.flasher.flash_image(valid_fw, verify=True)
        self.assertTrue(recovered, "Full image re-flash must succeed from recovery state")

        # Verify target is now fully restored and operating
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "Target must resume normal running state after successful re-flash")
        self.assertTrue(self.node.application_check_valid(),
                         "Restored application must pass vector table validation")

    def test_scenario_torn_entry_opcode_trapped(self):
        """Scenario 3b: Valid SP/PC with Erased Entry Opcode Lockout Prevention.
        Verify that an image with valid SP, PC, and exception vectors but erased entry code (0xFFFFFFFF)
        fails application_check_valid() and is permanently trapped in bootloader recovery mode.
        """
        self.flasher.trigger_handsfree_reboot()
        self.assertTrue(self.flasher.connect(), "Must connect to BareBoot")

        # Flash image with valid SP/PC but unprogrammed entry opcode
        torn_entry_fw = make_torn_entry_opcode_firmware_image(size=2048)
        self.flasher.flash_image(torn_entry_fw, verify=True)

        # Simulate reset after writing image
        self.node.power_cycle(self.bus.current_time)

        # application_check_valid must detect erased opcode and reject jump
        self.assertFalse(self.node.application_check_valid(),
                         "Erased entry opcode must fail application_check_valid")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Target must trap indefinitely in recovery mode when entry opcode is blank")
        self.assertEqual(self.node.boot_window_expires, 0.0,
                         "Application launch timer must remain disabled")

        # Advance time by 60s; target must never branch to blank memory
        self.bus.advance_time(60.0)
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Target must remain trapped in recovery mode across time")

        # Flashtool connects and re-flashes valid firmware to restore system
        self.assertTrue(self.flasher.connect(), "Must connect to trapped recovery target")
        valid_fw = make_valid_firmware_image(size=2048)
        self.assertTrue(self.flasher.flash_image(valid_fw, verify=True))
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "Target must resume normal running state after re-flash")
        self.assertTrue(self.node.application_check_valid())

    def test_scenario_inverted_block_ordering_aborted_after_block_1(self):
        """Scenario 3c: Inverted Flashtool Block Ordering Abort Prevention.
        Verify that aborting after Block 1 leaves Block 0 (vector table) unprogrammed (0xFFFFFFFF),
        preventing any jump on subsequent reset.
        """
        self.flasher.trigger_handsfree_reboot()
        self.assertTrue(self.flasher.connect(), "Must connect to BareBoot")

        valid_fw = make_valid_firmware_image(size=4096)
        # Abort transfer strictly after 1 payload block has been transmitted
        aborted = self.flasher.flash_image(valid_fw, abort_after_blocks=1)
        self.assertFalse(aborted, "Transfer must abort after 1 block")

        # Block 0 (the vector table at APP_BASE) must remain completely unprogrammed (0xFF)
        block_0_data = self.node.flash.read_range(APP_BASE, BLOCK_SIZE)
        self.assertEqual(block_0_data, b"\xFF" * BLOCK_SIZE,
                         "Block 0 (vector table) must remain 0xFFFFFFFF when transfer is aborted")

        # Power cycle target
        self.node.power_cycle(self.bus.current_time)
        self.assertFalse(self.node.application_check_valid(),
                         "Blank vector table must fail application_check_valid")
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_BOOT_TRAPPED,
                         "Target must be trapped in bootloader recovery mode")

        # Re-flash valid image to restore normal operation
        self.assertTrue(self.flasher.connect(), "Must connect to trapped recovery target")
        self.assertTrue(self.flasher.flash_image(valid_fw, verify=True))
        self.assertEqual(self.node.state, SimulatedFishyServoNode.STATE_APP_RUNNING,
                         "Target must boot restored application")
        self.assertTrue(self.node.application_check_valid())

    def test_scenario_4_nvm_calibration_retention(self):
        """Scenario 4: NVM Calibration Retention.
        Verify that Sector 14 and 15 (0x0801_C000..0x0802_0000) remain 100% bit-exact across all operations.
        """
        # Run full sequence of Scenario 1, 2, and 3
        self.test_scenario_1_normal_handsfree_ota()
        self.test_scenario_2_uncooperative_app_boot_window_catch()
        self.test_scenario_3_interrupted_torn_ota_recovery()

        # Check Sector 14 and 15 data in flash
        current_nvm_data = self.node.flash.read_range(NVM_CAL_BASE, NVM_CAL_SIZE)
        current_nvm_crc = zlib.crc32(current_nvm_data)
        current_nvm_sha256 = hashlib.sha256(current_nvm_data).hexdigest()

        self.assertEqual(current_nvm_crc, self.initial_nvm_crc,
                         "NVM calibration CRC32 in Sectors 14/15 must remain bit-exact")
        self.assertEqual(current_nvm_sha256, self.initial_nvm_sha256,
                         "NVM calibration SHA-256 in Sectors 14/15 must remain bit-exact")
        self.assertEqual(current_nvm_data, self.nvm_data,
                         "NVM calibration payload must be identical byte-for-byte")

    def test_boundary_protection_rejection(self):
        """Verify BareBoot rejects any write attempting to breach bootloader or NVM calibration."""
        self.flasher.trigger_handsfree_reboot()
        self.flasher.connect()

        # Attempt to write into bootloader Sector 0 (< 0x0800_2000)
        bad_payload_low = struct.pack("<I", 0x08001000) + bytes([0xAA] * BLOCK_SIZE)
        parsed_low, _ = self.bus.exchange(self.flasher.addr, CMD_BLOCK_WRITE, bad_payload_low, read_len=32)
        self.assertFalse(parsed_low and parsed_low["cmd"] == RESP_ACK,
                         "BareBoot must reject writes below APP_BASE (Sector 0 bootloader protection)")

        # Attempt to write into NVM Calibration Sector 14 (>= 0x0801_C000)
        bad_payload_high = struct.pack("<I", NVM_CAL_BASE) + bytes([0xBB] * BLOCK_SIZE)
        parsed_high, _ = self.bus.exchange(self.flasher.addr, CMD_BLOCK_WRITE, bad_payload_high, read_len=32)
        self.assertFalse(parsed_high and parsed_high["cmd"] == RESP_ACK,
                         "BareBoot must reject writes into NVM Calibration (Sector 14/15 protection)")


def run_production_verification_suite():
    print("=" * 80)
    print(" BAREBOOT PRODUCTION LOCKOUT & RECOVERY VERIFICATION SUITE (NO SWD)")
    print("=" * 80)
    print(" Target Hardware:  FishyServo (STM32H503KB Arm Cortex-M33)")
    print(" Interface Bus:    2-Wire Serial Bus (I2C/MIPI I3C on PB3/PB4)")
    print(" Production Rule:  SWD Disconnected / Bus-Only Operation")
    print(" Flash Map:        0x0800_0000..0x0800_2000: BareBoot Bootloader (8 KiB)")
    print("                   0x0800_2000..0x0801_C000: FishyServo App Slot (104 KiB)")
    print("                   0x0801_C000..0x0802_0000: H5FlashStorage NVM (16 KiB)")
    print("=" * 80)

    suite = unittest.TestLoader().loadTestsFromTestCase(TestProductionLockoutAndRecovery)
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    print("\n" + "=" * 80)
    print(" PRODUCTION LOCKOUT & RECOVERY VERIFICATION SUMMARY:")
    print(f"  Tests Executed:   {result.testsRun}")
    print(f"  Failures:         {len(result.failures)}")
    print(f"  Errors:           {len(result.errors)}")
    print(f"  Verification:     {'PASSED - ALL PRODUCTION LOCKOUT SCENARIOS VALIDATED' if result.wasSuccessful() else 'FAILED'}")
    print("=" * 80)

    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(run_production_verification_suite())
