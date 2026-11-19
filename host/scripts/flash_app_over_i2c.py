#!/usr/bin/env python3
"""Flash an application into BareBoot using only AgentLA's I2C REST API.

Entry is fully in-band.  A CONNECT is attempted first; if the target is
already in the bootloader (for example during the boot window) the flash
proceeds immediately.  If CONNECT fails, a raw single-byte write of 0xF0 is
sent to I2C address 0x40, which makes the application firmware reboot into
BareBoot; CONNECT is then retried until the reset completes.  No nRST line,
HID device, or debug probe is used.
"""
import argparse
import hashlib
import json
import os
import sys
import time
import urllib.request

DEFAULT_BASE = "http://10.10.10.10"
DEFAULT_IMAGE = "/tmp/opencode/n20-flash-test.bin"

BASE = os.environ.get("BAREBOOT_REST_URL", DEFAULT_BASE)
IMG_PATH = DEFAULT_IMAGE

# In-band entry contract implemented by the application firmware.
ENTRY_ADDR = 0x40
ENTRY_BYTE = "F0"
ENTRY_RESET_DELAY = 0.3
CONNECT_RETRIES = 20
CONNECT_RETRY_DELAY = 0.15


def post(path, payload, retries=5):
    req = urllib.request.Request(BASE + path,
                                 data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"},
                                 method="POST")
    for attempt in range(retries):
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                return json.loads(r.read().decode())
        except Exception as e:
            if attempt < retries - 1:
                time.sleep(0.015 * (attempt + 1))
                continue
            return {"err": str(e)}


def crc16(data):
    # CRC16-CCITT (Katapult/BareBoot). Compute without the
    # `v = d ^ (c & 0xff); v ^= (v << 4) & 0xff` pattern, which CPython 3.14
    # miscompiles via a combined STORE_FAST_LOAD_FAST operation.
    crc = 0xffff
    for byte in data:
        tmp = byte ^ (crc & 0xff)
        tmp = (tmp ^ ((tmp << 4) & 0xff)) & 0xff
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp >> 4) ^ (tmp << 3)) & 0xffff
    return crc

def frame(cmd, payload=b''):
    assert len(payload) % 4 == 0
    wc = len(payload) // 4
    core = bytes([cmd, wc]) + payload
    c = crc16(core)
    return bytes([0x01, 0x88]) + core + bytes([c & 0xff, (c >> 8) & 0xff,
                                               0x99, 0x03])


def parse(data):
    for i in range(len(data) - 1):
        if data[i] == 0x01 and data[i + 1] == 0x88 and len(data) - i >= 8:
            cmd = data[i + 2]
            wc = data[i + 3]
            fl = 4 + wc * 4 + 4
            if len(data) - i < fl:
                continue
            cand = data[i:i + fl]
            if cand[-2] != 0x99 or cand[-1] != 0x03:
                continue
            if crc16(cand[2:4 + wc * 4]) != (cand[-4] | (cand[-3] << 8)):
                continue
            return {"cmd": cand[2], "wc": wc,
                    "payload": cand[4:4 + wc * 4]}
    return None


def xfer_write(hexstr):
    return post("/api/v1/bus/i2c/transfer",
                {"addr": 0x40, "data_hex": hexstr, "read_len": 0})


def xfer_read(n=32):
    return post("/api/v1/bus/i2c/transfer",
                {"addr": 0x40, "data_hex": "", "read_len": n})


def exchange(cmd, payload=b'', wait=0.03, rd=32):
    r = xfer_write(frame(cmd, payload).hex())
    if r.get("status") != "ok":
        return None, r
    time.sleep(wait)
    r = xfer_read(rd)
    p = parse(bytes.fromhex(r.get("rx_hex", ""))) if r.get("rx_hex") else None
    return p, r


def connect_once():
    """One CONNECT attempt. Returns (parsed_frame, raw_response)."""
    return exchange(0x11, wait=0.05, rd=128)


