#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR=/run/media/sun/halo/bareBoot
FIRMWARE="$PROJECT_DIR/bareboot/target/riscv32imac-unknown-none-elf/release/bareboot-ch32v203"

if ! command -v wlink >/dev/null 2>&1; then
    echo "wlink is not installed. Install it with:"
    echo "  cargo install wlink"
    exit 1
fi

if [ ! -f "$FIRMWARE" ]; then
    echo "Firmware not found; building..."
    cd "$PROJECT_DIR/bareboot"
    BAREBOOT_CONFIG="$PROJECT_DIR/bareboot.toml" cargo build --target riscv32imac-unknown-none-elf --release -p bareboot-ch32v203
    cd - >/dev/null
fi

echo "Current wlink probe status:"
wlink list || true

# The WCH-Link should be in RV mode before flashing CH32V203.
wlink mode-switch --rv

echo "Flashing $FIRMWARE to CH32V20X..."
wlink flash --chip CH32V20X --erase --no-run "$FIRMWARE"

echo ""
echo "Flash complete."
echo "Connect the serial adapter to USART1 (PA9/PA10) or, on custom boards, use the chosen transport."
echo "On the host, you can test with:"
echo "  bareboot-cli --serial /dev/ttyUSB0 --baud 115200 --firmware <app.bin> --tries 20 --timeout 3 --no-verify"
