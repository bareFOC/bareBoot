# Proposal: `stm32h503-i3c-16kb-layout`

## Why
Fitting `bareboot-stm32h503-i3c` into a single 8 KiB sector (`Sector 0`, `8,036 / 8,192` bytes) leaves only `156` bytes of headroom, constraining future diagnostics, multi-drop/CCC features, and safe refactoring. Allocating **16 KiB (Sectors 0–1, `0x0800_0000..0x0800_3FFF`)** for `bareBoot` and moving `APP_BASE` to **Sector 2 (`0x0800_4000`)** provides ample bootloader headroom while leaving **88 KiB (`0x0800_4000..0x0801_9FFF`)** for applications (`FishyServo` is `~75 KiB`).

## What Changes
- `targets/stm32h503-i3c/memory.x`: Set bootloader `FLASH` `LENGTH = 16K` (`0x0800_0000..0x0800_4000`).
- `targets/stm32h503-i3c/src/main.rs`:
  - Update `APP_BASE = 0x0800_4000` (Sectors 2–12, `88 KiB`).
  - Update Option-Byte WRP guard to protect **both Sector 0 and Sector 1** (`wrp_cur & 0x03 == 0` -> `wrp_cur & !0x03`) alongside `BOR = 3`.
- `host/scripts/flash_app_over_i3c_usb.py` & tests: Update default `app_base` (`0x08004000` for STM32H503 16 KiB bootloader, while still honoring `CMD_CONNECT` reported `app_base`).
- Rebuild `artifacts/bareboot-stm32h503-i3c-rust.bin`.
