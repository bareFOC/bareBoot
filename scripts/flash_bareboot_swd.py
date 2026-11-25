#!/usr/bin/env python3
"""
flash_bareboot_swd.py

Flashes BareBoot (artifacts/bareboot-stm32h503-i3c-i2c.bin) into STM32H503 Sector 0
via native CMSIS-DAP over SWD (/dev/hidraw5) using an in-RAM execution stub.
"""

import argparse
import select
import sys
import time
import struct
import subprocess
from pathlib import Path

DEFAULT_PROBE_PATH = "/dev/hidraw5"
DEFAULT_BAREBOOT_BIN = "/home/sun/Filelink/Firmwares/bareBoot/artifacts/bareboot-stm32h503-i3c-i2c.bin"

parser = argparse.ArgumentParser(description="Flash BareBoot into STM32H503 Sector 0 via AgentLA SWD.")
parser.add_argument("--bin", dest="bin_path", default=DEFAULT_BAREBOOT_BIN, help="Path to BareBoot .bin image")
parser.add_argument("--probe", dest="probe_path", default=DEFAULT_PROBE_PATH, help="CMSIS-DAP hidraw device path")
args = parser.parse_args()

PROBE_PATH = args.probe_path
BAREBOOT_BIN = Path(args.bin_path)

if not BAREBOOT_BIN.exists():
    print(f"Error: {BAREBOOT_BIN} not found")
    sys.exit(1)

bareboot_data = BAREBOOT_BIN.read_bytes()
print(f"[*] BareBoot image: {len(bareboot_data)} bytes")

# Pad to multiple of 16 bytes (quadword)
if len(bareboot_data) % 16 != 0:
    bareboot_data += b"\xFF" * (16 - (len(bareboot_data) % 16))
num_quads = len(bareboot_data) // 16
print(f"[*] Padded to {len(bareboot_data)} bytes ({num_quads} quad-words)")

# Build flash loader stub
c_code = '''
#include <stdint.h>

#define FLASH_NSKEYR  (*(volatile uint32_t*)0x40022004)
#define FLASH_OPTKEYR (*(volatile uint32_t*)0x4002200C)
#define FLASH_OPTCR   (*(volatile uint32_t*)0x4002201C)
#define FLASH_NSSR    (*(volatile uint32_t*)0x40022020)
#define FLASH_NSCR    (*(volatile uint32_t*)0x40022028)
#define FLASH_NSCCR   (*(volatile uint32_t*)0x40022030)
#define FLASH_WRP1CUR (*(volatile uint32_t*)0x400220E8)
#define FLASH_WRP1PRG (*(volatile uint32_t*)0x400220EC)
#define RCC_AHB1ENR   (*(volatile uint32_t*)0x44020C88)

void erase_sector(uint32_t sector) {
    RCC_AHB1ENR |= (1 << 8) | (1 << 31);
    while (FLASH_NSSR & 1);
    FLASH_NSCCR = 0x00FE0000;
    if (sector == 0 && ((FLASH_WRP1CUR & 1) == 0)) {
        if (FLASH_OPTCR & 1) {
            FLASH_OPTKEYR = 0x08192A3B;
            FLASH_OPTKEYR = 0x4C5D6E7F;
            while (FLASH_OPTCR & 1);
        }
        FLASH_WRP1PRG = FLASH_WRP1CUR | 1;
        FLASH_OPTCR |= (1 << 1);
        while (FLASH_OPTCR & (1 << 1));
        FLASH_OPTCR |= 1;
    }
    if (FLASH_NSCR & 1) {
        FLASH_NSKEYR = 0x45670123;
        FLASH_NSKEYR = 0xCDEF89AB;
    }
    FLASH_NSCCR = 0x00FE0000;
    FLASH_NSCR = (1 << 2) | (1 << 5) | ((sector & 0x7F) << 6);
    while (FLASH_NSSR & 1);
    FLASH_NSCR &= ~( (1 << 2) | (1 << 5) | (0x7F << 6) );
    FLASH_NSCR |= 1;
    __asm__ volatile ("bkpt #0");
}

void write_quadwords(uint32_t flash_addr, const uint32_t *src, uint32_t quads) {
    RCC_AHB1ENR |= (1 << 8) | (1 << 31);
    if (FLASH_NSCR & 1) {
        FLASH_NSKEYR = 0x45670123;
        FLASH_NSKEYR = 0xCDEF89AB;
    }
    FLASH_NSCCR = 0x00FE0000;
    FLASH_NSCR |= (1 << 1); // PG
    volatile uint32_t *dst = (volatile uint32_t *)flash_addr;
    for (uint32_t q = 0; q < quads; q++) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = src[3];
        while (FLASH_NSSR & 1);
        dst += 4;
        src += 4;
    }
    FLASH_NSCR &= ~(1 << 1);
    FLASH_NSCR |= 1;
    __asm__ volatile ("bkpt #0");
}
'''

