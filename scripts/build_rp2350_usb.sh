#!/usr/bin/env bash
# Build the RP2350 USB BareBoot bootloader (Katapult-compatible, no BOOTSEL fallback).
#
# This target is based on the Katapult reference bootloader (ref/katapult),
# which provides a proven hand-written USB CDC stack for RP2350. The Embassy-
# based Rust target (targets/rp2350-usb) did not enumerate reliably on RP2350
# A4 hardware; the Katapult transport does.
#
# BareBoot additions over upstream Katapult:
#   - commit_state.c/h: persistent VALID / UPDATE_IN_PROGRESS record in the
#     last 4 KB sector of the 32 KB bootloader slot (flash offset 0x7000).
#   - application_check_valid() requires a VALID commit record, so a torn or
#     interrupted update is never booted.
#   - First SEND_BLOCK marks the transfer in progress; COMPLETE (sent by stock
#     flashtool only after readback SHA1 verification) marks it VALID.
#   - No reset_to_usb_boot()/BOOTSEL fallback anywhere.
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KATAPULT="$ROOT/ref/katapult"
CONFIG="$KATAPULT/test/configs/bareboot-rp2350-usb.config"

echo "[+] Building RP2350 USB BareBoot from Katapult reference"
cd "$KATAPULT"
cp "$CONFIG" .config
make olddefconfig >/dev/null
make

echo "[+] Artifacts:"
echo "    $KATAPULT/out/katapult.bin   (raw binary, flash at 0x10000000)"
echo "    $KATAPULT/out/katapult.uf2   (UF2 via picotool)"
echo "    $KATAPULT/out/katapult.elf   (ELF with symbols)"
