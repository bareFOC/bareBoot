## 1. Klipper-Style `SafePin` Core (`bareboot-core`)
- [x] 1.1 Replace `PinLevel` with `PinMode` (`OutputLow`, `OutputHigh`, `InputPullUp`, `InputPullDown`) in `crates/bareboot-core/src/safe_pins.rs`, implement Klipper-style `SafePin::parse` (`!`, `^`, `~`), update `PinDriver` and `initial_safe_pins!`, and update unit tests.

## 2. `barefoc.toml` Schema & Auto-Derivation (`bareboot-engine`)
- [x] 2.1 Support `[board]` and `[memory]` (`*_origin` / `*_size`) aliases in `parse_target_config()` in `crates/bareboot-engine/src/config.rs`, and switch `initial_safe_pins` to Klipper-style strings.
- [x] 2.2 Implement `derive_safe_pins_from_gate_driver()` in `crates/bareboot-engine/src/config.rs` handling active-high (`OutputLow`), active-low `!` (`OutputHigh`), and input pulls (`^`/`~`), with unit tests for `barefoc.toml` and explicit overrides.

## 3. Build Codegen & Target Startup Wiring (`bareBoot`)
- [x] 3.1 Propagate `initial_safe_pins` via `BAREBOOT_SAFE_PINS` / `CONFIG_INITIAL_PINS` in `crates/bareboot-engine/src/build.rs`.
- [x] 3.2 Update `targets/stm32h503-i3c/build.rs` and `targets/stm32h503-i3c/src/main.rs` to generate `INITIAL_SAFE_PINS` and apply `OutputLow` / `OutputHigh` / `InputPullUp` / `InputPullDown` in `init_hardware()` before I3C2 init.

## 4. `bareFOC` Manifest Alignment & Verification
- [x] 4.1 Update `examples/fishyservo_stm32h5/barefoc.toml` and `bareConfig/boards/*/barefoc.toml` to use Klipper-style pin modifiers where applicable (e.g. `^!PB12` for `fault_pin`).
- [x] 4.2 Run `cargo test` in `bareBoot` (`bareboot-core`, `bareboot-engine`) and `cargo build --release` in `targets/stm32h503-i3c`.
