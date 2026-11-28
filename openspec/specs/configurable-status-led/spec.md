# configurable-status-led Specification

## Purpose

TBD - created by archiving change configurable-status-led. Update Purpose after archive.

## Requirements

### Requirement: Optional status_led TOML Configuration
The `[bootloader]` section of `bareboot.toml` and `barefoc.toml` SHALL support an optional `status_led` pin string using Klipper-style polarity syntax (`"!<PIN>"` for active-low, `"<PIN>"` for active-high), and SHALL treat an omitted `status_led` as no LED attached.

#### Scenario: Active-low LED configured in TOML
- **WHEN** `[bootloader]` specifies `status_led = "!PA0"`
- **THEN** the target configures `PA0` as a push-pull output and drives `PA0` low when the LED state is `on == true`.

#### Scenario: No LED configured in TOML
- **WHEN** `[bootloader]` omits `status_led`
- **THEN** the target does not configure any status LED GPIO and `set_status_led` is a zero-cost no-op.

### Requirement: State-Aware Single-LED Blink Patterns
`BootRunner` SHALL drive distinct deterministic single-LED blink patterns (`BootWindow`, `Parked`, `Flashing`, `BlankApp`, `BootLoopError`) from elapsed milliseconds (`ms % 1000`) and active bootloader state.

#### Scenario: Distinguishing blank flash from boot-loop trap
- **WHEN** `BootRunner` is initialized on a blank flash (`app_integrity_ok == false, boot_loop_trapped == false`)
- **THEN** `compute_led_on(BootStatus::BlankApp, ms)` produces a double-blink pattern distinct from the triple-blink `BootStatus::BootLoopError` pattern.
