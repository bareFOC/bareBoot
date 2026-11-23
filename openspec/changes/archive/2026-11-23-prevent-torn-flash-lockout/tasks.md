# Tasks

## 1. Vector and Entry Opcode Validation

- [x] 1.1 Extend `validate_app_vector` in `crates/bareboot-core/src/lib.rs` to accept an entry opcode slice and verify `*entry != 0xFFFFFFFF` and `*entry != 0x00000000`, verified via host unit tests.
- [x] 1.2 Add vector sanity check to `ref/katapult/src/generic/armcm_canboot.c` verifying `*entry_code != 0xFFFFFFFF` and `app[3] != 0xFFFFFFFF`, verified by compiling `stm32h503` target binaries.
- [x] 1.3 Add test scenario in `host/scripts/test_production_lockout_scenarios.py` verifying that an image with valid SP and PC but erased entry code is trapped in bootloader recovery mode.

## 2. Inverted Flashtool Block Ordering

- [x] 2.1 Update `host/scripts/flashtool.py` to write Block 0 (the vector table) last, after all payload blocks have been transmitted and verified.
- [x] 2.2 Verify with a simulated abort after block 1 that the target's initial vector remains unprogrammed (`0xFFFFFFFF`).

## 3. Universal Commit State Support for STM32

- [x] 3.1 Implement `commit_state_valid()`, `commit_state_begin()`, and `commit_state_commit()` for STM32 in `ref/katapult/src/commit_state.c` using high NVM flash sectors.
- [x] 3.2 Update `targets/stm32h503` patch set and recompile `artifacts/bareboot-stm32h503-i3c-i2c.bin`.
- [x] 3.3 Verify full end-to-end flash and torn recovery on physical hardware.
