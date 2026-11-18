#!/usr/bin/env python3
import time
import struct
import sys
import usb.core
import usb.util

def crc16_katapult(buf: bytes) -> int:
    crc = 0xffff
    for b in buf:
        d = (b ^ (crc & 0xff)) & 0xff
        d = (d ^ (d << 4)) & 0xff
        crc = (((d << 8) | (crc >> 8)) ^ (d >> 4) ^ (d << 3)) & 0xffff
    return crc

def make_frame(command: int, payload: bytes = b'') -> bytes:
    wc = (len(payload) + 3) // 4
    padded = bytearray(payload)
    while len(padded) < wc * 4:
        padded.append(0)
    out = bytearray([0x01, 0x88, command, wc])
    out.extend(padded)
    crc = crc16_katapult(out[2:])
    out.append(crc & 0xff)
    out.append((crc >> 8) & 0xff)
    out.append(0x99)
    out.append(0x03)
    return bytes(out)

class KatapultI2C:
    def __init__(self, addr: int = 0x40):
        self.addr = addr
        self.dev = usb.core.find(idVendor=0x1a86, idProduct=0x5512)
        if self.dev is None:
            raise RuntimeError("CH341 device not found (1a86:5512)")
        try:
            if self.dev.is_kernel_driver_active(0):
                self.dev.detach_kernel_driver(0)
        except Exception:
            pass

    def write_streaming(self, data: bytes):
        full_data = bytes([self.addr << 1]) + data
        idx = 0
        while idx < len(full_data):
            chunk = full_data[idx:idx+28]
            is_first = (idx == 0)
            is_last = (idx + len(chunk) >= len(full_data))
            
            cmd = bytearray([0xAA])
            if is_first:
                cmd.append(0x74)
            cmd.append(0x80 | len(chunk))
            cmd.extend(chunk)
            if is_last:
                cmd.append(0x75)
            cmd.append(0x00)
            self.dev.write(0x02, bytes(cmd), timeout=1000)
            idx += len(chunk)

    def read(self, count: int) -> bytes:
        res = bytearray()
        while count > 0:
            chunk = min(count, 28)
            cmd = bytearray([0xAA, 0x74, 0x81, (self.addr << 1) | 1])
            for _ in range(chunk - 1):
                cmd.append(0xC1)
            cmd.append(0xC0)
            cmd.extend([0x75, 0x00])
            self.dev.write(0x02, bytes(cmd), timeout=1000)
            data = self.dev.read(0x82, 32, timeout=1000)
            res.extend(data)
            count -= chunk
        return bytes(res)

    def send_and_read(self, command: int, payload: bytes = b'', expect_len: int = 16, timeout: float = 1.0) -> bytes:
        frame = make_frame(command, payload)
        self.write_streaming(frame)
        deadline = time.time() + timeout
        while time.time() < deadline:
            time.sleep(0.005)
            res = self.read(expect_len)
            if len(res) >= 4 and res[0] == 0x01 and res[1] == 0x88:
                return res
        return res

def main():
    bin_path = "test-firmware/blink-h503/target/thumbv8m.main-none-eabihf/release/blink-h503.bin"
    if len(sys.argv) > 1:
        bin_path = sys.argv[1]

    with open(bin_path, "rb") as f:
        firmware = f.read()

    print(f"Connecting to Katapult I2C target at 0x40 via CH341...")
    i2c = KatapultI2C(0x40)

    # 1. CONNECT
    resp = i2c.send_and_read(0x11, b'', expect_len=44, timeout=1.0)
    print("CONNECT response:", resp.hex())
    if resp[0:2] != b'\x01\x88':
        print("Invalid response sync header:", resp[:4].hex())
        sys.exit(1)

    cmd_ack, proto, app_base, blk_size = struct.unpack('<IIII', resp[4:20])
    mcu = resp[20:32].split(b'\x00')[0].decode('latin1', 'ignore')
    print(f"Device: MCU={mcu}, Protocol=0x{proto:06X}, AppBase=0x{app_base:08X}, BlockSize={blk_size}")
    assert blk_size == 64, f"Expected block size 64, got {blk_size}"

    # 2. Flash blocks
    total_blocks = (len(firmware) + blk_size - 1) // blk_size
    print(f"Flashing {len(firmware)} bytes ({total_blocks} blocks of {blk_size} bytes)...")

    start_time = time.time()
    for block_idx in range(total_blocks):
        offset = block_idx * blk_size
        block_data = firmware[offset:offset+blk_size]
        if len(block_data) < blk_size:
            block_data = block_data + b'\xff' * (blk_size - len(block_data))
        
        target_addr = app_base + offset
        payload = struct.pack('<I', target_addr) + block_data

        # Send RX_BLOCK (0x12)
        resp = i2c.send_and_read(0x12, payload, expect_len=16, timeout=1.0)
        if len(resp) < 16 or resp[0:2] != b'\x01\x88':
            print(f"\nBlock {block_idx} failed! Bad response: {resp.hex()}")
            sys.exit(1)
        
        acked_cmd, acked_addr = struct.unpack('<II', resp[4:12])
        if acked_cmd != 0x12 or acked_addr != target_addr:
            print(f"\nBlock {block_idx} NACK/Error! cmd=0x{acked_cmd:02X}, addr=0x{acked_addr:08X} (expected addr 0x{target_addr:08X})")
            sys.exit(1)

        percent = (block_idx + 1) * 100 // total_blocks
        print(f"\rProgress: [{block_idx+1}/{total_blocks}] {percent}% (addr 0x{target_addr:08X})", end="", flush=True)

    elapsed = time.time() - start_time
    print(f"\nFlashing completed in {elapsed:.2f}s ({len(firmware)/elapsed/1024:.2f} KB/s)")

    # 3. COMPLETE (0x15)
    print("Sending COMPLETE (0x15)...")
    resp = i2c.send_and_read(0x15, b'', expect_len=12, timeout=1.0)
    print("COMPLETE response:", resp.hex())
    acked_cmd = struct.unpack('<I', resp[4:8])[0]
    if acked_cmd == 0x15:
        print("Flashing succeeded! Target jumping to application.")
    else:
        print(f"Warning: Unexpected COMPLETE ACK: 0x{acked_cmd:02X}")

if __name__ == "__main__":
    main()
