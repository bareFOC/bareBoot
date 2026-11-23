# Proposal

## Why

When an application flash update is aborted or interrupted mid-stream on STM32 targets (e.g. STM32H503), the bootloader's existing `application_check_valid()` evaluates only the first two 32-bit words (Initial SP and Reset PC). If the first block was written, SP and PC appear within valid bounds, fooling the bootloader into jumping into an unpopulated or truncated application slot. Because the code at the Reset PC is unprogrammed (`0xFFFFFFFF`), the CPU executes an invalid instruction and traps in an unrecoverable `HardFault` loop, taking down the I2C/I3C bus slave and locking out the device.

Furthermore, `commit_state_valid()` is currently stubbed as `return 1;` on all non-RP2350 platforms, leaving STM32 targets without two-phase commit verification. This change provides deep vector table and entry opcode validation in the bootloader, enables commit state verification for STM32, and enforces inverted vector table programming in host flashtool scripts.

## What Changes

- **Entry Opcode Sanity Validation**: Extend `application_check_valid()` and `validate_app_vector()` to dereference `pc & ~1` and verify the first instruction opcode is neither `0xFFFFFFFF` (unprogrammed flash) nor `0x00000000` (zeroed flash).
- **Multi-Vector Table Verification**: Verify that the ARM Cortex-M `HardFault_Handler` vector (`app[3]`) is programmed (not `0xFFFFFFFF`) with the Thumb bit set (`bit 0 == 1`).
- **Universal Commit State Enforcement on STM32**: Implement `commit_state_valid()`, `commit_state_begin()`, and `commit_state_commit()` for STM32 targets using dedicated flash storage (such as Sector 15 / high NVM), preventing auto-boot if a transfer was interrupted while in `UPDATE_IN_PROGRESS`.
- **Inverted Flashtool Programming Order**: Update host flashtool algorithms (`flashtool.py` and Python update scripts) to flash application code blocks first and write the vector table block (offset 0..64) last, ensuring an interrupted flash always leaves SP/PC unprogrammed (`0xFFFFFFFF`).

## Capabilities

### Modified Capabilities
- `katapult-single-slot-engine`: Extend commit state tracking and failsafe vector verification to validate entry opcodes and exception vectors across all architectures including STM32.
- `stm32h503-i2c-bootloader`: Update boot decision logic to permanently trap in bootloader recovery mode when entry opcodes are blank or commit state is unverified.

## Impact

- Affected files in `bareBoot`: `crates/bareboot-core/src/lib.rs`, `ref/katapult/src/generic/armcm_canboot.c`, `ref/katapult/src/commit_state.h`, `targets/stm32h503/src/bootentry.c`, `host/scripts/`.
- No breaking protocol wire changes: wire commands (`CONNECT`, `SEND_BLOCK`, `COMPLETE`) remain 100% Katapult protocol compatible.
