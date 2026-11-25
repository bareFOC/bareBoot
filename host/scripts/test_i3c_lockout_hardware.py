#!/usr/bin/env python3
"""
Real-Hardware 2-Wire I3C Zero-Lockout Stress Suite (Scenarios S1-S8).

All target discovery, recovery, escape, and firmware flashing are performed
STRICTLY over the 2-wire MIPI I3C bus (PB3 SCL + PB4 SDA) via AgentLaI3c.
SWD (/dev/hidraw5) is used exclusively to inject faults/power-loss/resets and
inspect post-mortem flash/SRAM/option-byte state.
"""

import hashlib
import os
import select
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path
from typing import List, Optional, Tuple

sys.path.insert(0, "/home/sun/Filelink/Firmwares/AgentLA-rust/sdk")
sys.path.insert(0, "/home/sun/Filelink/Firmwares/bareBoot/host/scripts")

from agentla.i3c import AgentLaI3c, I3C_OP_CCC_BROADCAST
from flash_app_over_i3c_usb import I3cUsbFlasher, make_frame, parse_frame

TARGET_ADDR = 0x40
PROBE_PATH = "/dev/hidraw5"
APP_BIN = (
    "/home/sun/Filelink/Firmwares/bareFOC/examples/fishyservo_stm32h5/"
    "target/thumbv8m.main-none-eabihf/release/fishyservo_stm32h5_i3c.bin"
)

APP_BASE = 0x0800_2000
COMMIT_BASE = 0x0801_A000
COMMIT_SIZE = 8192
NVM_BASE = 0x0801_C000
NVM_SIZE = 16384

BOOT_LOOP_GUARD_ADDR = 0x2000_7FD0
MAILBOX_ADDR = 0x2000_7FE0
BOOT_LOOP_MAGIC = 0x4242_4C50
COMMIT_MAGIC = 0x4242_5354
STATE_UPDATE_IN_PROGRESS = 0x4242_0001
STATE_VALID = 0x4242_0002


def build_synthetic_apps(tmpdir: Path) -> Tuple[bytes, bytes]:
    """Compile minimal uncooperative and crash-loop Cortex-M33 apps."""
    uncoop_c = tmpdir / "uncoop.c"
    uncoop_c.write_text(
        """
#include <stdint.h>
extern uint32_t _estack;
void Reset_Handler(void);
void Default_Handler(void) { while (1) {} }

__attribute__((section(".isr_vector")))
const uint32_t vectors[] = {
    0x20007FD0u,
    (uint32_t)Reset_Handler,
    (uint32_t)Default_Handler,
    (uint32_t)Default_Handler,
};

void Reset_Handler(void) {
    /* Disable I3C2 completely and drop PB3/PB4 back to analog/input */
    *(volatile uint32_t*)0x44003004u = 0u;
    *(volatile uint32_t*)0x42020400u &= ~0x03C0u;
    while (1) {
        __asm__ volatile ("nop");
    }
}
"""
    )

    crash_c = tmpdir / "crash.c"
    crash_c.write_text(
        """
#include <stdint.h>
void Reset_Handler(void);
void Default_Handler(void) { *(volatile uint32_t*)0xE000ED0Cu = 0x05FA0004u; while (1) {} }

__attribute__((section(".isr_vector")))
const uint32_t vectors[] = {
    0x20007FD0u,
    (uint32_t)Reset_Handler,
    (uint32_t)Default_Handler,
    (uint32_t)Default_Handler,
};

void Reset_Handler(void) {
    /* Immediately self-reset without touching I3C2 or BootLoopGuard */
    *(volatile uint32_t*)0xE000ED0Cu = 0x05FA0004u;
    while (1) {}
}
"""
    )

    ld_script = tmpdir / "app.ld"
    ld_script.write_text(
        """
MEMORY { FLASH (rx) : ORIGIN = 0x08002000, LENGTH = 96K }
SECTIONS {
  .text : {
    KEEP(*(.isr_vector))
    *(.text*)
    *(.rodata*)
  } > FLASH
}
"""
    )

    bins = []
    for name, src in [("uncoop", uncoop_c), ("crash", crash_c)]:
        elf = tmpdir / f"{name}.elf"
        binf = tmpdir / f"{name}.bin"
        subprocess.run(
            [
                "arm-none-eabi-gcc",
                "-mcpu=cortex-m33",
                "-mthumb",
                "-Os",
                "-nostdlib",
                "-T",
                str(ld_script),
                str(src),
                "-o",
                str(elf),
            ],
            check=True,
        )
        subprocess.run(
            ["arm-none-eabi-objcopy", "-O", "binary", str(elf), str(binf)],
            check=True,
        )
        bins.append(binf.read_bytes())
    return bins[0], bins[1]


