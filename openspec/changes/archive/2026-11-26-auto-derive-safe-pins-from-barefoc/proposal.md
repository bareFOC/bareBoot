# Change: Klipper-Style Safe Startup Pins & Direct `barefoc.toml` Support

## Why

Maintaining separate `bareboot.toml` and `barefoc.toml` files duplicates board pinout and memory map definitions. Additionally, the current `initial_safe_pins` format (`"PB15:low"` / `{ pin, level }`) is verbose, cannot express active-high vs. active-low driver polarity or input pull-ups/pull-downs, and differs from Klipper/Katapult's native pin syntax (`!`, `^`, `~`).

Adopting Klipper-style pin syntax across `barefoc.toml`, `bareboot-core`, `bareboot-engine`, and target bootloaders—without legacy `:low`/`:high` backward compatibility—lets users define pins once in `barefoc.toml` and automatically derive safe startup states for both active-high and active-low pins.

## What Changes

1. **Klipper-Style `SafePin` & `PinMode` (`crates/bareboot-core/src/safe_pins.rs`)**:
   - Replace `PinLevel` with `PinMode`: `OutputLow`, `OutputHigh`, `InputPullUp`, `InputPullDown`.
   - Replace `SafePin::parse` with Klipper-style modifier parsing:
     - `"!PA9"` → `OutputLow`
     - `"PA9"` → `OutputHigh`
     - `"^PB12"` / `"^!PB12"` → `InputPullUp`
     - `"~PB12"` / `"~!PB12"` → `InputPullDown`
   - Update `PinDriver::configure(port, pin, mode)` and `initial_safe_pins![]` macro.
2. **Direct `barefoc.toml` Support & `[gate_driver]` Auto-Derivation (`crates/bareboot-engine/src/config.rs`)**:
   - Support `[board]` (`mcu`, `target`, `family`) and `[memory]` (`flash_origin`, `app_origin` + `app_size`, `nvm_origin`) directly.
   - Parse `[bootloader].initial_safe_pins` as Klipper-style pin strings (`Vec<SafePinSpec>`).
   - When `[bootloader].initial_safe_pins` is omitted, auto-derive safe startup pins from `[gate_driver]`:
     - Output pins (`sleep_pin`, `enable_pin`/`en_pin`, `inl_a..c`, `inh_a..c`): no `!` (active-high) → safe `OutputLow` (`!<pin>`); `!` prefix (active-low) → safe `OutputHigh` (`<pin>`).
     - Input pins (`fault_pin`): `^` → `InputPullUp`, `~` → `InputPullDown`.
3. **Build Codegen & Target Startup Execution (`crates/bareboot-engine/src/build.rs`, `targets/stm32h503-i3c`, `targets/stm32h503`)**:
   - Pass resolved Klipper-style pins via `BAREBOOT_SAFE_PINS` (and `CONFIG_INITIAL_PINS` directly for Katapult C, which uses the same Klipper syntax).
   - Generate `INITIAL_SAFE_PINS` in `targets/stm32h503-i3c/build.rs` and call `apply_safe_pins()` in `init_hardware()` before enabling communication peripherals.
4. **`bareFOC` Alignment (`examples/*/barefoc.toml`, `bareConfig`)**:
   - Standardize `[gate_driver]` pin fields in `barefoc.toml` on Klipper modifiers (e.g., `fault_pin = "^!PB12"`, `enable_pin = "!PB1"` for active-low enables).

## Impact

- Single source of truth: `barefoc.toml` drives both `bareFOC` firmware and `bareBoot` bootloader safe startup states.
- Native support for active-high, active-low, pull-up, and pull-down pins with zero legacy syntax.
