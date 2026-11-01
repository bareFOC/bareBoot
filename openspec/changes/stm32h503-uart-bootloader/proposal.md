## Why

BareBoot already supports STM32F1 over UART, but there is no target for the STM32H503 family used by the "Miniata" board. The H503 is a newer, Cortex-M33 part with a different flash controller and pin mux, so the F1 target cannot be reused. Adding H503 support (UART first) lets the same Katapult-compatible host flow flash and boot firmware on this hardware, and a blink test verifies the handoff.

## What Changes

- New target crate `bareboot/targets/bareboot-stm32h503` (UART transport only) mirroring `bareboot-stm32f1`, adapted for STM32H503KBU6.
- New board config `bareboot-stm32h503.toml`: HSI 64 MHz, 128 KB flash, 32 KB RAM, `app_base = 0x08004000`, 3000 ms boot window, transports = ["uart"].
- USART1 on PB6 (TX) / PB7 (RX) at 115200 8N1, AF7, with an IRQ-driven RX ring buffer (required because 8 KB sector erase stalls the CPU).
- `FlashTarget` implementation using the H5 non-secure flash controller (unlock, 8 KB sector erase, 16-byte program, readback verify).
- Bootloader size budget of 16 KB (two erase sectors) ahead of `app_base`.
- Blink-test firmware variant under `test-firmware/` for STM32H503 (embassy-stm32, `memory.x` ORIGIN `0x08004000`, toggling LED_B on PB8).
- README device-matrix update: STM32H5 UART marked supported for the H503 family.

## Capabilities

### New Capabilities

- `stm32h503-uart-bootloader`: Katapult-compatible UART bootloader for STM32H503 (H503KBU6), covering USART1 transport, H5 flash programming, boot window, and application handoff.

### Modified Capabilities

- (none) — no existing spec requirements change; this is a new target family.

## Impact

- Affected code: `bareboot/` workspace members (new crate + Cargo workspace entry), `bareboot/targets/`, `test-firmware/`, `README.md`.
- Dependencies: `stm32-metapac` (already in registry, feature `stm32h503kb`), `cortex-m`, `cortex-m-rt`; blink test uses `embassy-stm32` (`stm32h503kb` feature).
- Hardware: requires probe-rs/ST-Link on SWD to flash the bootloader itself; WCH-LinkE UART bridge (PB6/PB7) for the host flow.
- Risk: H503KBU6 is 32-pin UFQFPN; PC14 (LED_R) may not be bonded. LED_B on PB8 is the verification LED for the blink test.