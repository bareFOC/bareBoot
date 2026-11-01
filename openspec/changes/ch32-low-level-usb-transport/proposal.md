## Why

BareBoot already validates a shared, byte-oriented Katapult protocol on the Bluepill UART path and has reusable host/session and target trait scaffolding. USB is required for zero-wire setup and host portability, but the current CH32V203 USB attempt only reaches block upload progress. Its EOF/COMPLETE response path is unreliable, flash persistence is unverified, and the image is still experimentally oversized.

## What Changes

- Port a small, low-level CH32 USBFS/ CDC byte transport from the ch32fun reference (fsusb.c, usb_defines.h, ch20x_30x_flash.h) into the BareBoot Rust target architecture.
- Replace HAL-heavy USB code on the dedicated CH32 USB transport path with an interrupt-free, state-driven packet byte stream.
- Use the same inode-stream framing and ACK-then-dispatch logic as UART/CAN/I2C.
- Normalize flash sequencing to the CH32 256-byte page model in both target and core verification paths.
- Add a dedicated 8 KiB (and later 4 KiB) CH32 profile with USB-enabled pins and clock configuration, while keeping the UART path untouched for constrained builds.
- Expose a minimal completion path that does not require blocking response delays:
  - receive and scan one command frame,
  - wait for all acknowledgment frames to leave the endpoint queue,
  - `COMPLETE` only after the host acknowledges EOF.
- Improve host discovery/status handling so CDC devices and bare serial paths are treated consistently.

## Capabilities

### New Capabilities
- `ch32-usbfs-transport`: bare-metal USB CDC byte stream for CH32 V2/V3 FS-compatible MCUs.
- `usb-ack-staging`: explicit request/response completion state shared by UART/CAN/I2C/USB host flows.
- `family-usb-portability`: per-family driver registration while retaining one transport trait.

### Modified Capabilities
- `transport-matrix`: extend the feature and size model with a low-overhead USB transport.
- `configuration-pipeline`: validate that USB-specific clocks and endpoint settings are generated from `bareboot.toml`.

## Impact

- Firmware:
  - `bareboot-ch32v203` gets a USB transport module or a separate `usbfs` target.
  - Target-sized ports may choose 8 KiB or 16 KiB offsets while 4 KiB remains available for constrained UART/CAN builds.
- Host tooling:
  - `bareboot-client` treats any CDC ACM device through the same `HostBus` flow.
  - Linux-first discovery is immediate; cross-platform discovery follows once the byte-flow contract is frozen.
- Validation:
  - USB CONNECT + single RX_BLOCK + EOF + COMPLETE
  - flash readback at `0x08001000`
  - application jump on the Bluepill/CH32 experiment image before considering production sizing
