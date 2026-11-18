## Context

The RP2350 USB BareBoot target writes application firmware into a single slot at XIP `0x10008000..0x10140000`. Production units are factory-flashed once (BOOTSEL/SWD acceptable only at manufacture) and thereafter rely on BareBoot for handsfree Katapult USB updates.

**Hardware finding (verified on Pico 2 / RP2350 A4):** the Embassy-USB-based Rust target (`targets/rp2350-usb`) never enumerated — the device connected at full speed but descriptor reads timed out (`device descriptor read/64, error -110`) and the core span in the Embassy executor scheduler. The Katapult reference bootloader (`ref/katapult`, `test/configs/rp2350.config`) enumerated cleanly on the same board (`1d50:6177`) and passed a stock `flashtool.py` CONNECT. Therefore the working RP2350 USB transport is based on Katapult's hand-written USB CDC stack, not Embassy. The reliability features are implemented there; the Embassy Rust target retains them as well but is not the production transport.

Katapult reference analysis (`ref/katapult`):
- `application_check_valid()` validates only SP bounds, PC Thumb bit, and PC range — no image integrity or commit state.
- The `bootup_code` 64-bit value is an SRAM mailbox used for handsfree *entry* only; it is volatile across cold reset.
- `is_in_transfer` is used only for LED status, not for boot decisions.
- `command_complete()` ACKs and schedules `application_jump()` after 100 ms; it writes no persistent state.
- Stock `flashtool.py` performs a full REQUEST_BLOCK readback + SHA1 verification **before** sending COMPLETE.

Conclusion: Katapult has no cross-reset commit mechanism. BareBoot must supply its own, without altering the wire protocol.

## Goals / Non-Goals

**Goals:**
- Never boot an application from a torn, interrupted, or failed update.
- Make BareBoot the sole recovery path — remove all BOOTSEL/ROM USB fallback from runtime behavior.
- Preserve 100% wire compatibility with stock Katapult `flashtool.py` (commands, framing, CRC, block semantics).
- Keep the single-slot flash layout and the `0x5984E3FA6CA1589B` cookie mailbox contract unchanged.
- Keep firmware `#![no_std]`, zero-heap, within the 32 KB bootloader slot.

**Non-Goals:**
- Full-image CRC/bit-rot detection at boot (marker-only reliability chosen).
- Reintroducing A/B slots, boot tables, or rollback (explicitly removed earlier).
- Modifying host flashing semantics or image format.
- AgentLA application-side behavior beyond path-string corrections in build tooling.

## Decisions

### 1. Commit state in a dedicated 4 KB flash sector
Reserve the last 4 KB page of the BareBoot slot (`0x10007000..0x10008000`) as a commit-state sector, via `memory.x`:
```
FLASH (rx) : ORIGIN = 0x10000000, LENGTH = 28K
STATE (rw) : ORIGIN = 0x10007000, LENGTH = 4K
```
A Rust static in `#[link_section = ".commit_state"]` holds a magic + state word. Rationale: BareBoot text is ~19 KB (`0x10000000..0x10004A00`), so the state sector is a different 4 KB erase block — the bootloader never erases the sector it executes from. This keeps the app slot pure (no image contamination, no UF2 packaging change) and survives power loss.

Alternatives considered:
- *Small reserved area in app slot first page*: rejected — contaminates the app image and interacts with UF2 packaging.
- *Vector invalidation only*: rejected — a fully written page 0 with a valid vector could still boot a partial image.
- *Full-image CRC at boot*: deferred — stronger but higher boot cost and code size; marker-only meets the product requirement.

### 2. COMPLETE is the commit point; SEND_BLOCK is the in-progress marker
State machine (all writes go through `Flash::blocking_erase` + `blocking_write` on the STATE sector):
- On first `SEND_BLOCK` of a session: write `UPDATE_IN_PROGRESS` (magic + 0).
- On `COMPLETE` (after ACK, only reached when host verification succeeded): write `VALID` (magic + 1).
- On boot: `should_enter_recovery` = cookie present OR invalid vector OR state != VALID.
- On successful jump (cookie present path, valid existing app): state unchanged (already VALID).

Rationale: stock flashtool sends COMPLETE only after SHA1-verified readback, so COMPLETE is a trustworthy commit signal. No new command or image byte is needed.

### 3. Flash error: NACK, then self-reset into recovery
Replace `unrecoverable_fallback()` (`reset_to_usb_boot`) with a routine that replies `COMMAND_ERROR (0xf2)` and then triggers `SCB::sys_reset()`. State remains `UPDATE_IN_PROGRESS`, so after reset BareBoot stays in recovery. No ROM USB boot is ever invoked.

### 4. Katapult-aligned vector validation
Add `pc & 1 == 1` (Thumb bit) to `valid_app_vector`, matching `application_check_valid()` in `ref/katapult/src/generic/armcm_canboot.c`.

### 5. Cookie clearing remains BareBoot-only
No app-side dependency: BareBoot clears the mailbox before jumping (current behavior). The host places the cookie via SWD mailbox poke or app write; that contract is unchanged.

## Risks / Trade-offs

- [STATE sector flash endurance] Each update erases/writes one 4 KB sector; RP2350 flash tolerates ~100k cycles → acceptable for product update counts.
- [Power loss during marker write] STATE sector write is two-phase (magic then state word); a torn write could leave an indeterminate state. Mitigation: store state as two opposite words and only accept an exact `VALID` pattern; any other pattern means recovery. Use a CRC or double-write with complement to make corruption detectable — choose a 32-bit state + 32-bit complement, accepted only if `state ^ complement == 0xFFFFFFFF`.
- [Flash writes from XIP] The STATE sector is a distinct erase block from executing code; verified by the 19 KB text size vs `0x10007000` sector base. The flash driver already runs RAM helpers (`embassy_rp::flash::Blocking`) for erase/program.
- [Boot Shim / vector table] No change to the two-segment UF2 packaging; bootloader binary still starts at `0x10000000`.
- [Release workflow path] Build path must be corrected from `targets/bareboot-rp2350` to `targets/rp2350-usb` or releases ship without BareBoot.

## Migration Plan

1. Implement marker state machine and boot gating in the RP2350 target.
2. Run host unit tests and cross compile; verify size <= 32 KB.
3. Flash on Pico 2 via SWD/BOOTSEL once (factory-equivalent), verify USB CDC enumerates and stock-flashtool COMPLETE flow sets state VALID.
4. Correct release workflow path; no field migration needed (fresh factory images).

## Open Questions

- Exact 32-bit magic value for commit state (choose a fixed constant, e.g. `0x42425354` "BBST").
