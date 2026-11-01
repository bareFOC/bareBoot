#!/usr/bin/env python3
"""CH341 adapter loopback test: TX->RX jumper on the adapter itself.

Sends a deterministic LFSR pattern and verifies the adapter echoes it.
Run with no arguments to auto-detect /dev/ttyUSB*.
"""
import glob
import random
import struct
import sys

import serial


def detect_port():
    ports = sorted(glob.glob("/dev/ttyUSB*") + glob.glob("/dev/ttyACM*")) + sorted(glob.glob("/dev/ttyACM*"))
    for p in ports:
        try:
            s = serial.Serial(p, timeout=0.1)
            s.close()
            return p
        except Exception:
            continue
    return None


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else detect_port()
    if not port:
        print("no serial port found")
        sys.exit(1)
    print(f"port: {port}")
    ok = True
    for baud in (115200,):
        s = serial.Serial(port, baud, timeout=1)
        random.seed(0x1234)
        pattern = bytes(random.randrange(1, 256) for _ in range(256))
        pat = pattern
        s.reset_input_buffer()
        # small chunked writes: some adapters add latency or break long writes
        s.write(pat[:128])
        s.write(pat[128:])
        s.flush()
        r = s.read(len(pat))
        print(f"{baud}: sent {len(pat)} bytes, got {len(r)}")
        if r == pat:
            print(f"{baud}: PASS")
        else:
            ok = False
            mism = sum(1 for a, b in zip(pat, r) if a != b)
            print(f"{baud}: FAIL (mismatches: {mism}; got head: {r[:32].hex()})")
            print(f"       expected head: {pat[:32].hex()}")
        s.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
