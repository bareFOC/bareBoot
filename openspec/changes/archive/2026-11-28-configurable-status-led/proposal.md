# Proposal: Configurable Single-LED Status Indicator in BareBoot

## Why

`bareBoot` currently toggles a hardcoded active-low `PA0` LED at a fixed 250 ms rate (`on_heartbeat(led_on)`), ignoring board TOML configuration and providing no visual distinction between normal boot, parked OTA mode, active flashing, blank flash, or boot-loop errors. Adding a simple, optional `status_led` configuration key and state-driven single-LED blink patterns gives instant field diagnostics on any board with zero overhead when omitted.

## What Changes

- **TOML Schema (`[bootloader].status_led`)**:
  - Add optional `status_led: Option<String>` (Klipper-style pin syntax, e.g. `"!PA0"` for active-low, `"PC6"` for active-high; omitted if the board has no LED) to `[bootloader]` in both `bareBoot/crates/bareboot-engine/src/config.rs` and `bareFOC/bareConfig/src/bsp/schema.rs`.
- **State-Aware Single-LED Pattern Engine (`bareboot-core`)**:
  - Add `BootStatus` (`BootWindow`, `Parked`, `Flashing`, `BlankApp`, `BootLoopError`) and `compute_led_on(status: BootStatus, ms: u32) -> bool` in `bareboot-core`.
  - Simplify `TickTimer::on_heartbeat(&mut self, led_on: bool)` (or `set_status_led(&mut self, led_on: bool)`) so `BootRunner` drives the exact single-LED pattern on every 1 ms tick with minimal flash footprint.
- **Target `build.rs` Pin Synthesis (`targets/stm32h503-i3c`)**:
  - Parse `[bootloader].status_led` at build time (via `BAREBOOT_CONFIG` / `BAREFOC_BOARD`, defaulting to `!PA0` for the reference STM32H503 I3C target) and generate the GPIO port/pin/polarity setup in `$OUT_DIR`, compiling to a no-op when `status_led` is `None`.
- **Docs**:
  - Document `status_led` and blink patterns in `barefoc.github.io/bareboot/config_reference.md` and `barefoc.github.io/docs/config_reference/index.md`.

## Capabilities

### New Capabilities
- `configurable-status-led`: Optional Klipper-style `[bootloader].status_led` configuration and deterministic single-LED blink patterns for `BootWindow`, `Parked`, `Flashing`, `BlankApp`, and `BootLoopError`.

## Impact

- **BareBoot**: `crates/bareboot-core/src/{hal.rs,runner.rs}`, `crates/bareboot-engine/src/config.rs`, `targets/stm32h503-i3c/{build.rs,src/main.rs}`, `configs/*.toml`.
- **BareFOC**: `bareConfig/src/bsp/schema.rs`, `examples/{fishyservo,microspora,miniata}/barefoc.toml`.
- **Docs**: `barefoc.github.io/bareboot/config_reference.md`, `barefoc.github.io/docs/config_reference/index.md`.