with open('/tmp/flasher_stub.c', 'w') as f:
    f.write(c_code)

subprocess.run(['arm-none-eabi-gcc', '-mcpu=cortex-m33', '-mthumb', '-O2', '-c', '/tmp/flasher_stub.c', '-o', '/tmp/flasher_stub.o'], check=True)

# Find entry offsets
res = subprocess.check_output(['arm-none-eabi-readelf', '-s', '/tmp/flasher_stub.o']).decode()
erase_off = None
write_off = None
for line in res.splitlines():
    if 'erase_sector' in line:
        erase_off = int(line.split()[1], 16)
    elif 'write_quadwords' in line:
        write_off = int(line.split()[1], 16)

assert erase_off is not None and write_off is not None
print(f"[*] Stub offsets: erase_sector=+0x{erase_off:X}, write_quadwords=+0x{write_off:X}")

subprocess.run(['arm-none-eabi-objcopy', '-O', 'binary', '/tmp/flasher_stub.o', '/tmp/flasher_stub.bin'], check=True)
stub_bin = open('/tmp/flasher_stub.bin', 'rb').read()
print(f"[*] Stub binary: {len(stub_bin)} bytes")

# Connect probe
probe = open(PROBE_PATH, "r+b", buffering=0)
# Drain any stale HID reports left by an interrupted session
while select.select([probe], [], [], 0.01)[0]:
    probe.read(64)

def send_cmd(cmd_bytes, retries=3):
    payload = bytes([0x00]) + cmd_bytes
    payload = payload + bytes(65 - len(payload))
    for _ in range(retries):
        probe.write(payload[:65])
        ready, _, _ = select.select([probe], [], [], 1.0)
        if ready:
            return probe.read(64)
    raise TimeoutError(f"CMSIS-DAP HID read timeout for cmd 0x{cmd_bytes[0]:02X}")

def transfer(transfers, max_retries=10):
    payload = bytearray([0x05, 0x00, len(transfers)])
    for r_b, d in transfers:
        payload.append(r_b)
        if d is not None:
            payload.extend(d.to_bytes(4, "little"))
    for attempt in range(max_retries):
        resp = send_cmd(bytes(payload))
        if resp[0] == 0x05 and resp[2] == 1:
            return resp
        # Clear DP abort / sticky error flags
        send_cmd(bytes([0x05, 0x00, 0x01, 0x00, 0x1E, 0x00, 0x00, 0x00]))
        time.sleep(0.001)
    raise RuntimeError(f"DAP_Transfer failed after {max_retries} attempts: count={resp[1]} ack=0x{resp[2]:02X}")

def read_mem(addr):
    transfer([(0x05, addr)])
    transfer([(0x0F, None)])
    resp = transfer([(0x0E, None)])
    return int.from_bytes(resp[3:7], "little")

def write_mem(addr, val):
    transfer([(0x05, addr), (0x0D, val)])

def wait_regrdy():
    for _ in range(100):
        st = read_mem(0xE000EDF0)
        if st & (1 << 16):
            return
    raise RuntimeError("S_REGRDY timeout")

def write_reg(reg_num, val):
    wait_regrdy()
    write_mem(0xE000EDF8, val)
    write_mem(0xE000EDF4, (1 << 16) | reg_num)
    wait_regrdy()

def read_reg(reg_num):
    wait_regrdy()
    write_mem(0xE000EDF4, reg_num)
    wait_regrdy()
    return read_mem(0xE000EDF8)

# Connect sequence
send_cmd(bytes([0x03])) # Disconnect
print("DAP_Connect:", send_cmd(bytes([0x02, 0x01]))[:4].hex())
print("DAP_Clock:", send_cmd(bytes([0x11, 0xE8, 0x03, 0x00, 0x00]))[:4].hex()) # 1 MHz

print("DAP_Reset_Lo:", send_cmd(bytes([0x10, 0x00, 0x80, 0x20, 0x4E, 0x00, 0x00]))[:4].hex())
time.sleep(0.05)
print("DAP_Reset_Hi:", send_cmd(bytes([0x10, 0x80, 0x80, 0x20, 0x4E, 0x00, 0x00]))[:4].hex())
time.sleep(0.05)

# Dormant-to-SWD wake sequence
seq = (
    [0xFF, 0xFF] +
    list(0x19BC0EA2.to_bytes(4, 'little')) +
    list(0xE3DDAFE9.to_bytes(4, 'little')) +
    list(0x86852D95.to_bytes(4, 'little')) +
    list(0x6209F392.to_bytes(4, 'little')) +
    [0xA0, 0xF1] +
    [0xFF] * 7 +
    [0x00]
)
print("DAP_Seq:", send_cmd(bytes([0x12, 224]) + bytes(seq))[:4].hex())

