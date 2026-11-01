## Why

CH32V203 and STM32G0 gimbal motor nodes need a very small bootloader (4 KB or less) that remains flashable with the existing Katapult host tooling, while adding a compile-time configuration pipeline and a Rust-native host flashing cockpit. This lets BareBoot targets coexist with the Katapult ecosystem instead of requiring a completely bespoke protocol or repeated manual configuration.

## What Changes

- Establish a Rust workspace for `bareboot` firmware and `bareflash` host tooling with a pure, `no_std` protocol/core layer that can be unit-tested on the host.
- Implement the Katapult wire protocol byte-for-byte, including its 2-byte `0x01 0x88` / `0x99 0x03` frame, CRC16-CCITT checksum, command set (`CONNECT`, `SEND_BLOCK`, `SEND_EOF`, `REQUEST_BLOCK`, `COMPLETE`, `GET_CANBUS_ID`), NACK/error/busy responses, and CAN admin node discovery/assignment behavior.
- Add target-specific bare-metal drivers for CH32V203 (QingKe RV32IMAC) and STM32G0 (Cortex-M0+) using PACs directly, including clock setup, flash erase/write with page buffering, CAN/I2C/UART/USB transports as selected, and safe application jump.
- Replace Kconfig/menuconfig with a declarative `bareboot.toml` and `build.rs` that emits linker scripts, constants, and enabled transport features.
- Add a non-blocking multi-transport arbiter that can lock to the first valid interface during a boot window for images built with two transports.
- Build a host flashing engine with SocketCAN, I2C, and serial backends for discovery, pipelined block writes, read-back verification, and jump-to-application.
- Add an optional `ratatui` cockpit for bus scanning, configuration generation, and probe-based factory flashing.
- Keep every firmware image within a strict 4 KB limit, with CI size checks and optimization profiles.

## Capabilities

### New Capabilities

- `firmware-protocol`: Katapult-compatible framed protocol, CRC16-CCITT, command dispatch, response framing, CAN admin UUID/node assignment, and receive-window handling.
- `bootloader-runtime`: Polling state machine, countdown-based first-arrival transport arbiter, optional multi-interface operation, flash buffer lifecycle, and application jump behavior.
- `flash-target`: Flash erase/write/read traits, page-buffered writes, UID readout, app base address enforcement, and post-write verification hooks.
- `transport-matrix`: Compile-time selectable CAN, I2C, UART, and USB transport implementations plus byte-stream feeding into the shared protocol parser.
- `configuration-pipeline`: Declarative `bareboot.toml`, generated constants and linker scripts, and feature selection at compile time.
- `host-flash-engine`: Cross-bus node discovery, target mismatch detection, pipelined block streaming, EOF verification, and jump orchestration.
- `interactive-tui`: Optional terminal cockpit for node scanning, configuration editing, compilation logs, and probe-based factory flashing.

### Modified Capabilities

<!-- No existing main specs are present, so no requirements are being changed. -->

## Impact

- New firmware crates: `bareboot-protocol`, `bareboot-core`, `bareboot-ch32v203`, and `bareboot-stm32g0`.
- New host crates: `bareboot-client`, `bareboot-cli`, and `bareboot-tui`.
- Depends on `ch32-metapac`, `stm32-metapac`, `riscv-rt` or `cortex-m-rt`, `probe-rs`, SocketCAN/I2C/serial host crates, and `ratatui`.
- Affects future SimpleFOC nodes, gimbal wiring through slip rings, and potentially Klipper ecosystem interoperability because the wire protocol stays byte-compatible with Katapult.
- Requires target board validation for CH32 clock tree, bxCAN bit timing, I2C clock stretching, and flash page behavior under 4 KB.
