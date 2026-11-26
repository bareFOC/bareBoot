# stm32h503-i3c Specification

## Purpose

TBD - created by archiving change stm32h503-i3c-production-hardening. Update Purpose after archive.

## Requirements

### Requirement: Clean `boot_jump()` Peripheral Handoff & Validation
`bareboot-stm32h503-i3c` SHALL validate `app_vector_is_valid(APP_BASE)`, disable `SysTick`, reset `I3C2` via `RCC_APB3RSTR`, feed `IWDG` if active, and fit within Sector 0 (`<= 8,192` bytes) before jumping to `APP_BASE`.

#### Scenario: Post-`CMD_COMPLETE` jump validates vector table and cleans up peripherals
- **WHEN** `CMD_COMPLETE` succeeds and `boot_jump(APP_BASE)` is invoked
- **THEN** `app_vector_is_valid(APP_BASE)` is verified, `SysTick` is disabled, `I3C2` is reset, and `IWDG` is fed if active

### Requirement: Mailbox Ordering, NVM Readback Isolation & Protocol NACK
`bareboot-stm32h503-i3c` SHALL check `SoftResetMailbox` before `LEGACY_COOKIE_ADDR`, clamp `read_bytes` to `[APP_BASE, APP_END)`, and encode `responses::NACK` (`0xF1`) on `dispatch_one()` frame errors.

#### Scenario: Corrupted wire frame returns `NACK` (`0xF1`)
- **WHEN** `dispatch_one()` returns `Err` on a malformed or bad-CRC frame
- **THEN** the target stages `responses::NACK` (`0xF1`) in `I3C2_TGTTDR` instead of a 0-byte response

### Requirement: Host Script I3C SDR `0xF0` Reboot & NACK Codes
`flash_app_over_i3c_usb.py` SHALL use `RESP_NACK = 0xF1` / `RESP_CMD_ERR = 0xF2` and support I3C SDR `private_write([0xF0])` in addition to open-drain I2C `0xF0`.

#### Scenario: Flashing an app already running in I3C SDR mode
- **WHEN** `flash_app_over_i3c_usb.py` runs against an app in I3C SDR mode
- **THEN** it triggers `0xF0` over I3C SDR (`private_write`), connects to `bareBoot`, and flashes bit-exact

### Requirement: STM32H503 16 KiB Bootloader Slot & Dual-Sector WRP
`bareboot-stm32h503-i3c` SHALL reside in Sectors 0–1 (`0x0800_0000..0x0800_3FFF`, `16 KiB`), set `APP_BASE = 0x0800_4000` (`88 KiB` app slot up to `0x0801_A000`), and write-protect both Sector 0 and Sector 1 (`WRP[1:0] = 00`) with `BOR = 3`.

#### Scenario: `CMD_CONNECT` reports `0x08004000` and WRP protects Sectors 0–1
- **WHEN** `bareboot-stm32h503-i3c` boots and responds to `CMD_CONNECT`
- **THEN** `app_base` is `0x0800_4000`, `FLASH_WRPSGN1R` bits `[1:0]` are `00`, and writes below `0x0800_4000` are rejected