# Configure transfer with 8 idle cycles
print("DAP_Cfg:", send_cmd(bytes([0x04, 0x08, 0x64, 0x00, 0x00, 0x00]))[:4].hex())

resp = transfer([(0x02, None)])
dpidr = int.from_bytes(resp[3:7], 'little')
print(f"[*] Target DPIDR: 0x{dpidr:08X}")

# Clear abort and power up debug system/core
transfer([(0x00, 0x0000001E)]) # DP_ABORT = 0x1E
transfer([(0x04, 0x50000000)]) # DP_CTRLSTAT = 0x50000000
time.sleep(0.01)

# Check DP_CTRLSTAT
resp = transfer([(0x06, None)])
ctrlstat = int.from_bytes(resp[3:7], 'little')
print(f"[*] DP_CTRLSTAT: 0x{ctrlstat:08X}")

transfer([(0x08, 0x01000000)]) # AP 1 (AHB-AP)
transfer([(0x01, 0x23000012)]) # CSW Auto-inc single, secure, privileged

# Halt core immediately via VC_CORERESET + SYSRESETREQ so IWDG is cleared and cannot fire during SWD programming
transfer([(0x05, 0xE000EDF0), (0x0D, 0xA05F0003)]) # DHCSR C_DEBUGEN | C_HALT
write_mem(0xE000EDFC, 0x00000001) # DEMCR: VC_CORERESET
write_mem(0xE000ED0C, 0x05FA0004) # AIRCR: SYSRESETREQ
time.sleep(0.02)
# Re-establish CSW after core reset and confirm halt
transfer([(0x00, 0x0000001E)])
transfer([(0x08, 0x01000000), (0x01, 0x23000012)])
transfer([(0x05, 0xE000EDF0), (0x0D, 0xA05F0003)])
write_mem(0xE000EDFC, 0x00000000) # Clear VC_CORERESET
# Freeze IWDG during debug halt (DBGMCU_CR + DBGMCU_APB1LFZR)
write_mem(0x44024004, read_mem(0x44024004) | 0x07)
write_mem(0x44024008, read_mem(0x44024008) | (1 << 11) | (1 << 12))

# Enable SRAM1 and FLITFEN
write_mem(0x44020C88, 0x80000100)

STUB_BASE = 0x20006000
DATA_BASE = 0x20000000

# Write stub to SRAM2
print("[*] Uploading flash loader stub to SRAM2...")
for i in range(0, len(stub_bin), 4):
    chunk = stub_bin[i:i+4]
    val = int.from_bytes(chunk + bytes(4 - len(chunk)), 'little')
    write_mem(STUB_BASE + i, val)

# 1. Erase Sector 0
print("[*] Erasing Flash Sector 0 (0x08000000..0x08002000)...")
write_reg(0, 0) # R0 = Sector 0
write_reg(13, 0x20007800) # SP
write_reg(14, 0xFFFFFFFF) # LR
write_reg(15, STUB_BASE + erase_off) # PC
write_reg(16, 0x01000000) # xPSR

# Run stub
transfer([(0x05, 0xE000EDF0), (0x0D, 0xA05F0001)])

# Wait for BKPT
for _ in range(50):
    time.sleep(0.01)
    dhcsr = read_mem(0xE000EDF0)
    if dhcsr & (1 << 17):
        break
else:
    raise RuntimeError("Sector 0 erase timed out")

print("[*] Sector 0 erased successfully!")

# Write payload to SRAM1 in 32-byte chunks (32 divides 1024 evenly so auto-inc never crosses 1KB page boundary)
print(f"[*] Uploading {len(bareboot_data)} bytes of BareBoot payload to SRAM1...", flush=True)
transfer([(0x08, 0x01000000), (0x01, 0x23000012)]) # Auto-inc on
for chunk_idx in range(0, len(bareboot_data), 32):
    if chunk_idx % 1024 == 0:
        print(f"[*] Uploaded {chunk_idx}/{len(bareboot_data)} bytes...", flush=True)
    chunk = bareboot_data[chunk_idx:chunk_idx+32]
    curr_addr = DATA_BASE + chunk_idx
    reqs = [(0x05, curr_addr)]
    for w_idx in range(0, len(chunk), 4):
        w = int.from_bytes(chunk[w_idx:w_idx+4], 'little')
        reqs.append((0x0D, w))
    transfer(reqs)
transfer([(0x08, 0x01000000), (0x01, 0x23000002)]) # Auto-inc off

