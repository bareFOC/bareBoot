## Context

Production FishyServo boards expose only power (`VDD`/`GND`) and the 2-wire I3C bus (`PB3` SCL / `PB4` SDA). Because backward compatibility with pre-existing STM32H503 images is not required, we can design the cleanest architecture across all `bareboot-core` targets (`STM32H5`, `STM32F1/F4/G0/G4/H7`, `RP2350`, `CH32V203`) while fitting the STM32H503 bootloader strictly within Sector 0 (`8192` bytes at `0x0800_0000`).

## Cross-MCU Design (`bareboot-core`)

### 1. Alignment-Generic `MonotonicCommitJournal<SECTOR_SIZE, SLOT_ALIGN>` + `CRC32`
Instead of duplicating `commit_state` per MCU target, `crates/bareboot-core/src/commit.rs` provides a single journal parameterized by flash programming alignment:
- `SLOT_ALIGN = 16` for STM32H5 (128-bit ECC quad-word; also works on 4/8-byte MCUs), or `SLOT_ALIGN = 32` for STM32H7.
- Each slot holds a fixed `16`-byte record (padded to `SLOT_ALIGN` if `> 16`):
  - `magic: u32 = 0x4242_5354` (`"BBST"`)
  - `state: u32` (`0x4242_0001` = `IN_PROGRESS`, `0x4242_0002` = `VALID`)
  - `app_len: u32` (exact image byte length written to `[APP_BASE, APP_BASE + app_len)`)
  - `app_crc32: u32` (IEEE 802.3 CRC32 over `APP_BASE .. APP_BASE + app_len`)
- `PagedWriter` tracks `written_len` and computes incremental `app_crc32` (over the padded page contents actually programmed into flash) with zero changes to Katapult framing.
- On the first `RX_BLOCK`, `PagedWriter` emits `IN_PROGRESS`. On `COMPLETE` (after `finish()` flushes the last partial page), `PagedWriter` writes `VALID(app_len, app_crc32)`.
- Scanning the sector on boot reads `SLOT_ALIGN`-spaced records until the first erased `0xFFFF_FFFF` slot; any non-matching magic or torn slot transitions to `CommitState::Invalid`. With an 8 KB Sector 13 on STM32H503, 16-byte slots yield **512 updates** before a sector erase is ever needed.

### 2. `BootLoopGuard` (`.uninit` SRAM, CRC32-checked)
- A 16-byte struct at `0x2000_7FD0..0x2000_7FE0` (immediately below `SoftResetMailbox` at `0x2000_7FE0` and legacy cookie `0x2000_7FF8`):
  - `magic: u32 = 0x4242_4C50` (`"BBLP"`)
  - `attempts: u32`
  - `reserved: u32`
  - `crc32: u32`
- On boot: if valid and `attempts >= 3`, BareBoot parks permanently in recovery mode (cancels boot window).
- Before `boot_jump()`: BareBoot increments `attempts` (or initializes `attempts = 1`) and updates `crc32`.
- After the app survives 500 ms of healthy execution (with `I3C2` and `motor_task` running), the app clears `attempts = 0` with valid `crc32`.

## STM32H503 Memory & Hardware Design (`targets/stm32h503-i3c`)

### Flash Map (128 KB STM32H503KB)
- `0x0800_0000 .. 0x0800_2000` (Sector 0, 8 KB): `BareBoot` (hardware write-protected via `FLASH_WRP`)
- `0x0800_2000 .. 0x0801_A000` (Sectors 1–12, 96 KB): Application slot (`fishyservo_stm32h5_i3c.bin` ~75.2 KB)
- `0x0801_A000 .. 0x0801_C000` (Sector 13, 8 KB): `CommitStore` monotonic journal
- `0x0801_C000 .. 0x0802_0000` (Sectors 14–15, 16 KB): `H5FlashStorage` NVM (untouched by BareBoot)

