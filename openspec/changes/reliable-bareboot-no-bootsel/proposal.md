## Why

Production RP2350 units are flashed with the BareBoot USB bootloader once in the factory; there is no BOOTSEL button and no debug probe in the field. Today BareBoot can jump into a partially written or corrupt application (the boot decision only checks two vector words), and a flash error during an update calls `reset_to_usb_boot()` — a BOOTSEL dead end on a production unit. BareBoot must be the sole, dependable recovery path for handsfree app updates.

## What Changes

- Add a persistent commit state in a dedicated 4 KB flash sector inside BareBoot's own 32 KB slot: `UPDATE_IN_PROGRESS` or `VALID`.
- Set state to `UPDATE_IN_PROGRESS` at the start of a transfer (first `SEND_BLOCK`) and to `VALID` only after a successful `COMPLETE` (which stock Katapult `flashtool.py` sends only after full readback SHA1 verification).
- Gate the application boot decision on `VALID` commit state in addition to the existing cookie and SP/PC vector checks. A torn or failed transfer therefore leaves BareBoot in recovery instead of booting a corrupt app.
- Replace the BOOTSEL fallback on flash error with NACK (`0xf2`) followed by a clean self-reset into BareBoot recovery. BareBoot never calls `reset_to_usb_boot()`.
- Tighten `valid_app_vector` with the Katapult-reference Thumb-bit check (`pc & 1 == 1`).
- Update `memory.x` to reserve the commit-state sector and add a linker section for the marker.
- Fix the stale bootloader build path in the AgentLA release workflow and `package_uf2.py` docstring so factory images actually contain BareBoot.
- Add host unit tests for commit-state transitions and the boot decision matrix.
- Use the Katapult reference bootloader (`ref/katapult`) hand-written USB CDC stack as the production RP2350 USB transport; the Embassy-USB-based Rust target did not enumerate on RP2350 A4 hardware (verified), while the Katapult transport enumerates and passes stock `flashtool.py` on the same board.

## Capabilities

### New Capabilities
- `rp2350-usb-recovery`: The RP2350 USB BareBoot target's reliable, Katapult-wire-compatible recovery behavior, including commit-state-based boot decisions and BOOTSEL-independent error handling.

### Modified Capabilities
(none — no existing spec-level requirements change)

## Impact

- `targets/rp2350-usb/src/main.rs`, `src/boot.rs`, `src/lib.rs`, `memory.x`, `build.rs`
- `crates/bareboot-core` (commit-state trait additions if needed)
- AgentLA-rust `.github/workflows/release.yml` and `tools/package_uf2.py` docstring (path corrections only)
- Stock `ref/katapult/scripts/flashtool.py` wire protocol: **unchanged**
