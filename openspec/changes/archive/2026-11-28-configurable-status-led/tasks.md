# Tasks: Configurable Single-LED Status Indicator

## 1. Core State Machine (`bareboot-core`)
- [x] 1.1 Add `BootStatus` enum and `pub const fn compute_led_on(status: BootStatus, ms: u32) -> bool` in `bareboot-core/src/runner.rs` (or `led.rs`).
- [x] 1.2 Track `flash_activity_ms` and `flash_error` in `BootRunner` and update `on_heartbeat(led_on)` whenever the computed LED state changes.
- [x] 1.3 Add host unit tests verifying all 5 `BootStatus` blink patterns and priority transitions.

## 2. TOML Schema & Target Codegen
- [x] 2.1 Add optional `status_led: Option<String>` to `[bootloader]` in `bareBoot/crates/bareboot-engine/src/config.rs` and `bareFOC/bareConfig/src/bsp/schema.rs` (with unit tests).
- [x] 2.2 Update `bareBoot/targets/stm32h503-i3c/build.rs` and `src/main.rs` to synthesize the status LED GPIO port/pin/polarity from `status_led` (no-op when `None`, default `!PA1`).
- [x] 2.3 Add `status_led` to `bareFOC/examples/fishyservo/barefoc.toml` (`"!PA1"`) and `bareFOC/examples/microspora/barefoc.toml` (`"PC6"`).

## 3. Verification & Docs
- [x] 3.1 Run `cargo test` in `bareBoot` and `bareFOC/bareConfig`, and verify `targets/stm32h503-i3c` builds within the 16 KiB slot (`<= 16,384` bytes).
- [x] 3.2 Update `barefoc.github.io/bareboot/config_reference.md` and `barefoc.github.io/docs/config_reference/index.md` with `status_led` and blink pattern table.
- [x] 3.3 Sync specs and archive `configurable-status-led`.
