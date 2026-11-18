#!/usr/bin/env bash
set -euo pipefail

# WCH-Link udev fix for CH32V203 firmware programming with wlink.
if [ "$(id -u)" -ne 0 ]; then
    echo "Please run this script as root: sudo ./wch-udev-fix.sh" >&2
    exit 1
fi

cat >/etc/udev/rules.d/60-wch.rules <<'RULES'
SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="8010", MODE="0666"
SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="8012", MODE="0666"
SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="8008", MODE="0666"
RULES

udevadm control --reload-rules
udevadm trigger
wlink mode-switch --to-rv --rv 2>/dev/null || true

echo "WCH rules installed."
echo "Current WCH links:"
/home/sun/.cargo/bin/wlink list || wlink list || true

# chmod existing devices as a fallback just in case the rules do not immediately apply
for d in /dev/bus/usb/*/*; do
    if sudo cat "$d" >/dev/null 2>&1; then
        :
    fi
done

echo "You can now try flashing the CH32V203 bootloader:"
echo "wlink flash --chip CH32V20X --erase --no-run /run/media/sun/halo/bareBoot/bareboot/target/riscv32imac-unknown-none-elf/release/bareboot-ch32v203"