### Watchdog (`IWDG`), Boot Window, & Code Size Budget
- `BOOT_WINDOW_MS = 1500` ms on cold boot.
- `IWDG` (`0x4000_3000`): key `0xCCCC` start, `0x5555` unlock, prescaler `/32` (1 kHz LSI), reload `2000` (~2.0 s). Fed every main loop tick and inside `erase_page` / `write_page_bytes` via `WatchdogFlash`.
- Note: Sector 0 is **8192 bytes** (`BareBoot` currently 6888 bytes = 1304 bytes free). Keep software CRC32 compact (bit-by-bit loop, ~40 bytes) and avoid `core::fmt` / panicking formatting so release binary stays `<= 8192` bytes.

### I3C Resilience (`BareBoot` & `bus_target_i3c.rs`)
1. **`EVR_RSTF` (Target Reset Pattern)**:
   - On STM32H5 `I3C_EVR`, bit 15 (`RSTF` — verify exact bit in `i3c_v1.rs`!) signals the MIPI I3C Target Reset Pattern (14+ SDA transitions while SCL is low).
   - In BareBoot: `RSTF` resets `I3C2` state (`rx_len = 0`, `tx_len = 0`, `DEVR0 &= !1`), clears `I3C2_CEVR`, and cancels `boot_window_active` so host can immediately run `ENTDAA` + `CMD_CONNECT`.
   - In `bus_target_i3c.rs` (when `bareboot` feature enabled): `RSTF` or `CCC RSTACT` (`0x2A`/`0x9A`/`0xAA` with `0x02`) immediately invokes `barefoc_failsafe_recovery()`.
2. **`EVR_ERRF` & Malformed Frames**:
   - In BareBoot: clear `EVR_ERRF` (`I3C2_CEVR`), read/clear `I3C2_SER`, and reset `rx_len = 0` on bus errors or `FrameParse::Invalid` so corrupted/partial bytes never accumulate in `rx_buf`.
3. **`bus_target_i3c.rs` `preload_read()` Fix**:
   - Check `i3c_v1.rs` for STM32H5 `I3C_CFGR` / `I3C_CR` / `I3C_TGTTDR` TX FIFO flush semantics.
   - Clamp `len` to `extent.saturating_sub(ptr).min(READ_WINDOW)` so reads near the end of the MultiBus map never fail with `Error::Unmapped` -> `TGTTDR = 0`.
   - Avoid disabling `I3C2_CFGR.EN` mid-bus-activity (preload only when needed / flush cleanly without dropping `I3C2` address ACK across back-to-back host requests).
4. **Option Bytes (`WRP` + `BOR`)**:
   - Provide an explicit, opt-in option-byte check/programming utility (or BareBoot boot-time check) verifying `BOR_LEV` (2.7 V) and Sector 0 `WRP` (`WRPSG1R` bit 0) without locking out SWD (`RDP = Level 0` / `0xAA`) during bench verification.

## Risks & Trade-offs
- **8 KB Sector 0 ceiling**: Adding `MonotonicCommitJournal`, `CRC32`, `BootLoopGuard`, `IWDG`, and `EVR_RSTF` must fit within `8192` bytes of Sector 0; verify exact `.bin` size via `cargo build --release` and assert `<= 8192`.
- **App slot reduced 104 KB -> 96 KB**: Current `fishyservo_stm32h5_i3c.bin` is `75,208` bytes (~20.8 KB headroom). Update `memory.x` / `build.rs` in `fishyservo_stm32h5` so `FLASH LENGTH = 96K` (`0x0800_2000..0x0801_A000`) and `RAM` reserves `0x2000_7FD0..0x2000_8000` (`_stack_start = 0x2000_7FD0`).
- **STM32H5 `IWDG` persists across warm reset**: Once started by BareBoot, `IWDG` keeps running after `boot_jump()`, so any test app (including S2 "uncooperative app" in the stress suite) will trigger `IWDG` if it doesn't feed it — which accurately reflects production safety!
