## 1. Setup

- [x] 1.1 Create `bareboot/targets/bareboot-stm32h503` crate skeleton (Cargo.toml, build.rs, src/main.rs, src/lib.rs) mirroring `bareboot-stm32f1`.
- [x] 1.2 Add `bareboot-stm32h503` to the workspace members in `bareboot/Cargo.toml`.
- [x] 1.3 Create `bareboot-stm32h503.toml` config (HSI 64 MHz, 128 KB flash, 32 KB RAM, app_base 0x08004000, 3000 ms, transports = ["uart"]).
- [x] 1.4 Size-budget spike: build the empty target and record the `.bin` size.

## 2. Core Implementation

- [x] 2.1 Implement `uart_init` and `uart_irq_enable` for USART1 on PB6/PB7 (AF7, 115200 @ 64 MHz).
- [x] 2.2 Implement the USART1 RX ring buffer and `uart_get_byte`.
- [x] 2.3 Implement `H503Flash: FlashTarget` using metapac PAC (unlock, 8 KB sector erase, 16-byte program, readback).
- [x] 2.4 Implement `read_uid` from the H5 UID register.
- [x] 2.5 Implement `boot_jump` (MSP from vector table at 0x08004000).
- [x] 2.6 Wire `main` loop: timing, frame scan, dispatch, I/O, boot window, handoff.

## 3. Verification Firmware

- [x] 3.1 Create `test-firmware/blink-h503` from `blink-test` (embassy-stm32 feature `stm32h503kb`, memory.x ORIGIN 0x08004000, toggle PB8).
- [x] 3.2 Build blink firmware and confirm it fits below 128 KB.

## 4. Flash and Verify

- [x] 4.1 Flash bootloader via probe-rs (SWD) and confirm UART output.
- [x] 4.2 Flash blink firmware at 0x08004000 via probe-rs.
- [x] 4.3 Run host flash flow over WCH-LinkE UART and confirm LED_B blinks after COMPLETE.
- [x] 4.4 Update README device matrix (STM32H5 UART supported for H503).