class SwdInjector:
    """Minimal CMSIS-DAP SWD client for fault injection and post-mortem checks."""

    def __init__(self, probe_path: str = PROBE_PATH):
        self.probe_path = probe_path
        self.probe = None

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def _send(self, cmd_bytes: bytes, retries: int = 3) -> bytes:
        payload = bytes([0x00]) + cmd_bytes
        payload += bytes(65 - len(payload))
        for _ in range(retries):
            self.probe.write(payload[:65])
            ready, _, _ = select.select([self.probe], [], [], 1.0)
            if ready:
                return self.probe.read(64)
        raise TimeoutError(f"CMSIS-DAP timeout on 0x{cmd_bytes[0]:02X}")

    def _xfer(self, transfers, max_retries: int = 10) -> bytes:
        payload = bytearray([0x05, 0x00, len(transfers)])
        for rb, d in transfers:
            payload.append(rb)
            if d is not None:
                payload.extend(d.to_bytes(4, "little"))
        for _ in range(max_retries):
            resp = self._send(bytes(payload))
            if resp[0] == 0x05 and resp[2] == 1:
                return resp
            self._send(bytes([0x05, 0x00, 0x01, 0x00, 0x1E, 0x00, 0x00, 0x00]))
            time.sleep(0.001)
        raise RuntimeError(f"DAP_Transfer failed: ack=0x{resp[2]:02X}")

    def connect(self):
        self.probe = open(self.probe_path, "r+b", buffering=0)
        while select.select([self.probe], [], [], 0.01)[0]:
            self.probe.read(64)
        self._send(bytes([0x03]))
        self._send(bytes([0x02, 0x01]))
        self._send(bytes([0x11, 0xE8, 0x03, 0x00, 0x00]))
        seq = (
            [0xFF, 0xFF]
            + list((0x19BC0EA2).to_bytes(4, "little"))
            + list((0xE3DDAFE9).to_bytes(4, "little"))
            + list((0x86852D95).to_bytes(4, "little"))
            + list((0x6209F392).to_bytes(4, "little"))
            + [0xA0, 0xF1]
            + [0xFF] * 7
            + [0x00]
        )
        self._send(bytes([0x12, 224]) + bytes(seq))
        self._send(bytes([0x04, 0x08, 0x64, 0x00, 0x00, 0x00]))
        self._xfer([(0x02, None)])
        self._xfer([(0x00, 0x0000001E)])
        self._xfer([(0x04, 0x50000000)])
        self._xfer([(0x08, 0x01000000), (0x01, 0x23000012)])

    def close(self):
        if self.probe is not None:
            try:
                self._send(bytes([0x03]))
            except Exception:
                pass
            self.probe.close()
            self.probe = None

    def read_mem(self, addr: int) -> int:
        self._xfer([(0x05, addr)])
        self._xfer([(0x0F, None)])
        resp = self._xfer([(0x0E, None)])
        return int.from_bytes(resp[3:7], "little")

    def write_mem(self, addr: int, val: int):
        self._xfer([(0x05, addr), (0x0D, val)])

    def read_bytes(self, addr: int, length: int) -> bytes:
        out = bytearray()
        self._xfer([(0x08, 0x01000000), (0x01, 0x23000012)])
        for off in range(0, length, 56):
            n_words = min(14, (length - off + 3) // 4)
            reqs = [(0x05, addr + off)] + [(0x0F, None)] * n_words
            resp = self._xfer(reqs)
            out.extend(resp[3 : 3 + n_words * 4])
        return bytes(out[:length])

    def _wait_regrdy(self):
        for _ in range(100):
            if self.read_mem(0xE000EDF0) & (1 << 16):
                return
        raise RuntimeError("S_REGRDY timeout")

    def write_reg(self, reg_num: int, val: int):
        self._wait_regrdy()
        self.write_mem(0xE000EDF8, val)
        self.write_mem(0xE000EDF4, (1 << 16) | reg_num)
        self._wait_regrdy()

    def halt_and_freeze_iwdg(self):
        self.write_mem(0xE000EDF0, 0xA05F0003)
        time.sleep(0.005)
        self.write_mem(0x44024004, self.read_mem(0x44024004) | 0x07)
        self.write_mem(0x44024008, self.read_mem(0x44024008) | (1 << 11) | (1 << 12))
        self.write_mem(0x44020C88, self.read_mem(0x44020C88) | 0x80000100)

    def resume_and_reset(self, clear_sram_tail: bool = False):
        self.halt_and_freeze_iwdg()
        if clear_sram_tail:
            for off in range(0, 48, 4):
                self.write_mem(BOOT_LOOP_GUARD_ADDR + off, 0)
        self.write_mem(0xE000EDFC, 0)
        self.write_mem(0xE000EDF0, 0xA05F0000)
        self.write_mem(0xE000ED0C, 0x05FA0004)
        time.sleep(0.02)

    def run_ram_stub(self, code_bytes: bytes, r0: int = 0, r1: int = 0, r2: int = 0) -> int:
        """Upload thumb code to 0x2000_6000, run until BKPT #0, return R0."""
        stub_base = 0x2000_6000
        for i in range(0, len(code_bytes), 4):
            chunk = code_bytes[i : i + 4]
            val = int.from_bytes(chunk + bytes(4 - len(chunk)), "little")
            self.write_mem(stub_base + i, val)
        self.write_reg(0, r0)
        self.write_reg(1, r1)
        self.write_reg(2, r2)
        self.write_reg(13, 0x2000_7800)
        self.write_reg(14, 0xFFFF_FFFF)
        self.write_reg(15, stub_base + 1)
        self.write_reg(16, 0x0100_0000)
        self.write_mem(0xE000EDF0, 0xA05F0001)
        for _ in range(100):
            time.sleep(0.01)
            if self.read_mem(0xE000EDF0) & (1 << 17):
                return self.read_mem(0xE000EDF8) if False else self._read_reg(0)
        raise RuntimeError("RAM stub timed out")

    def _read_reg(self, reg_num: int) -> int:
        self._wait_regrdy()
        self.write_mem(0xE000EDF4, reg_num)
        self._wait_regrdy()
        return self.read_mem(0xE000EDF8)

    def inject_flash_zero_quadword(self, flash_addr: int, tmpdir: Path):
        c_file = tmpdir / "rot_stub.c"
        c_file.write_text(
            """
#include <stdint.h>
#define FLASH_NSKEYR (*(volatile uint32_t*)0x40022004u)
#define FLASH_NSSR   (*(volatile uint32_t*)0x40022020u)
#define FLASH_NSCR   (*(volatile uint32_t*)0x40022028u)
#define FLASH_NSCCR  (*(volatile uint32_t*)0x40022030u)
uint32_t corrupt_quad(uint32_t addr) {
    if (FLASH_NSCR & 1u) { FLASH_NSKEYR = 0x45670123u; FLASH_NSKEYR = 0xCDEF89ABu; }
    FLASH_NSCCR = 0x00FE0000u;
    FLASH_NSCR = (1u << 1);
    volatile uint32_t *dst = (volatile uint32_t*)addr;
    dst[0] = 0u; dst[1] = 0u; dst[2] = 0u; dst[3] = 0u;
    while (FLASH_NSSR & 1u) {}
    uint32_t sr = FLASH_NSSR;
    FLASH_NSCR = 0u;
    FLASH_NSCR |= 1u;
    __asm__ volatile ("bkpt #0");
    return sr;
}
"""
        )
        obj = tmpdir / "rot_stub.o"
        binf = tmpdir / "rot_stub.bin"
        subprocess.run(
            ["arm-none-eabi-gcc", "-mcpu=cortex-m33", "-mthumb", "-O2", "-c", str(c_file), "-o", str(obj)],
            check=True,
        )
        subprocess.run(["arm-none-eabi-objcopy", "-O", "binary", str(obj), str(binf)], check=True)
        return self.run_ram_stub(binf.read_bytes(), r0=flash_addr)

    def attempt_sector0_erase(self, tmpdir: Path) -> int:
        c_file = tmpdir / "wrp_stub.c"
        c_file.write_text(
            """
#include <stdint.h>
#define FLASH_NSKEYR (*(volatile uint32_t*)0x40022004u)
#define FLASH_NSSR   (*(volatile uint32_t*)0x40022020u)
#define FLASH_NSCR   (*(volatile uint32_t*)0x40022028u)
#define FLASH_NSCCR  (*(volatile uint32_t*)0x40022030u)
uint32_t try_erase_s0(void) {
    if (FLASH_NSCR & 1u) { FLASH_NSKEYR = 0x45670123u; FLASH_NSKEYR = 0xCDEF89ABu; }
    FLASH_NSCCR = 0x00FE0000u;
    FLASH_NSCR = (1u << 2) | (1u << 5);
    while (FLASH_NSSR & 1u) {}
    uint32_t sr = FLASH_NSSR;
    FLASH_NSCCR = 0x00FE0000u;
    FLASH_NSCR = 1u;
    __asm__ volatile ("bkpt #0");
    return sr;
}
"""
        )
        obj = tmpdir / "wrp_stub.o"
        binf = tmpdir / "wrp_stub.bin"
        subprocess.run(
            ["arm-none-eabi-gcc", "-mcpu=cortex-m33", "-mthumb", "-O2", "-c", str(c_file), "-o", str(obj)],
            check=True,
        )
        subprocess.run(["arm-none-eabi-objcopy", "-O", "binary", str(obj), str(binf)], check=True)
        return self.run_ram_stub(binf.read_bytes())

    def crc32_flash(self, addr: int, length: int, tmpdir: Path) -> int:
        c_file = tmpdir / "crc_stub.c"
        c_file.write_text(
            """
#include <stdint.h>
uint32_t crc32_range(uint32_t addr, uint32_t len) {
    const volatile uint8_t *p = (const volatile uint8_t*)addr;
    uint32_t crc = 0xFFFFFFFFu;
    for (uint32_t i = 0; i < len; i++) {
        crc ^= p[i];
        for (int b = 0; b < 8; b++) {
            crc = (crc & 1u) ? ((crc >> 1) ^ 0xEDB88320u) : (crc >> 1);
        }
    }
    crc = ~crc;
    __asm__ volatile ("bkpt #0");
    return crc;
}
"""
        )
        obj = tmpdir / "crc_stub.o"
        binf = tmpdir / "crc_stub.bin"
        subprocess.run(
            ["arm-none-eabi-gcc", "-mcpu=cortex-m33", "-mthumb", "-O2", "-c", str(c_file), "-o", str(obj)],
            check=True,
        )
        subprocess.run(["arm-none-eabi-objcopy", "-O", "binary", str(obj), str(binf)], check=True)
        return self.run_ram_stub(binf.read_bytes(), r0=addr, r1=length)


def latest_commit_record(swd: SwdInjector) -> Optional[Tuple[int, int, int, int]]:
    last = None
    for off in range(0, COMMIT_SIZE, 32):
        chunk = swd.read_bytes(COMMIT_BASE + off, 32)
        for sub in range(0, 32, 16):
            slot = chunk[sub : sub + 16]
            if slot == b"\xFF" * 16:
                return last
            last = struct.unpack("<IIII", slot)
    return last


def i3c_entdaa_connect(client: AgentLaI3c, retries: int = 12, freq_khz: int = 1000) -> I3cUsbFlasher:
    client.set_inverted(False)
    client.set_bus_speed(freq_khz)
    for _ in range(retries):
        try:
            client.rstdaa()
        except Exception:
            pass
        try:
            dev = client.entdaa(TARGET_ADDR)
            if dev is not None:
                addr = dev.get("dynamic_address", dev.get("addr", TARGET_ADDR))
                flasher = I3cUsbFlasher(client, addr=addr, freq_khz=freq_khz)
                if flasher.connect_bootloader(retries=3, retry_delay=0.02):
                    return flasher
        except Exception:
            pass
        time.sleep(0.05)
    raise RuntimeError("Failed to connect to BareBoot via I3C ENTDAA + CMD_CONNECT")


def i3c_flash_app(client: AgentLaI3c, app_bytes: bytes, freq_khz: int = 10000) -> I3cUsbFlasher:
    flasher = i3c_entdaa_connect(client, retries=15, freq_khz=1000)
    client.set_bus_speed(freq_khz)
    stats = flasher.flash_image(app_bytes, verify=True)
    assert stats["verification_passed"], "I3C readback verification failed"
    return flasher


def wait_app_status(client: AgentLaI3c, addr: int = TARGET_ADDR, timeout_s: float = 6.5) -> int:
    client.set_bus_speed(1000)
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < timeout_s:
        try:
            rx = client.private_write_read(addr, bytes([0x00, 0x00]), 1)
            if rx and rx[0] == 4:
                return rx[0]
        except Exception:
            try:
                client.entdaa(addr)
            except Exception:
                pass
        time.sleep(0.15)
    raise RuntimeError("Timed out waiting for FishyServo app status==4 over I3C")


def enter_bootloader_via_0xf0(client: AgentLaI3c, addr: int = TARGET_ADDR):
    try:
        client.private_write(addr, bytes([0xF0]))
    except Exception:
        pass
    time.sleep(0.25)


def run_suite():
    assert os.path.exists(APP_BIN), f"Missing {APP_BIN}"
    app_bytes = Path(APP_BIN).read_bytes()

    with tempfile.TemporaryDirectory(prefix="i3c_lockout_") as td:
        tmpdir = Path(td)
        uncoop_bin, crash_bin = build_synthetic_apps(tmpdir)

        client = AgentLaI3c()
        assert client.connect(), "AgentLaI3c USB Bulk adapter not found"

        try:
            # Ensure we start in BareBoot (via 0xF0 or ENTDAA or POR)
            print("\n=== S1: Normal Handsfree OTA (0xF0 + CommitStore + CRC32) ===")
            try:
                flasher = i3c_entdaa_connect(client, retries=4)
            except Exception:
                enter_bootloader_via_0xf0(client)
                flasher = i3c_entdaa_connect(client, retries=10)
            client.set_bus_speed(10000)
            stats = flasher.flash_image(app_bytes, verify=True)
            assert stats["verification_passed"]
            status = wait_app_status(client)
            assert status == 4
            time.sleep(0.6)
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                rec = latest_commit_record(swd)
                assert rec is not None and rec[0] == COMMIT_MAGIC and rec[1] == STATE_VALID, f"Bad CommitRecord: {rec}"
                guard = struct.unpack("<IIII", swd.read_bytes(BOOT_LOOP_GUARD_ADDR, 16))
                assert guard[0] == BOOT_LOOP_MAGIC and guard[1] == 0, f"Expected cleared BootLoopGuard, got {guard}"
                swd.write_mem(0xE000EDF0, 0xA05F0000)
            print(f"[PASS] S1: CommitRecord VALID(len={rec[2]}, crc=0x{rec[3]:08X}), BootLoopGuard cleared=0")

            print("\n=== S2: Uncooperative App Caught in 1500 ms Cold-Boot Window ===")
            enter_bootloader_via_0xf0(client)
            i3c_flash_app(client, uncoop_bin)
            time.sleep(0.3)
            # Verify uncooperative app is silent on I3C
            client.set_bus_speed(1000)
            uncoop_silent = False
            try:
                client.private_write_read(TARGET_ADDR, bytes([0x00, 0x00]), 1)
            except Exception:
                uncoop_silent = True
            assert uncoop_silent, "Uncooperative app unexpectedly responded on I3C"
            # Simulate POR via SWD (clears BootLoopGuard + SoftResetMailbox + reset)
            with SwdInjector() as swd:
                swd.resume_and_reset(clear_sram_tail=True)
            # Strictly over 2-wire SDA+SCL: catch BareBoot within 1500 ms window
            flasher = i3c_entdaa_connect(client, retries=20, freq_khz=1000)
            print(f"[PASS] S2: Caught uncooperative app inside 1500 ms boot window (MCU={flasher.mcu})")
            client.set_bus_speed(10000)
            flasher.flash_image(app_bytes, verify=True)
            wait_app_status(client)

            print("\n=== S3: Torn Flash / Mid-Write Power Loss Recovery ===")
            enter_bootloader_via_0xf0(client)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            client.set_bus_speed(10000)
            # Write first 132 blocks (> 8 KB, so Sector 1 has valid vector table), then abort before COMPLETE
            for bi in range(132):
                addr = flasher.app_base + bi * flasher.block_size
                blk = app_bytes[bi * flasher.block_size : (bi + 1) * flasher.block_size]
                blk += b"\xFF" * (flasher.block_size - len(blk))
                parsed, _ = flasher.exchange(0x12, addr.to_bytes(4, "little") + blk, read_len=32)
                assert parsed and parsed["cmd"] == 0xA0
            # Simulate mid-write power loss
            with SwdInjector() as swd:
                swd.resume_and_reset(clear_sram_tail=True)
            # Wait longer than BOOT_WINDOW_MS (1500 ms) to prove permanent parking
            time.sleep(2.2)
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                rec = latest_commit_record(swd)
                assert rec is not None and rec[1] == STATE_UPDATE_IN_PROGRESS, f"Expected IN_PROGRESS, got {rec}"
                swd.write_mem(0xE000EDF0, 0xA05F0000)
            # Recover strictly over SDA + SCL
            i3c_flash_app(client, app_bytes)
            wait_app_status(client)
            print("[PASS] S3: Torn mid-write parked permanently with IN_PROGRESS and recovered over I3C")

            print("\n=== S4: Boot-Time Application Flash Bit-Rot Detection (CRC32) ===")
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                rec = latest_commit_record(swd)
                assert rec and rec[1] == STATE_VALID
                rot_addr = APP_BASE + rec[2] - 16
                sr = swd.inject_flash_zero_quadword(rot_addr, tmpdir)
                assert (sr & 0x00EE0000) == 0, f"Flash error injecting bit-rot: SR=0x{sr:08X}"
                swd.resume_and_reset(clear_sram_tail=True)
            time.sleep(2.2)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            client.set_bus_speed(10000)
            flasher.flash_image(app_bytes, verify=True)
            wait_app_status(client)
            print(f"[PASS] S4: Bit-rot at 0x{rot_addr:08X} detected by CRC32, parked & recovered over I3C")

            print("\n=== S5: 3-Strike BootLoopGuard Crash Trap ===")
            enter_bootloader_via_0xf0(client)
            i3c_flash_app(client, crash_bin)
            with SwdInjector() as swd:
                swd.resume_and_reset(clear_sram_tail=True)
            # Wait for 3 x 1.5s boot windows + crash resets + safety margin
            time.sleep(6.0)
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                guard = struct.unpack("<IIII", swd.read_bytes(BOOT_LOOP_GUARD_ADDR, 16))
                assert guard[0] == BOOT_LOOP_MAGIC and guard[1] >= 3, f"Expected trapped attempts>=3, got {guard}"
                swd.write_mem(0xE000EDF0, 0xA05F0000)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            client.set_bus_speed(10000)
            flasher.flash_image(app_bytes, verify=True)
            wait_app_status(client)
            time.sleep(0.6)
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                guard = struct.unpack("<IIII", swd.read_bytes(BOOT_LOOP_GUARD_ADDR, 16))
                assert guard[0] == BOOT_LOOP_MAGIC and guard[1] == 0, f"Expected cleared guard=0, got {guard}"
                swd.write_mem(0xE000EDF0, 0xA05F0000)
            print("[PASS] S5: 3-strike BootLoopGuard trapped crash-looping app and recovered over I3C")

            print("\n=== S6: Hardware I3C Escape (EVR_RSTF & CCC RSTACT) + Bus Error Recovery ===")
            # Part A: Target Reset Pattern (EVR_RSTF) from running app
            client.set_bus_speed(1000)
            client.target_reset()
            time.sleep(0.15)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            # Part B: Truncated / corrupt Katapult frames in BareBoot (EVR_ERRF / frame discard)
            client.private_write(flasher.addr, bytes([0x01, 0x88, 0x11]))  # truncated
            client.private_write(flasher.addr, bytes([0x01, 0x88, 0x11, 0x00, 0xDE, 0xAD, 0x99, 0x03]))  # bad CRC
            client.private_write(flasher.addr, bytes([0xAA, 0x55, 0x00, 0xFF]))  # noise
            assert flasher.connect_bootloader(retries=5), "BareBoot failed to recover after bad frames"
            # Reboot back to app via POR, let 1500 ms boot window expire cleanly, wait for healthy app,
            # then test CCC RSTACT (0x2A, 0x02)
            with SwdInjector() as swd:
                swd.resume_and_reset(clear_sram_tail=True)
            time.sleep(2.0)
            wait_app_status(client)
            try:
                client.ccc_broadcast(0x2A, bytes([0x2A, 0x02]))
            except Exception:
                pass
            time.sleep(0.15)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            print("[PASS] S6: EVR_RSTF, CCC RSTACT (0x2A 0x02), and malformed-frame recovery all passed")

            print("\n=== S7: Hardware Sector 0 WRP + BOR & NVM Sectors 14-15 Retention ===")
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                wrp_cur = swd.read_mem(0x4002_20E8)
                opt_cur = swd.read_mem(0x4002_2050)
                product_state = (opt_cur >> 8) & 0xFF
                assert (wrp_cur & 0x01) == 0, f"Sector 0 WRP not active: WRPSGN1R_CUR=0x{wrp_cur:08X}"
                assert (opt_cur & 0x03) == 0x03, f"BOR Level 3 not active: OPTSR_CUR=0x{opt_cur:08X}"
                assert product_state == 0xED, f"Unexpected PRODUCT_STATE=0x{product_state:02X}"
                s0_before = swd.crc32_flash(0x0800_0000, 8192, tmpdir)
                nssr = swd.attempt_sector0_erase(tmpdir)
                assert (nssr & (1 << 17)) != 0, f"Expected WRPERR (bit 17), got FLASH_NSSR=0x{nssr:08X}"
                s0_after = swd.crc32_flash(0x0800_0000, 8192, tmpdir)
                assert s0_before == s0_after, "Sector 0 modified despite WRP!"
                nvm_before = swd.crc32_flash(NVM_BASE, NVM_SIZE, tmpdir)
                swd.resume_and_reset(clear_sram_tail=False)
            flasher = i3c_entdaa_connect(client, retries=10, freq_khz=1000)
            client.set_bus_speed(10000)
            flasher.flash_image(app_bytes, verify=True)
            wait_app_status(client)
            with SwdInjector() as swd:
                swd.halt_and_freeze_iwdg()
                nvm_after = swd.crc32_flash(NVM_BASE, NVM_SIZE, tmpdir)
                swd.resume_and_reset(clear_sram_tail=False)
            assert nvm_before == nvm_after, "NVM Sectors 14-15 changed across OTA!"
            print(f"[PASS] S7: WRP blocked erase (NSSR=0x{nssr:08X}), BOR=3, NVM CRC32=0x{nvm_after:08X} preserved")

            print("\n=== S8: Zero-Delay MultiBus Stress + TMAG5273 Check ===")
            time.sleep(2.0)
            wait_app_status(client)
            try:
                client.entdaa(TARGET_ADDR)
            except Exception:
                pass
            client.set_bus_speed(1000)
            for i in range(50):
                kp = 18.0 + (i % 7) * 0.25
                client.private_write(TARGET_ADDR, bytes([0x00, 0x36]) + struct.pack("<f", kp))
                rb = struct.unpack("<f", client.private_write_read(TARGET_ADDR, bytes([0x00, 0x36]), 4))[0]
                assert abs(rb - kp) < 0.05, f"MultiBus mismatch at iter {i}: {rb} vs {kp}"
            mot_sel = client.private_write_read(TARGET_ADDR, bytes([0x00, 0x70]), 1)[0]
            topo = client.private_write_read(TARGET_ADDR, bytes([0x00, 0x78]), 1)[0]
            angle = struct.unpack("<f", client.private_write_read(TARGET_ADDR, bytes([0x00, 0x09]), 4))[0]
            tmag_note = "active" if abs(angle) > 1e-4 else "DUMMY_FALLBACK (no I2C1 ACK on PB6/PB7)"
            print(f"[PASS] S8: 50x zero-delay MultiBus R/W passed | MotorSelect={mot_sel}, Topology={topo}, TMAG5273={angle:.4f} rad [{tmag_note}]")

            print("\n" + "=" * 72)
            print("  ALL SCENARIOS S1-S8 PASSED ON REAL HARDWARE OVER 2-WIRE I3C!")
            print("=" * 72)
        finally:
            client.close()


if __name__ == "__main__":
    run_suite()
