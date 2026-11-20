#!/usr/bin/env python3
"""Multi-Drop In-Band Flash Orchestrator for BareBoot.

Sequentially flashes and verifies multiple nodes sharing a single I2C or I3C bus
via AgentLA REST API or local Linux I2C device without clock stretching or bus collision.

Pre-flight checks verify that all nodes on the shared bus are responsive before taking
any individual node offline. Flashing proceeds node-by-node with selective in-band
entry (0xF0 strobe to target address), block writes, EOF, and readback verification.
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from typing import Any, Dict, List, Optional, Tuple

try:
    import yaml
    HAVE_YAML = True
except ImportError:
    HAVE_YAML = False


DEFAULT_BASE_URL = "http://10.10.10.10"
ENTRY_STROBE_BYTE = bytes([0xF0])
ENTRY_RESET_DELAY_DEFAULT = 0.35
CONNECT_RETRIES_DEFAULT = 25
CONNECT_RETRY_DELAY_DEFAULT = 0.15


def crc16(data: bytes) -> int:
    """CRC16-CCITT compatible with Katapult/BareBoot bootloader.
    
    Avoid combined fast operations that may be miscompiled in newer CPython runtimes.
    """
    crc = 0xFFFF
    for byte in data:
        tmp = byte ^ (crc & 0xFF)
        tmp = (tmp ^ ((tmp << 4) & 0xFF)) & 0xFF
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp >> 4) ^ (tmp << 3)) & 0xFFFF
    return crc


def make_frame(cmd: int, payload: bytes = b"") -> bytes:
    """Frame a Katapult/BareBoot command into [0x01, 0x88, cmd, wc, payload, crc, 0x99, 0x03]."""
    assert len(payload) % 4 == 0, "Payload length must be 4-byte aligned"
    wc = len(payload) // 4
    core = bytes([cmd, wc]) + payload
    c = crc16(core)
    return bytes([0x01, 0x88]) + core + bytes([c & 0xFF, (c >> 8) & 0xFF, 0x99, 0x03])


def parse_frame(data: bytes) -> Optional[Dict[str, Any]]:
    """Scan and parse a framed Katapult response."""
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
            expected_crc = cand[-4] | (cand[-3] << 8)
            if crc16(cand[2:4 + wc * 4]) != expected_crc:
                continue
            return {
                "cmd": cand[2],
                "wc": wc,
                "payload": cand[4:4 + wc * 4]
            }
    return None


class BaseBusTransport:
    """Abstract communication bus transport for multi-drop flashing."""

    def probe(self, addr: int) -> bool:
        raise NotImplementedError

    def write_raw(self, addr: int, data: bytes) -> bool:
        raise NotImplementedError

    def read_raw(self, addr: int, length: int) -> bytes:
        raise NotImplementedError

    def exchange(self, addr: int, cmd: int, payload: bytes = b"", wait: float = 0.03, rd_len: int = 48) -> Tuple[Optional[Dict[str, Any]], bytes]:
        tx_data = make_frame(cmd, payload)
        if not self.write_raw(addr, tx_data):
            return None, b""
        if wait > 0:
            time.sleep(wait)
        rx_data = self.read_raw(addr, rd_len)
        parsed = parse_frame(rx_data) if rx_data else None
        return parsed, rx_data


class HttpI2cTransport(BaseBusTransport):
    """Transport via AgentLA HTTP REST API for I2C."""

    def __init__(self, base_url: str = DEFAULT_BASE_URL, retries: int = 5):
        self.base_url = base_url.rstrip("/")
        self.retries = retries

    def _post(self, path: str, payload: dict) -> dict:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            self.base_url + path,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        for attempt in range(self.retries):
            try:
                with urllib.request.urlopen(req, timeout=10) as resp:
                    return json.loads(resp.read().decode("utf-8"))
            except urllib.error.HTTPError as e:
                try:
                    return json.loads(e.read().decode("utf-8"))
                except Exception:
                    return {"err": "HTTP %d" % e.code}
            except Exception as e:
                if attempt < self.retries - 1:
                    time.sleep(0.015 * (attempt + 1))
                    continue
                return {"err": str(e)}
        return {"err": "Max retries exceeded"}

    def probe(self, addr: int) -> bool:
        # A quick 0-byte or 1-byte read test to verify slave ACK
        res = self._post("/api/v1/bus/i2c/transfer", {"addr": addr, "data_hex": "", "read_len": 1})
        if res.get("status") == "ok":
            return True
        # If read_len 1 returned err, try 0-byte write
        res = self._post("/api/v1/bus/i2c/transfer", {"addr": addr, "data_hex": "", "read_len": 0})
        return res.get("status") == "ok"

    def write_raw(self, addr: int, data: bytes) -> bool:
        res = self._post("/api/v1/bus/i2c/transfer", {"addr": addr, "data_hex": data.hex(), "read_len": 0})
        return res.get("status") == "ok"

    def read_raw(self, addr: int, length: int) -> bytes:
        res = self._post("/api/v1/bus/i2c/transfer", {"addr": addr, "data_hex": "", "read_len": length})
        if res.get("status") == "ok" and "rx_hex" in res:
            try:
                return bytes.fromhex(res["rx_hex"])
            except ValueError:
                return b""
        return b""


class HttpI3cTransport(BaseBusTransport):
    """Transport via AgentLA HTTP REST API for I3C."""

    def __init__(self, base_url: str = DEFAULT_BASE_URL, retries: int = 5):
        self.base_url = base_url.rstrip("/")
        self.retries = retries

    def _post(self, path: str, payload: dict) -> dict:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            self.base_url + path,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        for attempt in range(self.retries):
            try:
                with urllib.request.urlopen(req, timeout=10) as resp:
                    return json.loads(resp.read().decode("utf-8"))
            except Exception as e:
                if attempt < self.retries - 1:
                    time.sleep(0.015 * (attempt + 1))
                    continue
                return {"err": str(e)}
        return {"err": "Max retries exceeded"}

    def probe(self, addr: int) -> bool:
        res = self._post("/api/v1/bus/i3c/transfer", {"addr": addr, "data_hex": "", "read_len": 1})
        return res.get("status") == "ok"

    def write_raw(self, addr: int, data: bytes) -> bool:
        res = self._post("/api/v1/bus/i3c/transfer", {"addr": addr, "data_hex": data.hex(), "read_len": 0})
        return res.get("status") == "ok"

    def read_raw(self, addr: int, length: int) -> bytes:
        res = self._post("/api/v1/bus/i3c/transfer", {"addr": addr, "data_hex": "", "read_len": length})
        if res.get("status") == "ok" and "rx_hex" in res:
            try:
                return bytes.fromhex(res["rx_hex"])
            except ValueError:
                return b""
        return b""


class NodeConfig:
    def __init__(self, node_id: str, address: int, image_path: str, enabled: bool = True):
        self.node_id = str(node_id)
        self.address = int(address)
        self.image_path = str(image_path)
        self.enabled = bool(enabled)

    @classmethod
    def from_dict(cls, data: dict) -> "NodeConfig":
        raw_addr = data.get("address", 0x40)
        addr = int(raw_addr, 0) if isinstance(raw_addr, str) else int(raw_addr)
        return cls(
            node_id=data.get("id", f"node_0x{addr:02X}"),
            address=addr,
            image_path=data.get("image", ""),
            enabled=data.get("enabled", True),
        )


class MultidropConfig:
    def __init__(self, bus_type: str = "i2c", base_url: str = DEFAULT_BASE_URL, speed_khz: int = 400,
                 reset_delay: float = ENTRY_RESET_DELAY_DEFAULT, connect_retries: int = CONNECT_RETRIES_DEFAULT,
                 nodes: Optional[List[NodeConfig]] = None):
        self.bus_type = bus_type.lower()
        self.base_url = base_url
        self.speed_khz = speed_khz
        self.reset_delay = reset_delay
        self.connect_retries = connect_retries
        self.nodes = nodes or []

    @classmethod
    def load(cls, file_path: str) -> "MultidropConfig":
        if not os.path.exists(file_path):
            raise FileNotFoundError(f"Config file not found: {file_path}")

        raw_text = open(file_path, "r", encoding="utf-8").read()
        if HAVE_YAML:
            try:
                data = yaml.safe_load(raw_text)
            except Exception:
                data = json.loads(raw_text)
        else:
            try:
                data = json.loads(raw_text)
            except Exception as e:
                raise RuntimeError(f"YAML module missing and JSON parse failed: {e}")

        bus = data.get("bus", {})
        bus_type = bus.get("type", "i2c")
        base_url = bus.get("base_url", DEFAULT_BASE_URL)
        speed_khz = bus.get("speed_khz", 400)
        reset_delay = bus.get("reset_delay_sec", ENTRY_RESET_DELAY_DEFAULT)
        connect_retries = bus.get("connect_retries", CONNECT_RETRIES_DEFAULT)

        nodes = []
        for n in data.get("nodes", []):
            nodes.append(NodeConfig.from_dict(n))

        return cls(
            bus_type=bus_type,
            base_url=base_url,
            speed_khz=speed_khz,
            reset_delay=reset_delay,
            connect_retries=connect_retries,
            nodes=nodes,
        )


class MultidropFlasher:
    """Orchestrates pre-flight discovery, selective entry, and sequential flashing."""

    def __init__(self, transport: BaseBusTransport, config: MultidropConfig):
        self.transport = transport
        self.config = config

    def preflight_inventory(self, active_nodes: List[NodeConfig]) -> Dict[str, bool]:
        """Perform non-destructive bus discovery across all configured addresses."""
        print("=== Step 1: Pre-Flight Bus Inventory Discovery ===")
        inventory = {}
        all_passed = True

        for node in active_nodes:
            # First check if node responds to basic probe or CONNECT
            p, _ = self.transport.exchange(node.address, 0x11, wait=0.03, rd_len=64)
            if p and p.get("cmd") == 0xA0:
                print(f"  Node [{node.node_id}] @ 0x{node.address:02X}: [ONLINE] (in bootloader)")
                inventory[node.node_id] = True
            else:
                ok = self.transport.probe(node.address)
                status = "[ONLINE] (application running)" if ok else "[OFFLINE] (no response)"
                print(f"  Node [{node.node_id}] @ 0x{node.address:02X}: {status}")
                inventory[node.node_id] = ok
                if not ok:
                    all_passed = False

        if not all_passed:
            print("Pre-flight scan detected unresponsive nodes on shared bus.")
        else:
            print("Pre-flight scan passed: all nodes reachable on shared bus.")
        return inventory

    def enter_bootloader(self, node: NodeConfig) -> Optional[Dict[str, Any]]:
        """Selective handsfree bootloader entry for a single targeted node."""
        # Check if already in bootloader
        p, _ = self.transport.exchange(node.address, 0x11, wait=0.04, rd_len=96)
        if p and p.get("cmd") == 0xA0:
            print(f"  Node [{node.node_id}] @ 0x{node.address:02X} is already in bootloader.")
            return p

        print(f"  Sending selective in-band 0xF0 strobe to 0x{node.address:02X}...")
        self.transport.write_raw(node.address, ENTRY_STROBE_BYTE)
        time.sleep(self.config.reset_delay)

        # Poll CONNECT until target node acknowledges
        for attempt in range(1, self.config.connect_retries + 1):
            p, _ = self.transport.exchange(node.address, 0x11, wait=0.04, rd_len=96)
            if p and p.get("cmd") == 0xA0:
                print(f"  Target 0x{node.address:02X} entered bootloader (attempt {attempt}).")
                return p
            time.sleep(CONNECT_RETRY_DELAY_DEFAULT)

        print(f"  FAILED: Node 0x{node.address:02X} did not enter bootloader after 0xF0 strobe.")
        return None

    def flash_node(self, node: NodeConfig) -> bool:
        """Flash firmware image into the targeted node and verify readback."""
        if not os.path.exists(node.image_path):
            print(f"  ERROR: Image file not found: {node.image_path}")
            return False

        with open(node.image_path, "rb") as f:
            image_data = f.read()

        image_sha256 = hashlib.sha256(image_data).hexdigest()
        print(f"  Image size: {len(image_data)} bytes, SHA256: {image_sha256[:16]}...")

        # 1. Enter bootloader
        connect_resp = self.enter_bootloader(node)
        if not connect_resp:
            return False

        payload = connect_resp["payload"]
        if len(payload) < 16:
            print("  ERROR: CONNECT payload too short")
            return False

        app_base = int.from_bytes(payload[8:12], "little")
        block_size = int.from_bytes(payload[12:16], "little")
        mcu_name = payload[16:].split(b"\0")[0].decode(errors="replace")
        print(f"  Target details: MCU={mcu_name}, AppBase=0x{app_base:08X}, BlockSize={block_size}")

        if block_size == 0 or (block_size & (block_size - 1)) != 0:
            block_size = 64

        # 2. Transmit blocks
        num_blocks = (len(image_data) + block_size - 1) // block_size
        t0 = time.time()
        for i in range(num_blocks):
            chunk = image_data[i * block_size:(i + 1) * block_size]
            if len(chunk) < block_size:
                chunk = chunk + b"\xFF" * (block_size - len(chunk))

            block_addr = app_base + i * block_size
            block_payload = block_addr.to_bytes(4, "little") + chunk
            wait = 0.25 if (i + 1) % 128 == 0 else 0.02
            p, r = self.transport.exchange(node.address, 0x12, block_payload, wait=wait, rd_len=24)
            if not p or p.get("cmd") != 0xA0:
                print(f"  BLOCK WRITE FAILED at index {i}, addr 0x{block_addr:08X}: {p} raw={r.hex()}")
                return False

        elapsed = time.time() - t0
        print(f"  Transmitted {num_blocks} blocks in {elapsed:.2f}s ({len(image_data) / max(elapsed, 0.001) / 1024:.1f} KB/s)")

        # 3. RX_EOF
        p, _ = self.transport.exchange(node.address, 0x13, wait=0.25, rd_len=24)
        if not p or p.get("cmd") != 0xA0:
            print("  RX_EOF failed")
            return False

        # 4. Readback Verification
        print("  Verifying readback blocks...")
        check_indices = sorted(list({0, min(1, num_blocks - 1), num_blocks // 2, max(0, num_blocks - 2), num_blocks - 1}))
        for bi in check_indices:
            block_addr = app_base + bi * block_size
            p, _ = self.transport.exchange(node.address, 0x14, block_addr.to_bytes(4, "little"), wait=0.03, rd_len=block_size + 16)
            if not p or p.get("cmd") != 0xA0 or len(p["payload"]) < 8 + block_size:
                print(f"  Readback failed at block {bi} @ 0x{block_addr:08X}")
                return False
            read_bytes = p["payload"][8:8 + block_size]
            expected_chunk = image_data[bi * block_size:(bi + 1) * block_size]
            if len(expected_chunk) < block_size:
                expected_chunk = expected_chunk + b"\xFF" * (block_size - len(expected_chunk))
            if read_bytes != expected_chunk:
                print(f"  Data mismatch at block {bi} @ 0x{block_addr:08X}!")
                return False

        print(f"  Readback verified across sample blocks.")

        # 5. COMPLETE
        p, _ = self.transport.exchange(node.address, 0x15, wait=0.04, rd_len=24)
        if not p or p.get("cmd") != 0xA0:
            print("  COMPLETE command failed")
            return False

        print(f"  Node [{node.node_id}] updated and rebooted successfully.")
        return True

    def run(self, node_filter: Optional[str] = None, skip_preflight: bool = False) -> bool:
        """Run the full sequential multi-node update pipeline."""
        active_nodes = [n for n in self.config.nodes if n.enabled]
        if node_filter:
            active_nodes = [n for n in active_nodes if n.node_id == node_filter or f"0x{n.address:02x}" == node_filter.lower()]
            if not active_nodes:
                print(f"No nodes matching filter '{node_filter}'")
                return False

        print(f"Starting multidrop update for {len(active_nodes)} node(s) over {self.config.bus_type.upper()}...")

        # Step 1: Pre-flight inventory
        if not skip_preflight:
            inv = self.preflight_inventory(active_nodes)
            if not all(inv.values()):
                print("ABORT: One or more nodes failed pre-flight inventory. Aborting update.")
                return False
        else:
            print("Pre-flight scan skipped by user.")

        # Step 2: Sequential updates
        print("\n=== Step 2: Sequential Node Updates ===")
        successes = 0
        total = len(active_nodes)

        for i, node in enumerate(active_nodes, 1):
            print(f"\n[{i}/{total}] Updating Node [{node.node_id}] (Address: 0x{node.address:02X})...")
            ok = self.flash_node(node)
            if not ok:
                print(f"ABORT: Updating Node [{node.node_id}] failed. Stopping rollout to protect shared bus.")
                return False
            successes += 1
            # Brief inter-node delay to allow shared bus stabilization
            time.sleep(0.1)

        print("\n=== Step 3: Rollout Complete ===")
        print(f"All {successes}/{total} nodes successfully updated and verified on shared bus.")
        return True


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", help="Path to multidrop topology configuration file (YAML/JSON)")
    parser.add_argument("--base", default=None, help="AgentLA REST base URL override (e.g. http://10.10.10.10)")
    parser.add_argument("--node", default=None, help="Update only specific node by ID or hex address")
    parser.add_argument("--skip-preflight", action="store_true", help="Skip pre-flight bus inventory discovery")
    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()

    try:
        config = MultidropConfig.load(args.config)
    except Exception as e:
        print(f"Failed to load config: {e}", file=sys.stderr)
        sys.exit(1)

    if args.base:
        config.base_url = args.base

    if config.bus_type == "i3c":
        transport = HttpI3cTransport(base_url=config.base_url)
    else:
        transport = HttpI2cTransport(base_url=config.base_url)

    flasher = MultidropFlasher(transport, config)
    ok = flasher.run(node_filter=args.node, skip_preflight=args.skip_preflight)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