def ensure_bootloader():
    """Return the parsed CONNECT response, entering BareBoot in-band if needed."""
    p, r = connect_once()
    if p and p["cmd"] == 0xA0:
        print("CONNECT OK (target already in bootloader)")
        return p

    print("CONNECT unavailable; sending in-band entry strobe "
          "raw %s to 0x%02X" % (ENTRY_BYTE, ENTRY_ADDR))
    sr = xfer_write(ENTRY_BYTE)
    if sr.get("status") != "ok":
        print("WARNING: F0 strobe transfer did not report ok:", sr)
    time.sleep(ENTRY_RESET_DELAY)

    # Reset timing varies: poll CONNECT for a bounded window after the strobe.
    for attempt in range(1, CONNECT_RETRIES + 1):
        p, r = connect_once()
        if p and p["cmd"] == 0xA0:
            print("CONNECT OK after F0 strobe (attempt %d)" % attempt)
            return p
        time.sleep(CONNECT_RETRY_DELAY)

    print("CONNECT FAILED after in-band entry strobe", p, r)
    sys.exit(1)


def main():
    global BASE, IMG_PATH
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("image", nargs="?", default=DEFAULT_IMAGE,
                    help="firmware image to flash")
    ap.add_argument("--base", default=os.environ.get("BAREBOOT_REST_URL",
                                                     DEFAULT_BASE),
                    help="AgentLA REST base URL (env BAREBOOT_REST_URL; "
                         "default %(default)s)")
    args = ap.parse_args()
    BASE = args.base.rstrip("/")
    IMG_PATH = args.image

    # In-band entry: CONNECT first, F0 strobe to 0x40 and retry if needed.
    p = ensure_bootloader()
    pl = p["payload"]
    app_base = int.from_bytes(pl[8:12], "little")
    bs = int.from_bytes(pl[12:16], "little")
    mcu = pl[16:].split(b'\0')[0].decode(errors='replace')
    print("CONNECT OK: app_base=0x%08x block_size=%d mcu=%s"
          % (app_base, bs, mcu))

    img = open(IMG_PATH, "rb").read()
    print("image: %d bytes sha256=%s"
          % (len(img), hashlib.sha256(img).hexdigest()[:16]))
    nb = (len(img) + bs - 1) // bs
    t0 = time.time()
    fails = 0
    for i in range(nb):
        blk = img[i * bs:(i + 1) * bs]
        blk = blk + b'\xff' * (bs - len(blk))
        addr = app_base + i * bs
        payload = addr.to_bytes(4, "little") + blk
        # page flush every 128 blocks; allow extra time
        wait = 0.30 if (i + 1) % 128 == 0 else 0.02
        p, r = exchange(0x12, payload, wait=wait, rd=24)
        if not p or p["cmd"] != 0xA0:
            print("BLOCK %d FAIL addr=0x%08x p=%s r=%s" % (i, addr, p, r))
            fails += 1
            if fails > 3:
                sys.exit(2)
        if (i + 1) % 200 == 0:
            print("  %d/%d blocks, %.1fs" % (i + 1, nb, time.time() - t0))
    print("blocks done in %.1fs, fails=%d" % (time.time() - t0, fails))
    if fails:
        sys.exit(3)

    # RX_EOF (flushes final partial page -> allow extra time)
    p, r = exchange(0x13, wait=0.30, rd=24)
    print("RX_EOF:", p and hex(p["cmd"]), p and p["payload"].hex())
    if not p or p["cmd"] != 0xA0:
        sys.exit(4)

    # Readback verification via REQ_BLOCK at several offsets
    print("=== I2C readback verification ===")
    checks = [0, 1, nb // 2, nb - 2, nb - 1]
    ok = True
    for bi in checks:
        addr = app_base + bi * bs
        p, r = exchange(0x14, addr.to_bytes(4, "little"),
                        wait=0.05, rd=96)
        if not p or p["cmd"] != 0xA0 or len(p["payload"]) < 8 + bs:
            print("REQ %d FAIL" % bi, p)
            ok = False
            continue
        got = p["payload"][8:8 + bs]
        exp = img[bi * bs:bi * bs + bs]
        exp = exp + b'\xff' * (bs - len(exp))
        match = got == exp
        print("REQ block %d @0x%08x: %s"
              % (bi, addr, "MATCH" if match else "MISMATCH"))
        if not match:
            print("  got:", got[:16].hex(), "exp:", exp[:16].hex())
            ok = False
    if not ok:
        sys.exit(5)

    # COMPLETE -> jump to app
    p, r = exchange(0x15, wait=0.05, rd=24)
    print("COMPLETE:", p and hex(p["cmd"]))
    print("FLASH+VERIFY SUCCESS")


if __name__ == "__main__":
    main()
