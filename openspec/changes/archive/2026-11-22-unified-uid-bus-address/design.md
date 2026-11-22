# Design

## Context

See `proposal.md` for motivation. On STM32H503, SRAM spans `0x2000_0000..0x2000_8000` (32 KiB). In BareBoot's linker script `targets/stm32h503/src/armcm_link.lds.S`, the stack starts at `0x2000_7FF8`. The top 32 bytes (`0x2000_7FE0..0x2000_7FFF`) can be preserved across soft resets by marking the section `.reserved (NOLOAD)`. In Rust (`bareFOC`), this corresponds to `#[link_section = ".uninit"]`.

Persistent storage resides in Ping-Pong Sectors 14 and 15 (`0x0801_C000..0x0802_0000`).

## Goals / Non-Goals

**Goals:**
- Zero flash wear on warm reboots: dynamically assigned addresses passed seamlessly via SRAM mailbox.
- Deterministic silicon hardware identity matching across bootloader and application.
- Collision-free uninitialized fallback addressing.
- Strict `#![no_std]` and zero-heap compliance.

**Non-Goals:**
- A/B partition swapping or dual boot slots (strictly single-slot Katapult).
- Encrypted bootloader images or crypto authentication.

## Decisions

### Decision 1: Soft-Reset RAM Mailbox at `0x2000_7FE0` with CRC32
- *Rationale*: A 32-byte struct storing `magic`, `dynamic_node_id`, `bus_type`, `flags`, `can_assigned_id`, and `crc32`.
- *Alternatives Considered*: Writing dynamic address to Flash Sector 14 every time. Rejected because NOR flash erase takes 20-40 ms and wears flash cells unnecessarily.

### Decision 2: FastHash64 with Katapult Seed for CAN UUID
- *Rationale*: Upstream Katapult flashtool computes a 6-byte UUID from the raw silicon UID using Zilong Tan's FastHash64 with seed `0xA16231A7`. Implementing this in `bareboot-protocol` ensures full toolchain parity.
- *Alternatives Considered*: Simple XOR folding or CRC16. Rejected because it breaks stock Katapult `flashtool.py -u <uuid>`.

### Decision 3: Deterministic Silicon UID Fallback Address
- *Rationale*: If flash is unprogrammed, derive `0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F)`, mapping into `0x10..=0x4F`.
- *Alternatives Considered*: Always defaulting to `0x40`. Rejected because multiple new boards powered on simultaneously on a multidrop bus would collide.

## Risks / Trade-offs

- [Cold Power Loss drops RAM mailbox] -> Mitigation: System falls back to persistent Flash Sector 14/15 `H5FlashStorage`, retaining factory-saved NodeId.
- [SRAM corruption on brownout] -> Mitigation: Full 32-bit CRC32 check over the mailbox payload. If CRC32 fails, mailbox is rejected and flash is consulted.
