## 1. Workspace and protocol crate

- [x] 1.1 Create Cargo workspace with protocol, core, targets, client, cli, and tui crates.
- [x] 1.2 Implement CRC16-CCITT with scalar test vectors from Katapult C and Python hosts.
- [x] 1.3 Implement fixed-frame parser and serializer with zero-copy slices.
- [x] 1.4 Implement command dispatch, all command handlers, and NACK/error/busy responses.
- [x] 1.5 Add CONNECT metadata, block operation, EOF, and GET_CANBUS_ID handling.
- [x] 1.6 Add host-side protocol tests using valid, malformed, and truncated frames.

## 2. Core runtime

- [x] 2.1 Define FlashTarget and transport traits with application start address and UID.
- [x] 2.2 Implement polling countdown and first-arrival transport arbiter.
- [x] 2.3 Implement flash buffer lifecycle and EOF commit behavior.
- [x] 2.4 Implement safe application jump abstraction per target architecture.
- [x] 2.5 Add core tests with fake transport and fake flash implementations.

## 3. Configuration pipeline

- [x] 3.1 Define `bareboot.toml` schema and validate required fields.
- [x] 3.2 Implement build.rs generation of memory.x and generated constants.
- [x] 3.3 Add Cargo feature selection for enabled transports.
- [x] 3.4 Add CI size checks and release optimization profiles.

## 4. CH32V203 target

- [x] 4.1 Validate `ch32-metapac` feature for CH32V203 and target toolchain.
- [ ] 4.2 Implement clock tree setup to 144 MHz.
- [ ] 4.3 Implement Fast-Flash unlock, erase, and page-buffered writes.
- [ ] 4.4 Implement bxCAN init, filters, receive loop, and CAN administrative state.
- [ ] 4.5 Implement timer-based boot countdown.
- [ ] 4.6 Implement RISC-V vector relocation and application jump.
- [ ] 4.7 Measure and reduce firmware size to at most 3.5 KB when CAN-only.

## 5. STM32G0 target

- [x] 5.1 Validate `stm32-metapac` feature for selected STM32G0 variant.
- [ ] 5.2 Implement clock setup and flash FPEC page handling.
- [ ] 5.3 Implement FDCAN or bxCAN driver and admin behavior.
- [ ] 5.4 Implement I2C target mode with clock stretching.
- [ ] 5.5 Implement Cortex-M vector relocation and application jump.
- [ ] 5.6 Measure and reduce firmware size to at most 3.5 KB when CAN or I2C only.

## 6. Host flash engine and CLI

- [x] 6.1 Implement transport trait with SocketCAN backend.
- [ ] 6.2 Implement I2C backend for Linux dev nodes and CH341/MCP2221 bridges.
- [ ] 6.3 Implement serial backend with framed byte streaming.
- [x] 6.4 Implement node discovery and UUID verification.
- [x] 6.5 Implement block streaming, EOF, verification, and COMPLETE.
- [x] 6.6 Create scriptable CLI with address, timeout, and firmware arguments.

## 7. Interactive TUI

- [x] 7.1 Implement worker thread and UI/event channels.
- [x] 7.2 Implement bus node list, progress gauges, and packet logs.
- [x] 7.3 Implement configuration editor backed by `bareboot.toml`.
- [x] 7.4 Implement cargo build log streaming and error capture.
- [x] 7.5 Integrate probe-rs factory flashing module.

## 8. Validation

- [ ] 8.1 Test protocol core against host-generated Katapult frames.
- [ ] 8.2 Flash and verify CH32V203 with a SimpleFOC test application over CAN.
- [ ] 8.3 Validate I2C discovery, flashing, and jump on STM32G0.
- [ ] 8.4 Validate multi-node CAN flashing through slip-ring wiring.
- [ ] 8.5 Document board configs, pin maps, and known limitations.
