# bareboot-hal Specification

## Purpose

TBD - created by archiving change bareboot-hal-platform-traits. Update Purpose after archive.

## Requirements

### Requirement: Decoupled `BootPlatform` & `ArchBoot` Traits in `bareboot-core`
`bareboot-core` SHALL provide `#![no_std]`, zero-heap static-dispatch traits (`FlashRegion` / `NorFlash` adapter, `ArchBoot`, `TickTimer`, `BusTransport`, `BootPlatform`) and a generic `BootRunner` state machine that validates the application vector + CRC32, manages `BootLoopGuard` and `boot_window_ms`, dispatches Katapult frames, and performs `ArchBoot::jump_to_app()`.

#### Scenario: Host unit tests verify `BootRunner` with a mock `BootPlatform`
- **WHEN** `cargo test -p bareboot-core` runs on the host with a mock `BootPlatform`
- **THEN** `BootRunner` handles `boot_window_ms` expiry, `BootLoopGuard` 3-strike trapping, `CMD_BLOCK_WRITE`/`CMD_RX_EOF`/`CMD_COMPLETE` flashing, and `ArchBoot::is_valid_pc()` for both ARM Thumb and RISC-V targets

### Requirement: `stm32h503-i3c` Pilot Migration Fits Within 16 KiB
`targets/stm32h503-i3c` SHALL implement `BootPlatform` using `bareboot-arch-cortexm` and `BootRunner` while remaining `<= 16,384` bytes in Sectors 0–1.

#### Scenario: Release binary size check after `BootRunner` migration
- **WHEN** `bareboot-stm32h503-i3c` is compiled in `--release` with `BAREBOOT_SAFE_PINS`
- **THEN** the resulting `.bin` is `<= 16,384` bytes and passes `cargo test -p bareboot-core -p bareboot-protocol`
