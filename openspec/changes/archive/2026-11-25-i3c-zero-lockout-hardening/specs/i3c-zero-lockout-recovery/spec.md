# i3c-zero-lockout-recovery Specification

## Purpose

Guarantees non-brickable 2-wire (`SDA + SCL`) firmware recovery and bus resilience for MIPI I3C BareBoot and BareFOC targets without requiring SWD, `BOOT0`, or a hardware reset pin in production.

## ADDED Requirements

### Requirement: Alignment-Generic Monotonic Commit Journal with Full-Image CRC32
The `bareboot-core` crate SHALL provide an alignment-parameterized `MonotonicCommitJournal` (`4`, `8`, `16`, or `32`-byte write quanta) that records `IN_PROGRESS` before any application sector erase/program and `VALID(app_len, app_crc32)` only upon `CMD_COMPLETE`.

#### Scenario: Torn flash after Sector 1 write
- **WHEN** power is lost or the transfer aborts after Sector 1 (`0x0800_2000`) has been written with a valid vector table but before `CMD_COMPLETE`
- **THEN** the commit journal remains `IN_PROGRESS` and BareBoot parks permanently in I3C recovery mode on reset

#### Scenario: Application flash bit-rot detected at boot
- **WHEN** one or more bytes in `[APP_BASE, APP_BASE + app_len)` are corrupted after a valid commit
- **THEN** boot-time CRC32 verification fails and BareBoot parks permanently in I3C recovery mode

### Requirement: Hardware Watchdog and 3-Strike Boot-Loop Trap
BareBoot and the application SHALL run a hardware `IWDG` (~2.0 s) and maintain a CRC32-protected `BootLoopGuard` counter in `.uninit` SRAM.

#### Scenario: Repeated app panic, HardFault, or watchdog hang
- **WHEN** an application crashes, HardFaults, or hangs before clearing `boot_attempts` three consecutive times
- **THEN** BareBoot detects `boot_attempts >= 3` and parks permanently in I3C recovery mode

### Requirement: 1500 ms Cold-Boot Recovery Window
On cold boot with a valid committed image and `boot_attempts < 3`, BareBoot SHALL keep `I3C2` listening in `ENTDAA`-ready recovery mode for `1500 ms`, cancelling the auto-boot timer immediately upon `ENTDAA` or valid bootloader/reset traffic.

#### Scenario: Uncooperative app caught over 2-wire bus
- **WHEN** the installed application disables `I3C2` or omits handsfree re-entry logic and the board is power-cycled
- **THEN** the host discovers BareBoot via `ENTDAA` within `1500 ms` and flashes a healthy application over `SDA + SCL`

### Requirement: Hardware I3C Target Reset Pattern and Bus Error Recovery
BareBoot and `Stm32I3cTarget` SHALL handle the hardware I3C Target Reset Pattern (`EVR_RSTF`) and `CCC RSTACT`, and BareBoot SHALL clear `rx_len` and `I3C2_SER` on `EVR_ERRF` or invalid Katapult frames.

#### Scenario: Host escapes wedged app via Target Reset Pattern
- **WHEN** the host drives the MIPI I3C Target Reset Pattern (`SDA` pulses while `SCL` is low)
- **THEN** the target clears `BootLoopGuard` / sets `SoftResetMailbox` and resets into BareBoot recovery

#### Scenario: Bus noise or truncated frame during OTA
- **WHEN** truncated frames, corrupted CRC16s, or `I3C2_SER` framing errors arrive on `SDA + SCL`
- **THEN** BareBoot discards the bad buffer state and immediately accepts subsequent `CMD_CONNECT` / OTA commands

### Requirement: MultiBus I3C Target `preload_read()` Correctness
`Stm32I3cTarget::preload_read()` in `barefoc-stm32` SHALL clamp read prefetch lengths to `extent.saturating_sub(ptr).min(READ_WINDOW)` and arm `TGTTDR` without resetting `I3C2_CFGR.EN` mid-transaction.

#### Scenario: Back-to-back MultiBus register writes/reads and end-of-map reads
- **WHEN** the host issues zero-delay back-to-back `private_write` -> `private_read` / `private_write_read` transfers or reads registers near `map.extent()`
- **THEN** all transfers succeed without `ERR_NAK_ADDRESS` or empty `TGTTDR` underflows

#### Scenario: Stuck-SDA bus-wedge recovery and watchdog gating
- **WHEN** `PB4` (`I3C2_SDA`) remains continuously low (`0`) for `>= 50 ms` without bus event progress
- **THEN** BareBoot and `Stm32I3cTarget` pulse `RCC_APB3RSTR.I3C2RST` (bit 9) and re-initialize `I3C2` to release the line; if `SDA` remains wedged, the application withholds `IWDG` feeds so hardware reset recovers into BareBoot within ~2.0 s

### Requirement: Bootloader Write Protection and Non-Volatile Calibration Retention
STM32H503 Sector 0 (`0x0800_0000..0x0800_2000`) SHALL be protected via hardware flash `WRP` (with `BOR` Level 3 enabled, enforced by both factory SWD flashing and BareBoot), and Sectors 14–15 (`0x0801_C000..0x0802_0000`) SHALL remain bit-exact across all OTA and recovery cycles.

#### Scenario: App attempts to erase or write Sector 0
- **WHEN** application code attempts a flash erase/program on `0x0800_0000`
- **THEN** hardware `WRP` blocks the write (`WRPERR`), leaving BareBoot intact and recoverable over I3C
