## Why

In production FishyServo deployments, SWD (`PA13`/`PA14`), `BOOT0`, and hardware `NRST` pins are inaccessible — only power and the 2-wire MIPI I3C bus (`PB3` SCL / `PB4` SDA) are exposed. Any bootloader or bus-interface failure that prevents I3C recovery permanently bricks the unit.

Audit of `targets/stm32h503-i3c`, `crates/bareboot-core`, and `barefoc-stm32/src/bus_target_i3c.rs` uncovered multiple 2-wire lockout risks:
1. **No torn-update / image integrity check in `stm32h503-i3c`**: `app_is_valid()` validates only two vector words (`SP`, `PC`) and the first opcode. A power loss after Sector 1 is programmed (or flash corruption in later sectors) causes BareBoot to jump into a broken application.
2. **No hardware watchdog or boot-loop trap**: Neither `stm32h503-i3c` nor `fishyservo_stm32h5` enables `IWDG`, `stm32h503-i3c` uses `panic_halt`, and repeated app panics/HardFaults leave only a tight `250 ms` cold-boot catch window.
3. **Missing hardware I3C escape & bus error recovery**: Neither `stm32h503-i3c` nor `bus_target_i3c.rs` handles `EVR_RSTF` (I3C Target Reset Pattern) or `CCC SETDASA`; `stm32h503-i3c` does not clear partial `rx_buf` state on `EVR_ERRF`/malformed frames; and `fishyservo_stm32h5` does not wire `CCC RSTACT` (`bareboot` feature).
4. **Cross-MCU commit divergence**: `bareboot-core::commit::MonotonicCommitJournal` assumes 4-byte writes, which causes `PGSERR`/ECC faults on 16-byte-aligned STM32H5 ECC flash (and 8-byte/32-byte STM32G4/H7 flash), preventing a single shared implementation across BareBoot MCUs.
5. **`bus_target_i3c.rs` `preload_read()` race & unclamped read window**: `preload_read()` requests `extent().min(READ_WINDOW)` instead of clamping to remaining bytes from `ptr`, and toggles `I3C2_CFGR.EN = 0 -> 1` on every phase, causing `ERR_NAK_ADDRESS` on back-to-back host transfers.

## What Changes

- **Cross-MCU Commit & Boot-Loop Protection (`crates/bareboot-core`)**:
  - Generalize `MonotonicCommitJournal` for configurable write alignment (`4 | 8 | 16 | 32` bytes) storing `(magic, state, app_len, app_crc32)` without changing the Katapult wire protocol (`PagedWriter` tracks `bytes_written` and incremental IEEE 802.3 `CRC32`).
  - Add `BootLoopGuard` (CRC32-protected `.uninit` SRAM counter, 3-strike trap into recovery) and `WatchdogFeed` support.
- **Pure-Rust STM32H503 I3C BareBoot Hardening (`targets/stm32h503-i3c`)**:
  - Partition 128 KB flash as Sector 0 (`8 KB` BareBoot, WRP-protected), Sectors 1–12 (`96 KB` App), Sector 13 (`8 KB` `CommitStore`), Sectors 14–15 (`16 KB` `H5FlashStorage` NVM).
  - Gate app boot on `REQUEST_BOOTLOADER` mailbox, `BootLoopGuard` (`< 3`), `validate_app_vector`, `CommitState::Valid`, and full-image `CRC32` verification.
  - Increase `BOOT_WINDOW_MS` to `1500 ms`, enable `IWDG` (~`2.0 s`), replace `panic_halt` with self-reset, handle `EVR_RSTF` / `EVR_ERRF` / `CCC RSTDAA` / `CCC SETDASA`, and add Sector 0 `WRP` + `BOR` protection helpers.
- **Application I3C Target & Watchdog Integration (`barefoc-stm32/src/bus_target_i3c.rs`, `fishyservo_stm32h5`)**:
  - Fix `preload_read()` extent clamping and remove unsafe `I3C2_CFGR.EN` reset races during active transactions.
  - Handle `EVR_RSTF` (Target Reset Pattern) and `CCC RSTACT` (`0x2A`/`0x9A`/`0xAA`, `0x02`) in `Stm32I3cTarget::service_once()` -> `barefoc_failsafe_recovery()`.
  - Feed `IWDG` and clear `BootLoopGuard` after 500 ms of healthy execution in `fishyservo_stm32h5`; verify replugged `TMAG5273` on `I2C1`.
- **Real-Hardware 2-Wire Stress Suite (`host/scripts/test_i3c_lockout_hardware.py`)**:
  - Implement scenarios `S1–S8` against live FishyServo where **only `SDA + SCL`** (`AgentLaI3c`) performs discovery, recovery, and flashing, while SWD is used strictly to inject faults/power-cycle resets and inspect post-conditions.

## Capabilities

### New Capabilities
- `i3c-zero-lockout-recovery`: Hardware-verified 2-wire (`SDA + SCL`) recovery guarantees for BareBoot and BareFOC over MIPI I3C, including alignment-generic `CommitStore` + boot-time `CRC32`, `IWDG` + 3-strike boot-loop trapping, `EVR_RSTF`/`CCC RSTACT` out-of-band escape, bus error clearing, and Sector 0 `WRP`.

### Modified Capabilities
- `i3c-dynamic-bootloader`: Clarifies `EVR_RSTF` Target Reset Pattern, `EVR_ERRF` buffer reset, `1500 ms` boot window, and `CommitStore` + `CRC32` boot gating.

## Impact

- `bareBoot/crates/bareboot-core`: `commit.rs`, `writer.rs`, `lib.rs` (cross-MCU compatible, `#![no_std]`)
- `bareBoot/targets/stm32h503-i3c`: `src/main.rs`, `memory.x`
- `bareBoot/host/scripts`: `test_i3c_lockout_hardware.py` (new), `flash_app_over_i3c_usb.py` (as needed)
- `bareFOC/barefoc-hal/barefoc-stm32`: `src/bus_target_i3c.rs`
- `bareFOC/examples/fishyservo_stm32h5`: `src/main.rs`, `Cargo.toml`, `memory.x`, `build.rs`
- `FishyServo/test-scripts/verify_fishyservo_i3c.py`: verify zero-delay I3C MultiBus R/W and TMAG5273 angle tracking
- Katapult wire protocol: **unchanged**
