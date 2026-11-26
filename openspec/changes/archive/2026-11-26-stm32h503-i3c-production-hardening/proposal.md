# Proposal: `stm32h503-i3c-production-hardening`

## Why
A production-readiness audit of `targets/stm32h503-i3c` (`8,004 / 8,192` bytes in Sector 0) and `host/scripts/flash_app_over_i3c_usb.py` identified critical/high/medium risks before mass deployment:
1. `IWDG` is started unconditionally on every boot and left running across `boot_jump()` without a final feed, risking false resets on apps with >1.36 s init/calibration pauses.
2. `BootLoopGuard` records attempts unconditionally on every reset, so 3 rapid brownouts trap the node in bootloader recovery.
3. `boot_jump()` leaves `SysTick` enabled and `I3C2` unreset (`PB3`/`PB4` AF3), and the post-`CMD_COMPLETE` jump skips `app_vector_is_valid()`.
4. `DEFAULT_I3C_ADDR = 0x40` and 4-bit `EPIDR` UID cause multi-drop bus collisions instead of using `fallback_address(&uid)` and fuller UID bits in `EPIDR`.
5. `SoftResetMailbox` (`0x2000_7FE0..0x2000_7FFF`) overlaps `LEGACY_COOKIE_ADDR` (`0x2000_7FF8`).
6. Protocol errors return 0-length reads instead of `NACK` (`0xF1`), `Stm32H5Flash::read_bytes` allows readback above `APP_END` (`Sectors 13..15`), and `flash_app_over_i3c_usb.py` uses `RESP_NACK = 0xA1` and open-drain-only `0xF0`.

## What Changes
- Keep the **8 KB Sector 0 (`0x0800_0000..0x0800_1FFF`)** footprint (`<= 8,192` bytes) so `APP_BASE = 0x0800_2000` and all existing application linker scripts (`FishyServo`, `Miniata`) remain compatible.
- **`targets/stm32h503-i3c/src/main.rs`**:
  - Start `IWDG` only when `RCC_RSR` indicates `IWDGRSTF` was already active or when parked in bootloader flashing mode (never force `IWDG` onto an app that didn't opt in), and feed `IWDG` right before `boot_jump()` if active.
  - Increment `BootLoopGuard` only when jumping after a fault/watchdog reset or non-POR reset (preserving normal brownout/POR recovery), and check `SoftResetMailbox::is_valid()` **before** `LEGACY_COOKIE_ADDR`.
  - Cleanly disable `SysTick` (`SYST_CSR = 0`, `SYST_CVR = 0`), pulse `RCC_APB3RSTR` for `I3C2` (and reset `PB3`/`PB4` to analog/input), and verify `app_vector_is_valid()` before every `boot_jump()` (including post-`CMD_COMPLETE`).
  - Use `bareboot_protocol::fallback_address(&uid)` when `SoftResetMailbox` is absent/invalid (note: single-node test configs can still receive `0x40` via `ENTDAA`), and pack more UID bits into `I3C2_EPIDR`.
  - Encode `responses::NACK` (`0xF1`) on `dispatch_one()` `Err(...)`, and clamp `Stm32H5Flash::read_bytes` to `[APP_BASE, APP_END)`.
  - Deduplicate `I3C2` init/reset code so the release binary remains `<= 8,192` bytes in Sector 0.
- **`host/scripts/flash_app_over_i3c_usb.py`**:
  - Fix `RESP_NACK = 0xF1` / `RESP_CMD_ERR = 0xF2`, and send `0xF0` over **both** I3C SDR (`private_write`) and open-drain I2C fallback when triggering in-band reboot.

## Impact
- Hardens STM32H503 I3C bootloader handoff, multi-drop addressing, NVM readback isolation, and host flashing without changing `APP_BASE` (`0x0800_2000`).
