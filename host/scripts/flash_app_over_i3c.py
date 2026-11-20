#!/usr/bin/env python3
"""Flash an application into BareBoot using only AgentLA's I3C REST API.

This is the I3C counterpart of ``i2c-bootloader/flash_app_over_i2c.py``.
Entry, framing and all Katapult commands are identical; only the transport
differs.  The controller performs ENTDAA once to assign a dynamic address,
then uses that address for every private transfer.  No nRST line, HID device
or debug probe is used.

Prerequisite: the target must be running the dynamic I3C bootloader and must
enter it before this script runs (raw 0xF0 write to I2C address 0x40, or the
boot window).  The controller cannot move a running app into the bootloader
over I3C alone because the app has no I3C target code.
"""
import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request

DEFAULT_BASE = "http://10.10.10.10"
DEFAULT_IMAGE = "/tmp/opencode/n20-flash-test.bin"

BASE = os.environ.get("BAREBOOT_REST_URL", DEFAULT_BASE)
DYNAMIC_ADDR = 0x40           # proposed dynamic address (target static addr)
CONNECT_RETRIES = 20
CONNECT_RETRY_DELAY = 0.15


def post(path, payload=None, retries=5):
    data = json.dumps(payload if payload is not None else {}).encode()
    req = urllib.request.Request(BASE + path, data=data,
                                 headers={"Content-Type": "application/json"},
                                 method="POST")
    for attempt in range(retries):
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                body = r.read().decode()
            return json.loads(body)
        except urllib.error.HTTPError as e:
            try:
                return json.loads(e.read().decode())
            except Exception:
                return {"err": "HTTP %d" % e.code}
        except Exception as e:
            if attempt < retries - 1:
                time.sleep(0.02 * (attempt + 1))
                continue
            return {"err": str(e)}


def crc16(data):
    # CRC16-CCITT (Katapult/BareBoot). Do not collapse the XOR dance into a
    # single statement: CPython 3.14 miscompiles the combined form.
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
    return (bytes([0x01, 0x88]) + core +
            bytes([c & 0xff, (c >> 8) & 0xff, 0x99, 0x03]))


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


class I3cTransport:
    def __init__(self, addr):
        self.addr = addr

    def write(self, hexstr):
        return post("/api/v1/bus/i3c/transfer",
                    {"addr": self.addr, "data_hex": hexstr, "read_len": 0})

    def read(self, n):
        return post("/api/v1/bus/i3c/transfer",
                    {"addr": self.addr, "data_hex": "", "read_len": n})

    def exchange(self, cmd, payload=b'', wait=0.05, rd=96):
        r = self.write(frame(cmd, payload).hex())
        if r.get("status") != "ok":
            return None, r
        time.sleep(wait)
        r = self.read(rd)
        raw = bytes.fromhex(r.get("rx_hex", "")) if r.get("rx_hex") else b''
        return parse(raw), r


def connect_once(t):
    return t.exchange(0x11, wait=0.08, rd=96)


def init_i3c(freq_khz=1000):
    post("/api/v1/bridge/i3c/on", {})
    r = post("/api/v1/bus/i3c/init", {"freq_khz": freq_khz})
    if r.get("status") != "ok":
        print("I3C init failed:", r)
        return False
    return True


def ensure_dynamic_address(addr):
    """Run ENTDAA once and use the assigned dynamic address."""
    r = post("/api/v1/bus/i3c/entdaa", {"addr": addr})
    assigned = r.get("assigned_addr", 0)
    print("ENTDAA:", r.get("status"), "assigned=0x%02x" % assigned,
          "uid=%s bcr=%s dcr=%s" % (r.get("uid_hex"), r.get("bcr"),
                                    r.get("dcr")))
    return assigned if assigned else addr


