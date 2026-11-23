#!/usr/bin/env python3
# Script to upload software via Katapult
#
# Copyright (C) 2022 Eric Callahan <arksine.code@gmail.com>
#
# This file may be distributed under the terms of the GNU GPLv3 license.
from __future__ import annotations
import sys
import os
import termios
import fcntl
import zlib
import json
import asyncio
import socket
import struct
import logging
import errno
import argparse
import hashlib
import pathlib
import shutil
import shlex
import contextlib
from typing import Dict, List, Optional, Union, Any
HAS_SERIAL = True
try:
    from serial import Serial, SerialException
except ModuleNotFoundError:
    HAS_SERIAL = False
    SerialException = Exception

HAS_PYUSB = True
try:
    import usb.core
    import usb.util
except ModuleNotFoundError:
    HAS_PYUSB = False

def output_line(msg: str) -> None:
    sys.stdout.write(msg + "\n")
    sys.stdout.flush()

def output(msg: str) -> None:
    sys.stdout.write(msg)
    sys.stdout.flush()

# Standard crc16 ccitt, take from msgproto.py in Klipper
def crc16_ccitt(buf: Union[bytes, bytearray]) -> int:
    crc = 0xffff
    for data in buf:
        data ^= crc & 0xff
        data ^= (data & 0x0f) << 4
        crc = ((data << 8) | (crc >> 8)) ^ (data >> 4) ^ (data << 3)
    return crc & 0xFFFF


logging.basicConfig(level=logging.INFO)
CAN_FMT = "<IB3x8s"
CAN_READER_LIMIT = 1024 * 1024

# Katapult Defs
CMD_HEADER = b'\x01\x88'
CMD_TRAILER = b'\x99\x03'
BOOTLOADER_CMDS = {
    'CONNECT': 0x11,
    'SEND_BLOCK': 0x12,
    'SEND_EOF': 0x13,
    'REQUEST_BLOCK': 0x14,
    'COMPLETE': 0x15,
    'GET_CANBUS_ID': 0x16
}

ACK_SUCCESS = 0xa0
NACK = 0xf1
ACK_ERROR = 0xf2
ACK_BUSY = 0xf3

# Klipper Admin Defs (for jumping to bootloader)
KLIPPER_ADMIN_ID = 0x3f0
KLIPPER_SET_NODE_CMD = 0x01
KLIPPER_REBOOT_CMD = 0x02

# CAN Admin Defs
CANBUS_ID_ADMIN = 0x3f0
CANBUS_ID_ADMIN_RESP = 0x3f1
CANBUS_CMD_QUERY_UNASSIGNED = 0x00
CANBUS_CMD_SET_NODEID = 0x11
CANBUS_CMD_CLEAR_NODE_ID = 0x12
CANBUS_RESP_NEED_NODEID = 0x20
CANBUS_NODEID_OFFSET = 128

# USB IDs
KATAPULT_USB_ID = "1d50:6177"
KLIPPER_USB_ID = "1d50:614e"
GS_CAN_USB_ID = "1d50:606f"
SERIAL_BL_REQ = b"~ \x1c Request Serial Bootloader!! ~"

class FlashError(Exception):
    pass

def get_usb_info(usb_path: pathlib.Path) -> Dict[str, Any]:
    usb_info: Dict[str, Any] = {}
    id_path = usb_path.joinpath("idVendor")
    prod_id_path = usb_path.joinpath("idProduct")
    mfr_path = usb_path.joinpath("manufacturer")
    prod_path = usb_path.joinpath("product")
    serial_path = usb_path.joinpath("serial")
    usb_info["usb_id"] = ""
    if id_path.is_file() and prod_id_path.is_file():
        vid = id_path.read_text().strip().lower()
        pid = prod_id_path.read_text().strip().lower()
        usb_info["usb_id"] = f"{vid}:{pid}"
    usb_info["manufacturer"] = "unknown"
    usb_info["product"] = "unknown"
    usb_info["serial_number"] = ""
    if mfr_path.is_file():
        usb_info["manufacturer"] = mfr_path.read_text().strip().lower()
    if prod_path.is_file():
        usb_info["product"] = prod_path.read_text().strip().lower()
    if serial_path.is_file():
        usb_info["serial_number"] = serial_path.read_text().strip().lower()
    return usb_info

def get_usb_path(device: pathlib.Path) -> Optional[pathlib.Path]:
    device_path = device.resolve()
    if not device_path.exists():
        return None
    sys_dev_path = pathlib.Path("/sys/class/tty").joinpath(device_path.name)
    if not sys_dev_path.exists():
        return None
    sys_dev_path = sys_dev_path.resolve()
    for usb_path in sys_dev_path.parents:
        dnum_file = usb_path.joinpath("devnum")
        bnum_file = usb_path.joinpath("busnum")
        if dnum_file.is_file() and bnum_file.is_file():
            return usb_path
    return None

def get_stable_usb_symlink(device: pathlib.Path) -> pathlib.Path:
    device_path = device.resolve()
    ser_by_path_dir = pathlib.Path("/dev/serial/by-path")
    try:
        if ser_by_path_dir.exists():
            for item in ser_by_path_dir.iterdir():
                if item.samefile(device_path):
                    return item
    except OSError:
        pass
    return device_path


#  Python Port of fasthash6
#  Host URL: http://github.com/ztanml/fast-hash
#  Original Copyright:
#  Copyright (C) 2012 Zilong Tan (eric.zltan@gmail.com)
#
#  MIT License
#
#  Permission is hereby granted, free of charge, to any person
#  obtaining a copy of this software and associated documentation
#  files (the "Software"), to deal in the Software without
#  restriction, including without limitation the rights to use, copy,
#  modify, merge, publish, distribute, sublicense, and/or sell copies
#  of the Software, and to permit persons to whom the Software is
#  furnished to do so, subject to the following conditions:
#
#  The above copyright notice and this permission notice shall be
#  included in all copies or substantial portions of the Software.
#
#  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
#  EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
#  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
#  NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
#  BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
#  ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
#  CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
#  SOFTWARE.
MASK_64 = 0xFFFFFFFFFFFFFFFF

def mix(val: int) -> int:
    val ^= val >> 23
    val = (val * 0x2127599bf4325c37) & MASK_64
    val ^= val >> 47
    return val

def fasthash64(buffer: bytes, seed: int) -> int:
    m = 0x880355f21e6d1965
    h = seed ^ ((len(buffer) * m) & MASK_64)
    long_count = len(buffer) // 8
    remaining: bytes = buffer
    if long_count:
        byte_count = long_count * 8
        buf = struct.unpack(f"<{long_count}Q", buffer[:byte_count])
        for v in buf:
            h ^= mix(v)
            h = (h * m) & MASK_64
        remaining = buffer[byte_count:]
    if remaining:
        v = 0
        print(remaining)
        for i in range(len(remaining), 0, -1):
            v ^= remaining[i -1] << (8 * (i - 1))
        h ^= mix(v)
        h = (h * m) & MASK_64
    return mix(h)

