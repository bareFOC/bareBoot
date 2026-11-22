# Proposal

## Why

In multidrop actuator arrays connected via MIPI I3C, I2C, CAN, or RS485, multiple servo nodes share the same physical bus. When a host controller addresses a specific node (e.g. Node ID `0x42`) for firmware upgrade, the node must maintain the exact same hardware identity (96-bit silicon UID, 48-bit MIPI I3C Provisional ID, 6-byte Katapult CAN UUID) and the exact same bus address whether executing the bootloader (`BareBoot`) or the application (`bareFOC` / `FishyServo`). 

Without unified identity and dual-tier address synchronization, nodes either collide on default addresses or require flash wear and latency to persist runtime dynamic addresses across warm reboots.

## What Changes

- Implement `SoftResetMailbox` in `bareboot-protocol` and `targets/stm32h503` placed at uninitialized SRAM (`0x2000_7FE0..0x2000_7FFF`) with CRC32 integrity validation.
- Implement soft-reset address handoff: BareBoot reads active dynamic `node_id` from the mailbox before reading persistent flash, preserving dynamic addresses across soft resets with zero flash wear.
- Implement silicon hardware UID derivation: deterministically derive 48-bit MIPI I3C Provisional ID (`I3C_EPIDR`) and 6-byte Katapult CAN UUID (`fasthash64` with seed `0xA16231A7`) matching application identity.
- Implement deterministic collision-free fallback addressing on blank/uninitialized flash: `0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F)`.
- Update I3C and I2C target drivers in `targets/stm32h503` to bind directly to the unified address.

## Capabilities

### New Capabilities
- `unified-identity-addressing`: Silicon hardware UID derivation, 48-bit MIPI I3C Provisional ID matching, 6-byte Katapult CAN UUID generation, Soft-Reset RAM mailbox synchronization, and collision-free fallback addressing.

### Modified Capabilities
- `multidrop-inband-flashing`: Extend in-band bootloader entry and mailbox contract to pass active dynamic address and bus type across soft resets.
- `i3c-dynamic-bootloader`: Align I3C dynamic target registers (BCR, DCR, EPIDR) with silicon UID and active dynamic address from the mailbox.

## Impact

- `crates/bareboot-protocol`: Adds `SoftResetMailbox` struct, `REQUEST_CANBOOT`, `REQUEST_APP`, and `fasthash64`.
- `targets/stm32h503`: Updates `i3c_dynamic_target.c` to parse mailbox at `0x2000_7FE0` and configure `I3C2->EPIDR` and `DEVR0` / `OAR1`.
- Upstream compatibility: Preserves exact wire compatibility with Katapult flashtool and MIPI I3C specification.
