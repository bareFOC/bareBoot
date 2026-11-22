# unified-identity-addressing Specification

## Purpose

Provides unified silicon electronic signature derivation, dynamic address synchronization via soft-reset RAM mailbox, and deterministic fallback addressing across bootloader and application modes in multidrop networks.

## Requirements

### Requirement: Unified Silicon Identity Derivation
The bootloader and application SHALL deterministically derive their identity representations from factory electronic signature registers.

#### Scenario: 6-byte Katapult CAN UUID derivation
- **GIVEN** raw hardware factory silicon UID registers
- **WHEN** FastHash64 is evaluated over the UID bytes using seed `0xA16231A7`
- **THEN** the lower 6 bytes of the 64-bit hash match the Katapult CAN UUID format byte-for-byte

#### Scenario: 48-bit MIPI I3C Provisional ID derivation
- **GIVEN** STM32 factory UID registers
- **WHEN** the I3C target peripheral initializes
- **THEN** `I3C_EPIDR` is configured with MIPI Vendor ID `0x0114`, Part ID `0x0001`, and the lower 32 bits derived from silicon UID registers

### Requirement: Soft-Reset RAM Mailbox Synchronization
The bootloader SHALL inspect the uninitialized SRAM mailbox before reading persistent flash, preserving dynamically assigned addresses across soft resets without flash writes.

#### Scenario: Dynamic address handoff on warm reboot
- **GIVEN** a node assigned dynamic address `0x42` via I3C or CAN
- **WHEN** the node receives an in-band reboot command and writes `SoftResetMailbox` with valid CRC32 to `0x2000_7FE0`
- **THEN** BareBoot reads `0x42` from the mailbox, bypasses flash storage read, and immediately binds its serial target to `0x42`

### Requirement: Deterministic Fallback Addressing
The bootloader SHALL derive a collision-free default 7-bit address from the silicon UID when both the RAM mailbox and flash storage are blank or uninitialized.

#### Scenario: Fresh unprogrammed silicon power-on
- **GIVEN** a freshly manufactured node with unwritten flash sectors 14 and 15
- **WHEN** the node powers on and finds an invalid mailbox
- **THEN** it binds to address `0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F)`, ensuring distinct addresses across multiple unconfigured nodes on the shared bus
