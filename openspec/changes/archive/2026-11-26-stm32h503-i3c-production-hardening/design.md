# Design: `stm32h503-i3c-production-hardening`

## Sector 0 Size Budget (`<= 8,192` bytes)
STM32H503 flash sectors are 8 KiB each (`16 x 8 KiB = 128 KiB`). Keeping `bareboot-stm32h503-i3c` inside Sector 0 (`0x0800_0000..0x0800_1FFF`) preserves `APP_BASE = 0x0800_2000` (96 KiB app slot + 24 KiB CommitStore/NVM). To make room within `8,192` bytes for peripheral teardown, NACK framing, and UID addressing, `I3C2` GPIO/RCC setup and `I3C2_TIMINGR` initialization are consolidated into a single helper used by both cold boot and `reset_i3c2()`.

## Watchdog & BootLoopGuard Contract
- **No forced `IWDG` on cold POR**: Read `RCC_RSR` at boot; only call `iwdg_start()` if `IWDGRSTF` is already set or once the bootloader enters parked/flashing mode (not during the 250 ms pass-through boot window). If `IWDG` is active, call `iwdg_feed()` immediately prior to `boot_jump()`.
- **Mailbox vs Legacy Cookie**: Evaluate `SoftResetMailbox::is_valid()` at `0x2000_7FE0` first; only inspect `LEGACY_COOKIE_ADDR` (`0x2000_7FF8`) when the mailbox CRC/magic is invalid so `SoftResetMailbox.crc32` is never clobbered.

## Clean `boot_jump()` Handoff
1. Verify `app_vector_is_valid(APP_BASE)` before jumping (including after `CMD_COMPLETE`).
2. Disable `SysTick` (`SYST_CSR = 0`, `SYST_CVR = 0`).
3. Reset `I3C2` via `RCC_APB3RSTR` and return `PB3`/`PB4` to analog/input (`MODER = 0b11`).
4. Feed `IWDG` if active, set `SCB_VTOR = APP_BASE`, set `MSP`, and jump.

## Multi-Drop `EPIDR` & Static Address
- When `SoftResetMailbox` is absent/invalid, derive the static address from `bareboot_protocol::fallback_address(&uid)` (or keep `0x40` as `ENTDAA` proposed address when `ENTDAA` assigns dynamically).
- Pack `uid[0..4]` bits into `I3C2_EPIDR` `[11:0]` alongside `MIPIID` so `ENTDAA` provisional IDs do not collide across 4+ boards.
