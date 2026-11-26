# Proposal: `bareboot-hal-platform-traits`

## Why
Currently `bareboot-core` mixes flash I/O with CPU jump/UID concerns (`FlashTarget`), `validate_app_vector()` hardcodes the ARM Cortex-M Thumb bit, `SoftResetMailbox::ADDRESS` hardcodes STM32H503's `0x2000_7FE0`, and every MCU target (`targets/stm32h503-i3c`, `targets/rp2350-usb`, `targets/ch32v203-usb`) duplicates ~250 lines of boot-window, `BootLoopGuard`, CRC32, `boot_jump()`, and protocol dispatch loops. Refactoring `bareBoot` around idiomatic `#![no_std]` static-dispatch HAL traits and `embedded-storage::nor_flash::NorFlash` makes adding new MCUs (`STM32G4`, `STM32F4`, `RP2350`, `CH32V`) clean while keeping target binaries `<= 16 KiB`.

## What Changes
- **`crates/bareboot-core`**:
  - Decouple pure flash storage (`FlashRegion` / `embedded_storage::nor_flash::NorFlash` adapter) from `ArchBoot` (`jump_to_app`, `system_reset`, `is_valid_pc`), `TickTimer` (`poll_1ms`), `BusTransport` (`poll_rx`, `send_tx`, `tx_idle`), and `BootPlatform` (composite BSP trait).
  - Generalize `validate_app_vector()` via `ArchBoot::is_valid_pc()` so ARM Thumb (`pc & 1 == 1`) and RISC-V (`pc & 1 == 0`) both work without `#![cfg]` hacks.
  - Extract `verify_app_crc32()` and `BootRunner<P: BootPlatform, const PAGE: usize, const COMMIT_SIZE: usize, const COMMIT_ALIGN: usize>` into `bareboot-core` so targets share one tested, zero-heap, monomorphized state machine.
- **`crates/bareboot-arch-cortexm` (new tiny crate)**:
  - Shared `CortexMArch`: `SysTick` disable, `NVIC` clear, `VTOR` relocation, `dsb`/`isb`, `MSP` + entry jump, and `SCB::AIRCR` system reset.
- **`targets/stm32h503-i3c` (pilot migration)**:
  - Migrate `stm32h503-i3c` to implement `BootPlatform` + `BootRunner` and verify the release `.bin` remains well within the `16 KiB` (`16,384` B) Sectors 0–1 slot.

## Impact
- Shrinks board target crates from ~800 lines to thin BSP implementations (~150–250 lines).
- Zero `alloc`, zero `dyn Trait` vtables, and full host unit-testability of `BootRunner` using mock `BootPlatform` implementations.