def main():
    global BASE
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("image", nargs="?", default=DEFAULT_IMAGE,
                    help="firmware image to flash")
    ap.add_argument("--base", default=os.environ.get("BAREBOOT_REST_URL",
                                                     DEFAULT_BASE),
                    help="AgentLA REST base URL (env BAREBOOT_REST_URL)")
    ap.add_argument("--addr", type=lambda x: int(x, 0), default=DYNAMIC_ADDR,
                    help="proposed I3C dynamic address (default %(default)s)")
    ap.add_argument("--freq", type=int, default=1000,
                    help="I3C SCL frequency in kHz (default %(default)s)")
    args = ap.parse_args()
    BASE = args.base.rstrip("/")

    if not init_i3c(args.freq):
        sys.exit(1)
    addr = ensure_dynamic_address(args.addr)
    t = I3cTransport(addr)

    p, r = None, None
    for attempt in range(1, CONNECT_RETRIES + 1):
        p, r = connect_once(t)
        if p and p["cmd"] == 0xA0:
            print("CONNECT OK (attempt %d)" % attempt)
            break
        time.sleep(CONNECT_RETRY_DELAY)
    else:
        print("CONNECT FAILED", p, r)
        sys.exit(1)

    pl = p["payload"]
    app_base = int.from_bytes(pl[8:12], "little")
    bs = int.from_bytes(pl[12:16], "little")
    mcu = pl[16:].split(b'\0')[0].decode(errors='replace')
    print("CONNECT OK: app_base=0x%08x block_size=%d mcu=%s"
          % (app_base, bs, mcu))

    img = open(args.image, "rb").read()
    print("image: %d bytes sha256=%s"
          % (len(img), hashlib.sha256(img).hexdigest()[:16]))
    nb = (len(img) + bs - 1) // bs
    t0 = time.time()
    fails = 0
    for i in range(nb):
        blk = img[i * bs:(i + 1) * bs]
        blk = blk + b'\xff' * (bs - len(blk))
        target_addr = app_base + i * bs
        payload = target_addr.to_bytes(4, "little") + blk
        wait = 0.30 if (i + 1) % 128 == 0 else 0.04
        p, r = t.exchange(0x12, payload, wait=wait, rd=32)
        if not p or p["cmd"] != 0xA0:
            print("BLOCK %d FAIL addr=0x%08x p=%s r=%s" % (i, target_addr, p, r))
            fails += 1
            if fails > 3:
                sys.exit(2)
        if (i + 1) % 200 == 0:
            print("  %d/%d blocks, %.1fs" % (i + 1, nb, time.time() - t0))
    print("blocks done in %.1fs, fails=%d" % (time.time() - t0, fails))
    if fails:
        sys.exit(3)

    p, r = t.exchange(0x13, wait=0.30, rd=32)
    print("RX_EOF:", p and hex(p["cmd"]))
    if not p or p["cmd"] != 0xA0:
        sys.exit(4)

    print("=== I3C readback verification ===")
    checks = [0, 1, nb // 2, nb - 2, nb - 1]
    ok = True
    for bi in checks:
        target_addr = app_base + bi * bs
        p, r = t.exchange(0x14, target_addr.to_bytes(4, "little"),
                          wait=0.08, rd=128)
        if not p or p["cmd"] != 0xA0 or len(p["payload"]) < 8 + bs:
            print("REQ %d FAIL" % bi, p)
            ok = False
            continue
        got = p["payload"][8:8 + bs]
        exp = img[bi * bs:bi * bs + bs]
        exp = exp + b'\xff' * (bs - len(exp))
        match = got == exp
        print("REQ block %d @0x%08x: %s"
              % (bi, target_addr, "MATCH" if match else "MISMATCH"))
        if not match:
            print("  got:", got[:16].hex(), "exp:", exp[:16].hex())
            ok = False
    if not ok:
        sys.exit(5)

    p, r = t.exchange(0x15, wait=0.08, rd=32)
    print("COMPLETE:", p and hex(p["cmd"]))
    print("FLASH+VERIFY SUCCESS")


if __name__ == "__main__":
    main()