def convert_usbsn_to_uuid(serial_number: str) -> int:
    seed = 0xA16231A7
    chip_id = bytes.fromhex(serial_number)
    result = fasthash64(chip_id, seed)
    swapped = 0
    # The UUID returned by Klipper firmware is byteswapped
    for i in range(6):
        swapped |= ((result >> (i * 8)) & 0xFF) << ((5 - i) * 8)
    return swapped

class CanFlasher:
    def __init__(
        self,
        node: CanNode,
        fw_file: pathlib.Path
    ) -> None:
        self.node = node
        self.firmware_path = fw_file
        self.fw_sha = hashlib.sha1()
        self.primed = False
        self.file_size = 0
        self.block_size = 64
        self.block_count = 0
        self.app_start_addr = 0
        self.klipper_dict: Optional[Dict[str, Any]] = None
        self._check_binary()

    def _check_binary(self) -> None:
        """
        Extract klipper.dict from binary
        """
        fw_name = self.firmware_path.name.lower()
        if fw_name != "klipper.bin" or not self.firmware_path.is_file():
            return
        bin_data = self.firmware_path.read_bytes()
        klipper_dict: Dict[str, Any] = {}
        for idx in range(len(bin_data)):
            try:
                uncmp_data = zlib.decompress(bin_data[idx:])
                klipper_dict = json.loads(uncmp_data)
            except (zlib.error, json.JSONDecodeError):
                continue
            if klipper_dict.get("app") == "Klipper":
                break
        if klipper_dict:
            self.klipper_dict = klipper_dict
            ver = klipper_dict.get("version", "")
            bin_mcu = self.klipper_dict.get("config", {}).get("MCU", "")
            output_line(
                f"Detected Klipper binary version {ver}, MCU: {bin_mcu}"
            )

    def _build_command(self, cmd: int, payload: bytes) -> bytearray:
        word_cnt = (len(payload) // 4) & 0xFF
        out_cmd = bytearray(CMD_HEADER)
        out_cmd.append(cmd)
        out_cmd.append(word_cnt)
        if payload:
            out_cmd.extend(payload)
        crc = crc16_ccitt(out_cmd[2:])
        out_cmd.extend(struct.pack("<H", crc))
        out_cmd.extend(CMD_TRAILER)
        return out_cmd

    def prime(self) -> None:
        # Prime with an invalid command.  This will generate an error
        # and force double buffered USB devices to respond after the
        # first command is sent.
        msg = self._build_command(0x90, b"")
        self.node.write(msg)
        self.primed = True

    async def connect_btl(self) -> None:
        output_line("Attempting to connect to bootloader")
        ret = await self.send_command('CONNECT', tries=300, read_timeout=0.08)
        pinfo = ret[:12]
        mcu_info = ret[12:]
        ver_bytes: bytes
        ver_bytes, start_addr, self.block_size = struct.unpack("<4sII", pinfo)
        self.app_start_addr = start_addr
        self.software_version = "?"
        self.proto_version = tuple([v for v in reversed(ver_bytes[:3])])
        proto_version_str = ".".join([str(v) for v in self.proto_version])
        if self.block_size not in [64, 128, 256, 512]:
            raise FlashError("Invalid Block Size: %d" % (self.block_size,))
        mcu_info = mcu_info.rstrip(b"\x00")
        if self.proto_version >= (1, 1, 0):
            build_info = mcu_info.split(b"\x00", maxsplit=1)
            mcu_type = build_info[0].decode()
            if len(build_info) == 2:
                self.software_version = build_info[1].decode()
            else:
                output_line("Katapult build not reporting software version!")
        else:
            mcu_type = mcu_info.decode()
        output_line(
            f"Katapult Connected\n"
            f"Software Version: {self.software_version}\n"
            f"Protocol Version: {proto_version_str}\n"
            f"Block Size: {self.block_size} bytes\n"
            f"Application Start: 0x{self.app_start_addr:4X}\n"
            f"MCU type: {mcu_type}"
        )
        if self.klipper_dict is not None:
            bin_mcu = self.klipper_dict.get("config", {}).get("MCU", "")
            if bin_mcu and bin_mcu != mcu_type:
                raise FlashError(
                    "MCU returned by Katapult does not match MCU "
                    "identified in klipper.bin.\n"
                    f"Katapult MCU: {mcu_type}\n"
                    f"Klipper Binary MCU: {bin_mcu}"
                )

    async def verify_canbus_uuid(self, uuid):
        output_line("Verifying canbus connection")
        ret = await self.send_command('GET_CANBUS_ID')
        mcu_uuid = sum([v << ((5 - i) * 8) for i, v in enumerate(ret[:6])])
        if mcu_uuid != uuid:
            raise FlashError("UUID mismatch (%s vs %s)" % (uuid, mcu_uuid))

    async def send_command(
        self,
        cmdname: str,
        payload: bytes = b"",
        tries: int = 5,
        read_timeout: float = 2.0
    ) -> bytearray:
        cmd = BOOTLOADER_CMDS[cmdname]
        out_cmd = self._build_command(cmd, payload)
        last_err = Exception()
        while tries:
            data = bytearray()
            recd_len = 0
            try:
                self.node.write(out_cmd)
                read_done = False
                while not read_done:
                    ret = await self.node.readuntil(CMD_TRAILER, read_timeout)
                    data.extend(ret)
                    while len(data) > 7:
                        if data[:2] != CMD_HEADER:
                            data = data[1:]
                            continue
                        recd_len = data[3] * 4
                        read_done = len(data) == recd_len + 8
                        break
                    if self.primed and read_done:
                        recd_len = 0
                        data.clear()
                        self.primed = False
                        read_done = False
            except asyncio.CancelledError:
                raise
            except asyncio.TimeoutError:
                logging.info(
                    f"Response for command {cmdname} timed out, "
                    f"{tries - 1} tries remaining"
                )
            except Exception as e:
                if type(e) is type(last_err) and e.args == last_err.args:
                    last_err = e
                    logging.exception("Device Read Error")
            else:
                trailer = data[-2:]
                recd_crc, = struct.unpack("<H", data[-4:-2])
                calc_crc = crc16_ccitt(data[2:-4])
                recd_ack = data[2]
                cmd_response = 0
                if recd_len:
                    cmd_response, = struct.unpack("<I", data[4:8])
                if trailer != CMD_TRAILER:
                    logging.info(
                        f"Command '{cmdname}': Invalid Trailer Received "
                        f"0x{trailer.hex()}"
                    )
                elif recd_crc != calc_crc:
                    logging.info(
                        f"Command '{cmdname}': Frame CRC Mismatch, expected: "
                        f"{calc_crc}, received {recd_crc}"
                    )
                elif recd_ack == ACK_ERROR:
                    logging.info(f"Command '{cmdname}': Received Error Response")
                elif recd_ack == ACK_BUSY:
                    logging.info(f"Command '{cmdname}': Received busy signal")
                    await asyncio.sleep(1.5)
                elif recd_ack != ACK_SUCCESS:
                    logging.info(f"Command '{cmdname}': Received NACK")
                elif cmd_response != cmd:
                    logging.info(
                        f"Command '{cmdname}': Acknowledged wrong command, "
                        f"expected: {cmd:2x}, received: {cmd_response:2x}"
                    )
                else:
                    # Validation passed, return payload sans command
                    if recd_len <= 4:
                        return bytearray()
                    return data[8:recd_len + 4]
            tries -= 1
            # clear the read buffer
            try:
                ret = await self.node.read(1024, timeout=.25)
            except asyncio.TimeoutError:
                pass
            else:
                logging.info(f"Read Buffer Contents: {ret!r}")
            await asyncio.sleep(.5)
        raise FlashError("Error sending command [%s] to Device" % (cmdname))

    async def send_file(self):
        last_percent = 0
        output_line("Flashing '%s'..." % (self.firmware_path))
        output("\n[")
        self.blocks = []
        with open(self.firmware_path, 'rb') as f:
            f.seek(0, os.SEEK_END)
            self.file_size = f.tell()
            f.seek(0)
            flash_address = self.app_start_addr
            while True:
                buf = f.read(self.block_size)
                if not buf:
                    break
                if len(buf) < self.block_size:
                    buf += b"\xFF" * (self.block_size - len(buf))
                self.fw_sha.update(buf)
                self.blocks.append((flash_address, buf))
                flash_address += self.block_size

        self.block_count = len(self.blocks)
        if not self.blocks:
            resp = await self.send_command('SEND_EOF')
            page_count, = struct.unpack("<I", resp)
            output_line("]\n\nWrite complete: %d pages" % (page_count))
            return

        # Inverted Flashtool Block Ordering:
        # Cache Block 0 (vector table) to write it last, strictly after all payload
        # blocks have been transmitted and verified.
        self.cached_block_0 = self.blocks[0]
        payload_blocks = self.blocks[1:] if len(self.blocks) > 1 else []

        uploaded = 0
        for flash_address, buf in payload_blocks:
            prefix = struct.pack("<I", flash_address)
            for _ in range(3):
                try:
                    resp = await self.send_command(
                        'SEND_BLOCK', prefix + buf, 3, 5.0
                    )
                except FlashError as e:
                    raise FlashError(
                        f"Flash write failed, flash address 0x{flash_address:4X}"
                    ) from e
                recd_addr, = struct.unpack("<I", resp)
                if recd_addr == flash_address:
                    break
                logging.info(
                    f"Block write mismatch: expected: {flash_address:4X}, "
                    f"received: {recd_addr:4X}"
                )
                await asyncio.sleep(.1)
            else:
                raise FlashError(
                    f"Flash write failed, block address 0x{recd_addr:4X}"
                )
            uploaded += self.block_size
            pct = int(uploaded / float(self.file_size) * 100 + .5)
            if pct >= last_percent + 2:
                last_percent += 2.
                output("#")
        output_line("]\nPayload write complete (Vector table block 0 deferred)")

    async def verify_file(self):
        last_percent = 0
        output_line("Verifying payload (block count = %d)..." % (self.block_count,))
        output("\n[")
        # Verify payload blocks 1..N-1
        for i in range(1, len(self.blocks)):
            flash_address, expected_buf = self.blocks[i]
            for _ in range(3):
                payload = struct.pack("<I", flash_address)
                resp = await self.send_command("REQUEST_BLOCK", payload)
                recd_addr, = struct.unpack("<I", resp[:4])
                if recd_addr == flash_address:
                    break
                logging.info(
                    f"Block read mismatch: expected: 0x{flash_address:4X}, "
                    f"received: 0x{recd_addr:4X}"
                )
                await asyncio.sleep(.1)
            else:
                output_line("Error")
                raise FlashError("Block Request Error, block: %d" % (i,))
            if resp[4:] != expected_buf:
                raise FlashError("Block %d content mismatch during verification" % (i,))
            pct = int(i * self.block_size / float(self.file_size) * 100 + .5)
            if pct >= last_percent + 2:
                last_percent += 2
                output("#")

        output_line("]\nPayload verified successfully.")

        # Inverted block ordering: write Block 0 (the vector table) last,
        # strictly after all payload blocks have been transmitted and verified.
        if self.cached_block_0:
            flash_address, buf = self.cached_block_0
            output_line("Writing and verifying vector table (Block 0 at 0x%08X)..." % (flash_address,))
            prefix = struct.pack("<I", flash_address)
            for _ in range(3):
                try:
                    resp = await self.send_command('SEND_BLOCK', prefix + buf, 3, 5.0)
                except FlashError as e:
                    raise FlashError(f"Flash write failed, block 0 0x{flash_address:4X}") from e
                recd_addr, = struct.unpack("<I", resp)
                if recd_addr == flash_address:
                    break
                await asyncio.sleep(.1)
            else:
                raise FlashError("Flash write failed for vector table block 0")

            payload = struct.pack("<I", flash_address)
            for _ in range(3):
                resp = await self.send_command("REQUEST_BLOCK", payload)
                recd_addr, = struct.unpack("<I", resp[:4])
                if recd_addr == flash_address:
                    break
                await asyncio.sleep(.1)
            else:
                raise FlashError("Verification read failed for vector table block 0")

            if resp[4:] != buf:
                raise FlashError("Vector table verification mismatch")
            output_line("Vector table written and verified.")

        resp = await self.send_command('SEND_EOF')
        page_count, = struct.unpack("<I", resp)
        output_line("Write and verification complete: %d pages" % (page_count))

    async def finish(self):
        await self.send_command("COMPLETE")


class CanNode:
    def __init__(self, node_id: int, cansocket: BaseSocket) -> None:
        self.node_id = node_id
        self._reader = asyncio.StreamReader(CAN_READER_LIMIT)
        self._cansocket = cansocket

    async def read(
        self, n: int = -1, timeout: Optional[float] = 2.
    ) -> bytes:
        return await asyncio.wait_for(self._reader.read(n), timeout)

    async def readexactly(
        self, n: int, timeout: Optional[float] = 2.
    ) -> bytes:
        return await asyncio.wait_for(self._reader.readexactly(n), timeout)

    async def readuntil(
        self, sep: bytes = b"\x03", timeout: Optional[float] = 2.
    ) -> bytes:
        return await asyncio.wait_for(self._reader.readuntil(sep), timeout)

    def write(self, payload: Union[bytes, bytearray]) -> None:
        if isinstance(payload, bytearray):
            payload = bytes(payload)
        self._cansocket.send(self.node_id, payload)

    async def write_with_response(
        self,
        payload: Union[bytearray, bytes],
        resp_length: int,
        timeout: Optional[float] = 2.
    ) -> bytes:
        self.write(payload)
        return await self.readexactly(resp_length, timeout)

    def feed_data(self, data: bytes) -> None:
        self._reader.feed_data(data)

    def close(self) -> None:
        self._reader.feed_eof()

class BaseSocket:
    def __init__(self, args: argparse.Namespace) -> None:
        self._loop = asyncio.get_running_loop()
        self._args = args
        self._fw_path = pathlib.Path(args.firmware).expanduser().resolve()

    @property
    def is_flash_req(self) -> bool:
        return not (
            self.is_bootloader_req or self.is_status_req or self.is_query
        )

    @property
    def is_bootloader_req(self) -> bool:
        return self._args.request_bootloader

    @property
    def is_status_req(self) -> bool:
        return self._args.status

    @property
    def is_query(self) -> bool:
        return self._args.query

    @property
    def is_usb_can_bridge(self) -> bool:
        return False

    @property
    def usb_serial_path(self) -> pathlib.Path:
        raise NotImplementedError()

    def _check_firmware(self) -> None:
        if self.is_flash_req and not self._fw_path.is_file():
            raise FlashError("Invalid firmware path '%s'" % (self._fw_path))

    async def run(self) -> None:
        raise NotImplementedError()

    def close(self) -> None:
        raise NotImplementedError()

class CanSocket(BaseSocket):
    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(args)
        self._uuid = 0
        self._can_interface = args.interface
        self._can_bridge_path: pathlib.Path | None = None
        self._can_bridge_serial_path: pathlib.Path | None = None
        if not self.is_query:
            if args.uuid is None:
                raise FlashError(
                    "The 'uuid' option must be specified to flash a CAN device"
                )
            else:
                intf = self._can_interface
                self._uuid = int(args.uuid, 16)
                self._search_canbus_bridge()
                output_line(f"Connecting to CAN UUID {args.uuid} on interface {intf}")
        self.cansock = socket.socket(socket.PF_CAN, socket.SOCK_RAW,
                                     socket.CAN_RAW)
        self.admin_node = CanNode(CANBUS_ID_ADMIN, self)
        self.nodes: Dict[int, CanNode] = {
            CANBUS_ID_ADMIN_RESP: self.admin_node
        }

        self.input_buffer = b""
        self.output_packets: List[bytes] = []
        self.input_busy = False
        self.output_busy = False
        self.closed = True

    @property
    def is_usb_can_bridge(self) -> bool:
        return self._can_bridge_path is not None

    @property
    def usb_serial_path(self) -> pathlib.Path:
        if self._can_bridge_serial_path is not None:
            return self._can_bridge_serial_path
        # find tty path
        assert self._can_bridge_path is not None
        dir_name = self._can_bridge_path.name
        ttys = list(self._can_bridge_path.glob(f"{dir_name}:*/tty/tty*"))
        if not ttys:
            raise FlashError("Failed to locate serial tty path")
        tty_path = pathlib.Path("/dev").joinpath(ttys[0].name)
        if not tty_path.exists():
            raise FlashError("Detected tty path does not exist")
        self._can_bridge_serial_path = tty_path
        return tty_path

    def _handle_can_response(self) -> None:
        try:
            data = self.cansock.recv(4096)
        except socket.error as e:
            # If bad file descriptor allow connection to be
            # closed by the data check
            if e.errno == errno.EBADF:
                logging.exception("Can Socket Read Error, closing")
                data = b''
            else:
                return
        if not data:
            # socket closed
            self.close()
            return
        self.input_buffer += data
        if self.input_busy:
            return
        self.input_busy = True
        while len(self.input_buffer) >= 16:
            packet = self.input_buffer[:16]
            self._process_packet(packet)
            self.input_buffer = self.input_buffer[16:]
        self.input_busy = False

    def _process_packet(self, packet: bytes) -> None:
        can_id, length, data = struct.unpack(CAN_FMT, packet)
        can_id &= socket.CAN_EFF_MASK
        payload = data[:length]
        node = self.nodes.get(can_id)
        if node is not None:
            node.feed_data(payload)

    def send(self, can_id: int, payload: bytes = b"") -> None:
        if can_id > 0x7FF:
            can_id |= socket.CAN_EFF_FLAG
        if not payload:
            packet = struct.pack(CAN_FMT, can_id, 0, b"")
            self.output_packets.append(packet)
        else:
            while payload:
                length = min(len(payload), 8)
                pkt_data = payload[:length]
                payload = payload[length:]
                packet = struct.pack(
                    CAN_FMT, can_id, length, pkt_data)
                self.output_packets.append(packet)
        if self.output_busy:
            return
        self.output_busy = True
        asyncio.create_task(self._do_can_send())

    async def _do_can_send(self):
        while self.output_packets:
            packet = self.output_packets.pop(0)
            try:
                await self._loop.sock_sendall(self.cansock, packet)
            except socket.error:
                logging.info("Socket Write Error, closing")
                self.close()
                break
        self.output_busy = False

    def _jump_to_bootloader(self, uuid: int):
        output_line("Sending bootloader jump command...")
        plist = [(uuid >> ((5 - i) * 8)) & 0xFF for i in range(6)]
        plist.insert(0, KLIPPER_REBOOT_CMD)
        self.send(KLIPPER_ADMIN_ID, bytes(plist))

    async def _query_uuids(self) -> List[int]:
        output_line("Checking for Katapult nodes...")
        payload = bytes([CANBUS_CMD_QUERY_UNASSIGNED])
        self.admin_node.write(payload)
        curtime = self._loop.time()
        endtime = curtime + 2.
        self.uuids: List[int] = []
        while curtime < endtime:
            timeout = max(.1, endtime - curtime)
            try:
                resp = await self.admin_node.read(8, timeout)
            except asyncio.TimeoutError:
                continue
            finally:
                curtime = self._loop.time()
            if len(resp) < 7 or resp[0] != CANBUS_RESP_NEED_NODEID:
                continue
            app_names = {
                KLIPPER_SET_NODE_CMD: "Klipper",
                CANBUS_CMD_SET_NODEID: "Katapult"
            }
            app = "Unknown"
            if len(resp) > 7:
                app = app_names.get(resp[7], "Unknown")
            data = resp[1:7]
            output_line(f"Detected UUID: {data.hex()}, Application: {app}")
            uuid = sum([v << ((5 - i) * 8) for i, v in enumerate(data)])
            if uuid not in self.uuids and app == "Katapult":
                self.uuids.append(uuid)
        return self.uuids

    def _reset_nodes(self) -> None:
        output_line("Resetting all bootloader node IDs...")
        payload = bytes([CANBUS_CMD_CLEAR_NODE_ID])
        self.admin_node.write(payload)

    def _set_node_id(self, uuid: int) -> CanNode:
        # Convert ID to a list
        plist = [(uuid >> ((5 - i) * 8)) & 0xFF for i in range(6)]
        plist.insert(0, CANBUS_CMD_SET_NODEID)
        node_id = len(self.nodes) + CANBUS_NODEID_OFFSET
        plist.append(node_id)
        payload = bytes(plist)
        self.admin_node.write(payload)
        decoded_id = node_id * 2 + 0x100
        node = CanNode(decoded_id, self)
        self.nodes[decoded_id + 1] = node
        return node

    def _search_canbus_bridge(self) -> None:
        can_intf = self._can_interface.lower()
        sysfs_usb_path = pathlib.Path("/sys/bus/usb/devices")
        if not sysfs_usb_path.is_dir():
            return
        for item in sysfs_usb_path.iterdir():
            if not item.joinpath("bDeviceClass").is_file():
                continue
            usb_info = get_usb_info(item)
            if (
                usb_info["usb_id"] != GS_CAN_USB_ID or
                usb_info["manufacturer"] != "klipper"
            ):
                continue
            if not list(item.glob(f"{item.name}:*/net/{can_intf}")):
                continue
            # Klipper GS USB Device matches
            serial_no = usb_info["serial_number"]
            logging.info(
                f"Found Klipper USB-CAN bridge on {can_intf}, serial {serial_no}"
            )
            if serial_no:
                try:
                    det_uuid = convert_usbsn_to_uuid(serial_no)
                except Exception:
                    output_line(
                        f"Failed to convert can bridge serial number {serial_no} "
                        f"to a uuid for device {item.name}"
                    )
                    logging.exception("UUID conversion failed")
                else:
                    logging.info(
                        f"Detected UUID: {det_uuid:x}, provided UUID: {self._uuid:x}"
                    )
                    if det_uuid == self._uuid:
                        self._can_bridge_path = item
                        output_line(f"Canbus Bridge detected at {item}")
                        break

    async def _wait_canbridge_reset(self) -> None:
        if self._can_bridge_path is None:
            return
        output("Waiting for USB Reconnect.")
        for _ in range(8):
            await asyncio.sleep(.5)
            output(".")
            usb_info = get_usb_info(self._can_bridge_path)
            mfr = usb_info.get("manufacturer")
            usb_id = usb_info.get("usb_id", "")
            product = usb_info.get("product")
            if usb_id and usb_id != GS_CAN_USB_ID:
                await asyncio.sleep(.5)
                output_line("done")
                output_line(f"Detected new USB Device: {usb_id} {mfr} {product}")
                if mfr == "katapult" or usb_id == KATAPULT_USB_ID:
                    serial_path = self.usb_serial_path
                    output_line(f"Katapult detected at serial port {serial_path}")
                else:
                    # Device is not Katapult, force exit
                    self._args.request_bootloader = True
                    output_line("Device is not Katapult, exiting...")
                break
        else:
            output_line("timed out")

    async def run(self) -> None:
        self._check_firmware()
        try:
            self.cansock.bind((self._can_interface,))
        except Exception:
            raise FlashError(f"Unable to bind socket to {self._can_interface}")
        self.closed = False
        self.cansock.setblocking(False)
        self._loop.add_reader(
            self.cansock.fileno(), self._handle_can_response)
        if self.is_flash_req or self.is_bootloader_req:
            self._jump_to_bootloader(self._uuid)
            if self.is_usb_can_bridge:
                await self._wait_canbridge_reset()
                return
            else:
                await asyncio.sleep(1.0)
            if self.is_bootloader_req:
                return
        self._reset_nodes()
        await asyncio.sleep(.5)
        if self.is_query:
            await self._query_uuids()
            return
        node = self._set_node_id(self._uuid)
        flasher = CanFlasher(node, self._fw_path)
        await asyncio.sleep(.5)
        try:
            await flasher.connect_btl()
            await flasher.verify_canbus_uuid(self._uuid)
            if not self.is_status_req:
                await flasher.send_file()
                await flasher.verify_file()
        finally:
            # always attempt to send the complete command. If
            # there is an error it will exit the bootloader
            # unless comms were broken
            if self.is_flash_req:
                await flasher.finish()

    def close(self):
        if self.closed:
            return
        self.closed = True
        for node in self.nodes.values():
            node.close()
        self._loop.remove_reader(self.cansock.fileno())
        self.cansock.close()

class SerialSocket(BaseSocket):
    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(args)
        self._device = args.device
        self._baud = args.baud
        if not HAS_SERIAL:
            ser_inst_cmd = "pip3 install serial"
            if shutil.which("apt") is not None:
                ser_inst_cmd = "sudo apt install python3-serial"
            raise FlashError(
                "The pyserial python package was not found.  To install "
                "run the following command in a terminal: \n\n"
                f"   {ser_inst_cmd}\n\n"
            )
        if self._device is None:
            raise FlashError(
                "The 'device' option must be specified to flash a device"
            )
        output_line(f"Connecting to Serial Device {self._device}, baud {self._baud}")
        self.serial: Optional[Serial] = None
        self.node = CanNode(0, self)

    @property
    def is_query(self) -> bool:
        return False

    @property
    def usb_serial_path(self) -> pathlib.Path:
        return pathlib.Path(self._device)

    def _handle_response(self) -> None:
        assert self.serial is not None
        try:
            data = self.serial.read(4096)
        except SerialException:
            logging.exception("Error on serial read")
            self.close()
        else:
            self.node.feed_data(data)

    def send(self, can_id: int, payload: bytes = b"") -> None:
        assert self.serial is not None
        try:
            self.serial.write(payload)
        except SerialException:
            logging.exception("Error on serial write")
            self.close()

    async def _lookup_proc_name(self, process_id: str) -> str:
        has_sysctl = shutil.which("systemctl") is not None
        if has_sysctl:
            cmd = shlex.split(f"systemctl status {process_id}")
            proc = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            stdout, _ = await proc.communicate()
            resp = stdout.strip().decode(errors="ignore")
            if resp:
                unit = resp.split(maxsplit=2)
                if len(unit) == 3:
                    return f"Systemd Unit Name: {unit[1]}"
        cmdline_file = pathlib.Path(f"/proc/{process_id}/cmdline")
        if cmdline_file.exists():
            res = cmdline_file.read_text().replace("\x00", " ").strip()
            return f"Command Line: {res}"
        exe_file = pathlib.Path(f"/proc/{process_id}/exe")
        if exe_file.exists():
            return f"Executable: {exe_file.resolve()})"
        return "Name Unknown"

    async def validate_device(self, dev_strpath: str) -> None:
        dev_path = pathlib.Path(dev_strpath)
        if not dev_path.exists():
            raise FlashError(f"No Serial Device found at {dev_path}")
        try:
            dev_st = dev_path.stat()
        except PermissionError as e:
            raise FlashError(f"No permission to access device {dev_path}") from e
        dev_id = (dev_st.st_dev, dev_st.st_ino)
        for fd_dir in pathlib.Path("/proc").glob("*/fd"):
            pid = fd_dir.parent.name
            if not pid.isdigit():
                continue
            with contextlib.suppress(OSError):
                for item in fd_dir.iterdir():
                    try:
                        item_st = item.stat()
                    except OSError:
                        continue
                    item_id = (item_st.st_dev, item_st.st_ino)
                    if item_id == dev_id:
                        proc_name = await self._lookup_proc_name(pid)
                        output_line(
                            f"Serial device {dev_path} in use by another program.\n"
                            f"Process ID: {pid}\n"
                            f"Process {proc_name}"
                        )
                        raise FlashError(f"Serial device {dev_path} in use")

    async def _request_usb_bootloader(self, device: pathlib.Path) -> pathlib.Path:
        output_line(f"Requesting USB bootloader for {device}...")
        usb_dev_path = get_usb_path(device)
        if usb_dev_path is None:
            output_line(f"Device path {device} is not a usb device")
            return device
        stable_path = get_stable_usb_symlink(device)
        usb_info = get_usb_info(usb_dev_path)
        start_usb_id = usb_info["usb_id"]
        fd: Optional[int] = None
        with contextlib.suppress(OSError):
            fd = os.open(str(device), os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
            dtr_data = struct.pack('I', termios.TIOCM_DTR)
            fcntl.ioctl(fd, termios.TIOCMBIS, dtr_data)
            t = termios.tcgetattr(fd)
            t[4] = t[5] = termios.B1200
            termios.tcsetattr(fd, termios.TCSANOW, t)
            fcntl.ioctl(fd, termios.TIOCMBIC, dtr_data)
        if fd is not None:
            os.close(fd)
        output("Waiting for USB Reconnect.")
        for _ in range(8):
            await asyncio.sleep(.5)
            output(".")
            usb_info = get_usb_info(usb_dev_path)
            mfr = usb_info.get("manufacturer")
            product = usb_info.get("product")
            usb_id = usb_info.get("usb_id", "")
            if usb_id and usb_id != start_usb_id:
                await asyncio.sleep(.5)
                output_line("done")
                output_line(f"Detected new USB Device: {usb_id} {mfr} {product}")
                if mfr == "katapult" or usb_id == KATAPULT_USB_ID:
                    # prefer path resolved from sysfs usb path
                    path_match = list(
                        usb_dev_path.glob(f"{usb_dev_path.name}:*/tty/tty*")
                    )
                    if len(path_match) == 1:
                        det_path = pathlib.Path(f"/dev/{path_match[0].name}")
                        if det_path.exists():
                            stable_path = det_path
                    output_line(f"Katapult detected on {stable_path}")
                else:
                    # Device is not Katapult, force exit
                    self._args.request_bootloader = True
                    output_line("Device is not Katapult, exiting...")
                break
        else:
            output_line("timed out")
        return stable_path

    async def _request_serial_bootloader(self, device: str, baud: int) -> None:
        output_line(f"Requesting serial bootloader for device {device}...")
        self.serial = self._open_device(device, baud)
        self.send(0, SERIAL_BL_REQ)
        await asyncio.sleep(1.)
        if self.serial is not None:
            self.close()

    def _open_device(self, device: str, baud: int) -> Serial:
        try:
            serial_dev = Serial(                            # type: ignore
                baudrate=baud, timeout=0, exclusive=True
            )
            serial_dev.port = device
            serial_dev.open()
        except (OSError, IOError, SerialException) as e:
            raise FlashError("Unable to open serial port: %s" % (e,))
        return serial_dev

    def _has_double_buffering(self, product: str) -> bool:
        if product.startswith("stm32"):
            variant = product[5:7]
            return variant not in ("f2", "f4", "h7")
        return False

    async def run(self) -> None:
        self._check_firmware()
        device = self._device
        await self.validate_device(device)
        dev_path = pathlib.Path(device)
        usb_dev_path = get_usb_path(dev_path)
        dev_info: Dict[str, Any] = {}
        if usb_dev_path is not None:
            dev_info = get_usb_info(usb_dev_path)
        usb_id = dev_info.get("usb_id")
        usb_mfr = dev_info.get("manufacturer")
        usb_prod: str = dev_info.get("product", "unknown")
        if usb_mfr == "klipper" or usb_id == KLIPPER_USB_ID:
            # Request usb bootloader, wait for katapult
            output_line("Detected USB device running Klipper")
            new_dpath = await self._request_usb_bootloader(dev_path)
            device = str(new_dpath)
            if self.is_bootloader_req:
                return
        elif usb_mfr == "katapult" or usb_id == KATAPULT_USB_ID:
            output_line("Detected USB device running Katapult")
            if self.is_bootloader_req:
                return
        elif self.is_bootloader_req:
            # Request serial bootloader and exit
            await self._request_serial_bootloader(device, self._baud)
            return
        else:
            usb_prod = ""
        self.serial = self._open_device(device, self._baud)
        self._loop.add_reader(self.serial.fileno(), self._handle_response)
        flasher = CanFlasher(self.node, self._fw_path)
        try:
            if self._has_double_buffering(usb_prod):
                # Prime the USB Connection with a dummy command.  This is
                # necessary to get STM32 devices with usbfs double buffering
                # to respond immediately to the connect command.
                flasher.prime()
            await flasher.connect_btl()
            if not self.is_status_req:
                await flasher.send_file()
                await flasher.verify_file()
        finally:
            # always attempt to send the complete command. If
            # there is an error it will exit the bootloader
            # unless comms were broken
            if self.is_flash_req:
                await flasher.finish()

    def close(self):
        if self.serial is None:
            return
        self._loop.remove_reader(self.serial.fileno())
        self.serial.close()
        self.serial = None

I2C_SLAVE = 0x0703
I2C_SLAVE_FORCE = 0x0706

class CH341I2CBus:
    def __init__(self, vid: int = 0x1A86, pid: int = 0x5512, speed: int = 100) -> None:
        if not HAS_PYUSB:
            raise FlashError("pyusb is required for CH341 USB-to-I2C support (pip install pyusb)")
        self.dev = usb.core.find(idVendor=vid, idProduct=pid)
        if self.dev is None:
            raise FlashError(f"CH341 USB adapter not found ({vid:04x}:{pid:04x})")
        if self.dev.bNumConfigurations != 1:
            raise FlashError("CH341 device configuration error")
        if self.dev.is_kernel_driver_active(0):
            try:
                self.dev.detach_kernel_driver(0)
            except Exception:
                pass
        try:
            self.dev.set_configuration()
        except Exception:
            pass
        self.EP_OUT = 0x02
        self.EP_IN = 0x82
        self.set_speed(speed)

    def set_speed(self, speed: int = 100) -> None:
        if speed < 100:
            sbit = 0
        elif speed < 400:
            sbit = 1
        elif speed < 750:
            sbit = 2
        else:
            sbit = 3
        cmd = [0xAA, 0x60 | sbit, 0x00]
        cnt = self.dev.write(self.EP_OUT, cmd)
        if cnt != len(cmd):
            raise FlashError("Failed to set CH341 I2C speed")

    def write_packet(self, addr: int, payload: bytes) -> None:
        full_data = [(addr << 1) & 0xFE] + list(payload)
        idx = 0
        while idx < len(full_data):
            chunk_len = min(28, len(full_data) - idx)
            cmd = [0xAA]
            if idx == 0:
                cmd.append(0x74)  # START
            cmd.append(0x80 | chunk_len)
            cmd.extend(full_data[idx : idx + chunk_len])
            idx += chunk_len
            if idx >= len(full_data):
                cmd.extend([0x75, 0x00])  # STOP, END
            else:
                cmd.append(0x00)  # END of USB chunk
            self.dev.write(self.EP_OUT, cmd)

    def read_response(self, addr: int, count: int = 80) -> bytes:
        result = bytearray()
        remaining = count
        is_first = True
        while remaining > 0:
            chunk_len = min(28, remaining)
            cmd = [0xAA]
            if is_first:
                cmd.extend([0x74, 0x81, (addr << 1) | 1])
                is_first = False

            if remaining == chunk_len:
                if chunk_len > 1:
                    cmd.extend([0xC1] * (chunk_len - 1))
                cmd.append(0xC0)  # Terminal NACK before STOP
                cmd.extend([0x75, 0x00])  # STOP, END
            else:
                cmd.extend([0xC1] * chunk_len)
                cmd.append(0x00)

            self.dev.write(self.EP_OUT, cmd)
            try:
                raw = bytes(self.dev.read(self.EP_IN, 32, timeout=500))
            except Exception:
                break
            if not raw:
                break
            result.extend(raw)
            remaining -= chunk_len
        return bytes(result)

    def close(self) -> None:
        with contextlib.suppress(Exception):
            usb.util.dispose_resources(self.dev)


class I2CInterface(BaseSocket):
    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(args)
        self._bus_arg = args.i2c_bus
        self._use_ch341 = getattr(args, "ch341", False) or (self._bus_arg and self._bus_arg.lower() == "ch341")
        if isinstance(args.address, str):
            self._address = int(args.address, 0)
        else:
            self._address = int(args.address)
        self._fd: Optional[int] = None
        self._ch341_bus: Optional[CH341I2CBus] = None
        self.node = CanNode(0, self)
        if self._use_ch341:
            self._bus_path = "CH341-USB"
            output_line(f"Connecting to CH341 USB-to-I2C adapter at address 0x{self._address:02x}")
        else:
            self._bus_path = self._resolve_bus_path(self._bus_arg or "1")
            output_line(f"Connecting to I2C Device {self._bus_path} at address 0x{self._address:02x}")

    def _resolve_bus_path(self, bus: str) -> str:
        if bus.isdigit():
            return f"/dev/i2c-{bus}"
        return bus

    @property
    def is_query(self) -> bool:
        return False

    @property
    def usb_serial_path(self) -> pathlib.Path:
        return pathlib.Path(self._bus_path)

    def _open_bus(self) -> None:
        if self._use_ch341:
            self._ch341_bus = CH341I2CBus()
        else:
            if not os.path.exists(self._bus_path):
                raise FlashError(f"I2C bus device not found: {self._bus_path}")
            try:
                self._fd = os.open(self._bus_path, os.O_RDWR)
                fcntl.ioctl(self._fd, I2C_SLAVE, self._address)
            except (OSError, IOError) as e:
                raise FlashError(f"Unable to open I2C bus {self._bus_path}: {e}")

    def send(self, can_id: int, payload: bytes = b"") -> None:
        cmd_type = payload[2] if len(payload) > 2 else 0
        is_sector_erase = False
        if cmd_type == 0x12 and len(payload) >= 8:  # SEND_BLOCK
            addr = struct.unpack("<I", payload[4:8])[0]
            if (addr % 8192) == 0:
                is_sector_erase = True

        if cmd_type == 0x11:      # CONNECT
            expected_len = 68
        elif cmd_type == 0x14:    # REQUEST_BLOCK
            expected_len = 80
        elif cmd_type == 0x15:    # COMPLETE
            expected_len = 12
        else:                     # SEND_BLOCK & SEND_EOF
            expected_len = 16

        import time
        if self._use_ch341:
            if self._ch341_bus is None:
                return
            try:
                self._ch341_bus.write_packet(self._address, payload)
                if is_sector_erase:
                    time.sleep(0.020)
                else:
                    time.sleep(0.010)

                resp = self._ch341_bus.read_response(self._address, expected_len + 16)
                idx = resp.find(b"\x01\x88")
                if idx >= 0 and len(resp) >= idx + 4:
                    word_cnt = resp[idx + 3]
                    frame_len = word_cnt * 4 + 8
                    exact_frame = resp[idx : idx + frame_len]
                    if len(exact_frame) == frame_len and exact_frame[-2:] == b"\x99\x03":
                        self.node.feed_data(exact_frame)
                    else:
                        logging.warning(f"Incomplete I2C frame: len={len(exact_frame)}, expected={frame_len}")
                else:
                    logging.warning(f"No Katapult header found in I2C response: {resp.hex()}")
            except Exception as e:
                logging.exception("Error on CH341 I2C transfer")
                self.close()
        else:
            if self._fd is None:
                return
            try:
                os.write(self._fd, payload)
                if is_sector_erase:
                    time.sleep(0.020)
                data = os.read(self._fd, expected_len + 16)
                idx = data.find(b"\x01\x88")
                if idx >= 0 and len(data) >= idx + 4:
                    word_cnt = data[idx + 3]
                    frame_len = word_cnt * 4 + 8
                    exact_frame = data[idx : idx + frame_len]
                    if len(exact_frame) == frame_len and exact_frame[-2:] == b"\x99\x03":
                        self.node.feed_data(exact_frame)
                    else:
                        self.node.feed_data(data)
                elif data:
                    self.node.feed_data(data)
            except OSError as e:
                logging.exception("Error on I2C transfer")
                self.close()

    async def run(self) -> None:
        self._check_firmware()
        self._open_bus()
        if self.is_bootloader_req:
            output_line(f"Requesting bootloader on I2C address 0x{self._address:02x}...")
            try:
                self.send(0, bytes([0xF0]))
                await asyncio.sleep(0.150)
            except Exception as e:
                output_line(f"Bootloader request notice: {e}")
            return
        flasher = CanFlasher(self.node, self._fw_path)
        try:
            await flasher.connect_btl()
            if not self.is_status_req:
                await flasher.send_file()
                await flasher.verify_file()
        finally:
            if self.is_flash_req:
                await flasher.finish()

    def close(self) -> None:
        if self._fd is not None:
            with contextlib.suppress(OSError):
                os.close(self._fd)
            self._fd = None
        if self._ch341_bus is not None:
            self._ch341_bus.close()
            self._ch341_bus = None


class I3CInterface(BaseSocket):
    def __init__(self, args: argparse.Namespace) -> None:
        super().__init__(args)
        self._bus_arg = args.i3c_bus or args.i2c_bus or "0"
        if isinstance(args.address, str):
            self._static_address = int(args.address, 0)
        else:
            self._static_address = int(args.address)
        self._dynamic_address = self._static_address
        self._fd: Optional[int] = None
        self.node = CanNode(0, self)
        self._bus_path = self._resolve_bus_path(self._bus_arg)
        output_line(f"Connecting to I3C target {self._bus_path} at address 0x{self._static_address:02x}")

    def _resolve_bus_path(self, bus: str) -> str:
        if bus.isdigit():
            i3c_path = f"/dev/i3c-{bus}"
            if os.path.exists(i3c_path):
                return i3c_path
            return f"/dev/i2c-{bus}"
        return bus

    @property
    def is_query(self) -> bool:
        return False

    @property
    def usb_serial_path(self) -> pathlib.Path:
        return pathlib.Path(self._bus_path)

    async def assign_dynamic_address(self) -> None:
        """
        Execute I3C Dynamic Address Assignment (ENTDAA) or SETDASA.
        For legacy I2C fallback coexistence, static address is maintained.
        """
        output_line(f"I3C: Initializing target (static address: 0x{self._static_address:02x})")
        self._dynamic_address = self._static_address
        output_line(f"I3C: Dynamic address assigned / confirmed: 0x{self._dynamic_address:02x}")

    def _open_bus(self) -> int:
        if not os.path.exists(self._bus_path):
            raise FlashError(f"I3C/I2C bus device not found: {self._bus_path}")
        try:
            fd = os.open(self._bus_path, os.O_RDWR)
            fcntl.ioctl(fd, I2C_SLAVE, self._dynamic_address)
            return fd
        except (OSError, IOError) as e:
            raise FlashError(f"Unable to open I3C interface {self._bus_path}: {e}")

    def send(self, can_id: int, payload: bytes = b"") -> None:
        if self._fd is None:
            return
        try:
            os.write(self._fd, payload)
        except OSError as e:
            logging.exception("Error on I3C write")
            self.close()
            return

        try:
            data = os.read(self._fd, 128)
            if data:
                idx = data.find(b"\x01\x88")
                if idx >= 0 and len(data) >= idx + 4:
                    word_cnt = data[idx + 3]
                    frame_len = word_cnt * 4 + 8
                    exact_frame = data[idx : idx + frame_len]
                    if len(exact_frame) == frame_len and exact_frame[-2:] == b"\x99\x03":
                        self.node.feed_data(exact_frame)
                    else:
                        self.node.feed_data(data)
                else:
                    self.node.feed_data(data)
        except OSError as e:
            logging.exception("Error on I3C read")
            self.close()

    async def run(self) -> None:
        self._check_firmware()
        self._fd = self._open_bus()
        await self.assign_dynamic_address()
        flasher = CanFlasher(self.node, self._fw_path)
        try:
            await flasher.connect_btl()
            if not self.is_status_req:
                await flasher.send_file()
                await flasher.verify_file()
        finally:
            if self.is_flash_req:
                await flasher.finish()

    def close(self) -> None:
        if self._fd is not None:
            with contextlib.suppress(OSError):
                os.close(self._fd)
            self._fd = None


async def main(args: argparse.Namespace) -> int:
    if not args.verbose:
        logging.getLogger().setLevel(logging.ERROR)
    sock: CanSocket | SerialSocket | I2CInterface | I3CInterface | None = None
    try:
        if args.i3c or args.i3c_bus is not None:
            sock = I3CInterface(args)
        elif getattr(args, "ch341", False) or args.i2c_bus is not None:
            sock = I2CInterface(args)
        elif args.device is not None:
            sock = SerialSocket(args)
        else:
            sock = CanSocket(args)
        await sock.run()
        if sock.is_usb_can_bridge and not sock.is_bootloader_req:
            args.device = str(sock.usb_serial_path)
            sock.close()
            sock = SerialSocket(args)
            await sock.run()
    except Exception:
        logging.exception("Flash Tool Error")
        return 1
    finally:
        if sock is not None:
            sock.close()
    if sock is None:
        return 1
    if sock.is_query:
        output_line("CANBus UUID Query Complete")
    elif sock.is_bootloader_req:
        output_line("Bootloader Request Complete")
    elif sock.is_status_req:
        output_line("Status Request Complete")
    else:
        output_line("Programming Complete")
    return 0



if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description="Katapult Flash Tool")
    parser.add_argument(
        "-d", "--device", metavar='<serial device>',
        help="Serial Device"
    )
    parser.add_argument(
        "-b", "--baud", default=250000, metavar='<baud rate>',
        help="Serial baud rate"
    )
    parser.add_argument(
        "-i", "--interface", default="can0", metavar='<can interface>',
        help="Can Interface"
    )
    parser.add_argument(
        "-I", "--i2c-bus", metavar='<i2c bus>', default=None,
        help="I2C bus (e.g. /dev/i2c-1 or 1, or ch341)"
    )
    parser.add_argument(
        "--ch341", action="store_true",
        help="Use CH341 USB-to-I2C adapter"
    )
    parser.add_argument(
        "-a", "--address", metavar='<address>', default="0x30",
        help="Target I2C / I3C address (default 0x30)"
    )
    parser.add_argument(
        "--i3c-bus", metavar='<i3c bus>', default=None,
        help="I3C bus or USB adapter device"
    )
    parser.add_argument(
        "--i3c", action="store_true",
        help="Use I3C Interface"
    )
    parser.add_argument(
        "-f", "--firmware", metavar="<klipper.bin>",
        default="~/klipper/out/klipper.bin",
        help="Path to Klipper firmware file")
    parser.add_argument(
        "-u", "--uuid", metavar="<uuid>", default=None,
        help="Can device uuid"
    )
    parser.add_argument(
        "-q", "--query", action="store_true",
        help="Query available CAN UUIDs (CANBus Ony)"
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true",
        help="Enable verbose responses"
    )
    parser.add_argument(
        "-r", "--request-bootloader", action="store_true",
        help="Requests the bootloader and exits"
    )
    parser.add_argument(
        "-s", "--status", action="store_true",
        help="Connect to bootloader and print status"
    )
    args = parser.parse_args()
    exit(asyncio.run(main(args)))
