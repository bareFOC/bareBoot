# Tasks

## 1. Protocol and Framing (Spec: firmware-protocol)
- [x] 1.1 Create Cargo workspace with protocol, core, targets, client, cli, and tui crates.
- [x] 1.2 Implement CRC16-CCITT with scalar test vectors from Katapult C and Python hosts.
- [x] 1.3 Implement fixed-frame parser and serializer with zero-copy slices.
- [x] 1.4 Implement command dispatch, all command handlers, and NACK/error/busy responses.
- [x] 1.5 Add CONNECT metadata, block operation, EOF, and GET_CANBUS_ID handling.
- [x] 1.6 Add host-side protocol tests using valid, malformed, and truncated frames.

## 2. Core Runtime (Spec: bootloader-runtime, flash-target)
- [x] 2.1 Define FlashTarget and transport traits with application start address and UID.
- [x] 2.2 Implement polling countdown and first-arrival transport arbiter.
- [x] 2.3 Implement flash buffer lifecycle and EOF commit behavior.
- [x] 2.4 Implement safe application jump abstraction per target architecture.
- [x] 2.5 Add core tests with fake transport and fake flash implementations.

## 3. Configuration Pipeline (Spec: configuration-pipeline)
- [x] 3.1 Define `bareboot.toml` schema and validate required fields.
- [x] 3.2 Implement build.rs generation of memory.x and generated constants.
- [x] 3.3 Add Cargo feature selection for enabled transports.
- [x] 3.4 Add CI size checks and release optimization profiles.

## 4. STM32F103 BluePill Target (UART)
- [x] 4.1 Validate `stm32-metapac` feature for STM32F103 and target toolchain.
- [x] 4.2 Implement clock setup and flash page handling for STM32F1.
- [x] 4.3 Implement UART transport with IRQ-driven RX ring buffer and echo suppression.
- [x] 4.4 Implement polling boot countdown and application jump.
- [x] 4.5 Optimize for small footprint (UART-only) with fixed app base at 0x08002000.
- [x] 4.6 Validate blink app flashing over UART.

## 5. Host Flash Engine (Spec: host-flash-engine)
- [x] 5.1 Implement serial backend with framed byte streaming.
- [x] 5.2 Implement node discovery and UUID verification.
- [x] 5.3 Implement block streaming, EOF, verification, and COMPLETE.
- [x] 5.4 Create scriptable CLI with address, timeout, and firmware arguments.

## 6. Validation
- [x] 6.1 Test protocol core against host-generated Katapult frames.
- [x] 6.2 Flash and verify blink app on STM32F103 BluePill over UART.
- [x] 6.3 Document board config, pin maps (PA9/PA10), and known limitations.
