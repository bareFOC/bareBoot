# Design

## Context

See `proposal.md` for background on torn flash states.
Currently on STM32 targets, `application_check_valid()` checks only that Initial SP lies in RAM bounds and Reset PC lies in flash bounds with bit 0 set. `commit_state_valid()` is stubbed to `return 1;`. When an update writes block 0 and is subsequently aborted, the bootloader attempts to execute the unprogrammed code at `0x08002258`, triggering an immediate `HardFault` that renders slave bus communication dead.

## Goals / Non-Goals

**Goals:**
- Guarantee that BareBoot never branches to unprogrammed (`0xFFFFFFFF`) or zeroed (`0x00000000`) code at the Reset Vector.
- Ensure that an aborted flash transfer leaves the target permanently parked in bootloader mode across resets without SWD or hardware intervention.
- Provide inverted flash block sequencing in host tooling so block 0 is programmed strictly after full verification.

**Non-Goals:**
- Cryptographic code signing (e.g. Ed25519 verification) in this phase.
- Re-architecting flash memory layout away from the single-slot in-situ model.

## Decisions

### Decision 1: Dereference Entry Opcode in `application_check_valid()`
- **Approach**: After validating SP and PC addresses against memory bounds, dereference the 32-bit word at `pc & ~1`. If `*entry == 0xFFFFFFFF` or `*entry == 0x00000000`, evaluate the application as invalid.
- **Rationale**: An unprogrammed erase state (`0xFFFFFFFF`) or wiped memory (`0x00000000`) at the entry address is guaranteed to trigger an invalid opcode HardFault on Cortex-M. Rejecting it at boot time costs under 10 machine cycles and prevents the jump entirely.
- **Alternatives Considered**: Full image checksumming on boot. *Rejected*: Checking hundreds of kilobytes on every boot adds unacceptable latency (100–300 ms) to normal startup.

### Decision 2: Multi-Vector Verification
- **Approach**: Inspect `app[3]` (`HardFault_Handler`) and verify it is not `0xFFFFFFFF` and has bit 0 set.
- **Rationale**: A valid Cortex-M vector table always defines a `HardFault_Handler`. If `app[3]` is `0xFFFFFFFF`, the vector table was partially written and cannot safely catch faults.

### Decision 3: Inverted Flashtool Block Ordering
- **Approach**: In `flashtool.py` and Python update scripts, reorder the block sequence: cache Block 0 (`0x08002000..0x08002040`), flash all subsequent blocks (`0x08002040..APP_END`), and program Block 0 last upon receiving `COMPLETE`.
- **Rationale**: If the USB cable is unplugged or bus communication fails mid-transfer, Block 0 remains in its erased state (`0xFFFFFFFF`). On the next reset, `app[0]` is `0xFFFFFFFF`, which fails `application_check_valid()` immediately and leaves the device parked in bootloader mode.

## Risks / Trade-offs

- **[Risk]** Memory-mapped flash read during `application_check_valid` could trigger bus fault if `pc` points to an unclocked or reserved bus address.  
  → **Mitigation**: Bounds check `pc` against `[CONFIG_LAUNCH_APP_ADDRESS, CONFIG_FLASH_START + CONFIG_FLASH_SIZE - 4)` strictly before dereferencing.