print("[*] Payload uploaded to SRAM1 successfully.", flush=True)

# 2. Program Sector 0
print(f"[*] Programming {num_quads} quad-words into Sector 0...")
write_reg(0, 0x08000000) # R0 = flash dst
write_reg(1, DATA_BASE)   # R1 = sram src
write_reg(2, num_quads)   # R2 = quads count
write_reg(13, 0x20007800) # SP
write_reg(14, 0xFFFFFFFF) # LR
write_reg(15, STUB_BASE + write_off) # PC
write_reg(16, 0x01000000) # xPSR

# Run stub
transfer([(0x05, 0xE000EDF0), (0x0D, 0xA05F0001)])

# Wait for BKPT
for _ in range(100):
    time.sleep(0.01)
    dhcsr = read_mem(0xE000EDF0)
    if dhcsr & (1 << 17):
        break
else:
    raise RuntimeError("Flash programming timed out")

print("[*] Sector 0 programmed successfully!")

# 3. Verify Sector 0
print("[*] Verifying Sector 0...", flush=True)
transfer([(0x08, 0x01000000), (0x01, 0x23000012)]) # Auto-inc on
verified = True
for offset in range(0, len(bareboot_data), 32):
    if offset % 1024 == 0 and offset > 0:
        print(f"[*] Verified {offset}/{len(bareboot_data)} bytes...", flush=True)
    reqs = [(0x05, 0x08000000 + offset)]
    for _ in range(8):
        reqs.append((0x0F, None))
    resp = transfer(reqs)
    if resp[2] != 1:
        raise RuntimeError(f"Transfer error at offset 0x{offset:X}: ack=0x{resp[2]:02X}")
    read_bytes = resp[3:3+32]
    exp_chunk = bareboot_data[offset:offset+32]
    if read_bytes[:len(exp_chunk)] != exp_chunk:
        print(f"[!] Mismatch at offset 0x{offset:X}!")
        print(f"    Expected: {exp_chunk[:16].hex()}")
        print(f"    Actual  : {read_bytes[:len(exp_chunk)][:16].hex()}")
        verified = False
        break
    time.sleep(0.001)

transfer([(0x08, 0x01000000), (0x01, 0x23000002)]) # Auto-inc off

if not verified:
    raise RuntimeError("Verification failed!")

print(f"[*] VERIFICATION PASSED: All {len(bareboot_data)} bytes match perfectly!", flush=True)

# 4. Enforce factory Sector 0 WRP + BOR Level 3 before leaving SWD
print("[*] Enforcing Sector 0 WRP and BOR Level 3 option bytes...", flush=True)
wrp_cur = read_mem(0x400220E8)
opt_cur = read_mem(0x40022050)
if (wrp_cur & 0x01) != 0 or (opt_cur & 0x03) != 0x03:
    optcr = read_mem(0x4002201C)
    if optcr & 0x01:
        write_mem(0x4002200C, 0x08192A3B)
        write_mem(0x4002200C, 0x4C5D6E7F)
    write_mem(0x400220EC, wrp_cur & ~0x01)   # FLASH_WRP1PRG bit 0 = 0 protects Sector 0
    write_mem(0x40022054, (opt_cur & ~0x03) | 0x03)  # FLASH_OPTSR_PRG BOR Level 3
    write_mem(0x4002201C, read_mem(0x4002201C) | (1 << 1))  # OPTSTRT
    for _ in range(50):
        time.sleep(0.01)
        if (read_mem(0x4002201C) & (1 << 1)) == 0:
            break
    write_mem(0x4002201C, read_mem(0x4002201C) | 0x01)  # OPTLOCK
wrp_check = read_mem(0x400220E8)
opt_check = read_mem(0x40022050)
assert (wrp_check & 0x01) == 0, f"Sector 0 WRP not active: WRPSGN1R_CUR=0x{wrp_check:08X}"
assert (opt_check & 0x03) == 0x03, f"BOR Level 3 not active: OPTSR_CUR=0x{opt_check:08X}"
print(f"[*] Hardware Sector 0 WRP (0x{wrp_check:08X}) and BOR=3 (0x{opt_check:08X}) verified active!", flush=True)

# 5. Release core and reset target
print("[*] Resetting target into BareBoot...")
# Clear debug halt and release
transfer([(0x05, 0xE000EDFC), (0x0D, 0x00000000)])
transfer([(0x05, 0xE000EDF0), (0x0D, 0xA05F0000)]) # Disable debug halt
# Soft reset target
write_mem(0xE000ED0C, 0x05FA0004)
time.sleep(0.05)
send_cmd(bytes([0x03])) # DAP_Disconnect

probe.close()
print("[*] BareBoot installation complete!")
