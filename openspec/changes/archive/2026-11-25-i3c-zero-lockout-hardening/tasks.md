## 1. Cross-MCU Core Hardening (`bareBoot/crates/bareboot-core`)

- [x] 1.1 Generalize `MonotonicCommitJournal` in `crates/bareboot-core/src/commit.rs` for configurable write alignment (`4 | 8 | 16 | 32` bytes) and `CommitRecord { magic, state, app_len, app_crc32 }`, preserving `#![no_std]` and zero-heap constraints.
- [x] 1.2 Update `PagedWriter` in `crates/bareboot-core/src/writer.rs` to track written image length and incremental IEEE 802.3 `CRC32`, calling `mark_in_progress` on first block and `mark_valid(app_len, app_crc32)` on `COMPLETE` (`dispatch.rs`).
- [x] 1.3 Add `BootLoopGuard` (CRC32-checked `.uninit` SRAM counter, threshold 3) and `crc32` helper to `crates/bareboot-core/src/lib.rs`, plus comprehensive host unit tests (`cargo test --workspace`).

## 2. Pure-Rust STM32H503 I3C BareBoot Hardening (`bareBoot/targets/stm32h503-i3c`)

- [x] 2.1 Update flash partition constants (`APP_BASE = 0x0800_2000`, `APP_END = 0x0801_A000`, `COMMIT_BASE = 0x0801_A000`, `COMMIT_SIZE = 8192`, `NVM_BASE = 0x0801_C000`) and reserve `0x2000_7FD0..0x2000_7FE0` for `BootLoopGuard` (`_stack_start = 0x2000_7FD0`).
- [x] 2.2 Integrate `MonotonicCommitJournal<8192, 16>`, boot-time `CRC32` verification, `BootLoopGuard` 3-strike trap, `BOOT_WINDOW_MS = 1500`, `IWDG` (~2.0 s) + `WatchdogFlash`, and self-reset panic handler (no `panic_halt`).
- [x] 2.3 Inspect `i3c_v1.rs` / STM32H5 RM0492 and add `EVR_RSTF` (Target Reset Pattern), `EVR_ERRF` (`I3C2_SER`) + malformed-frame `rx_len = 0` recovery, `CCC RSTDAA (0x06)` & `CCC SETDASA (0x87)` handling.
- [x] 2.4 Build release binary and verify exact size `<= 8192` bytes (`bareboot-stm32h503-i3c-rust.bin`), updating `artifacts/bareboot-stm32h503-i3c-rust.bin`.
- [x] 2.5 Implement/verify Sector 0 `WRP` (`WRPSG1R` bit 0) + `BOR` (~2.7 V) option-byte check/configuration (keeping `RDP = 0xAA` open for bench verification).

## 3. BareFOC I3C Target & FishyServo App Hardening (`bareFOC`)

- [x] 3.1 Fix `Stm32I3cTarget::preload_read()` in `barefoc-hal/barefoc-stm32/src/bus_target_i3c.rs`: clamp read prefetch length to `extent.saturating_sub(ptr).min(READ_WINDOW)` (and handle short-read pointer tracking cleanly), and remove unsafe `I3C2_CFGR.EN = 0 -> 1` reset races using `i3c_v1.rs`-verified TX FIFO / `TGTTDR` handling.
- [x] 3.2 Add `EVR_RSTF` (Target Reset Pattern) and `CCC RSTACT` (`0x2A`/`0x9A`/`0xAA`, `0x02`) escape handling in `Stm32I3cTarget::service_once()` -> `barefoc_failsafe_recovery()`.
- [x] 3.3 Update `examples/fishyservo_stm32h5` (`memory.x`, `build.rs`, `Cargo.toml`, `src/main.rs`): set bootloader app `FLASH LENGTH = 96K` and `_stack_start = 0x2000_7FD0`, enable `bareboot` feature + `target.enable_bootloader(...)`, feed `IWDG`, and clear `BootLoopGuard` after 500 ms of healthy execution.
- [x] 3.4 Run host tests (`cargo test` in `barefoc-hal`), rebuild `fishyservo_stm32h5_i3c.bin`, and verify replugged `TMAG5273` on `I2C1` (`PB6/PB7`) + zero-delay MultiBus reads/writes on live FishyServo hardware.

## 4. 2-Wire Production Lockout Stress-Test Suite & Verification

- [x] 4.1 Update `FishyServo/test-scripts/verify_fishyservo_i3c.py` (remove any 2 ms write-settle workaround if resolved by `bus_target_i3c.rs`, verify end-of-map reads and `TMAG5273` angle tracking).
- [x] 4.2 Create `bareBoot/host/scripts/test_i3c_lockout_hardware.py` implementing real-hardware scenarios `S1–S8` (all recovery/discovery/flashing strictly over `SDA + SCL` via `AgentLaI3c`; SWD `/dev/hidraw5` used solely for fault/power-loss injection and post-mortem verification).
- [x] 4.3 Flash hardened `bareboot-stm32h503-i3c-rust.bin` to Sector 0 once via `flash_bareboot_swd.py`, run `test_i3c_lockout_hardware.py` (`S1–S8`) and `verify_fishyservo_i3c.py`, and record all pass/fail results.
- [x] 4.4 Sync specs, update `bareBoot` / `barefoc.github.io` documentation if applicable, and report observed results.
