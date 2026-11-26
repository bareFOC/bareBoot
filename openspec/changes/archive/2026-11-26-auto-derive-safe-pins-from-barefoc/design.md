# Design: Klipper-Style Safe Pins & Direct `barefoc.toml` Integration

## 1. Klipper-Style Pin Model (`crates/bareboot-core/src/safe_pins.rs`)
- Replace `PinLevel` with:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PinMode {
      OutputLow,
      OutputHigh,
      InputPullUp,
      InputPullDown,
  }
  ```
- `SafePin::parse(s: &str) -> Result<Self, ()>`:
  - Scans leading Klipper modifiers (`!`, `^`, `~`) in any order:
    - If `^` is present → `PinMode::InputPullUp`
    - Else if `~` is present → `PinMode::InputPullDown`
    - Else if `!` is present → `PinMode::OutputLow`
    - Else → `PinMode::OutputHigh`
  - Parses remaining `P?<PORT><NUM>` (`A..=Z`, `0..=31`).
  - Legacy `:low` / `:high` suffixes are rejected (no backward compatibility).
- `PinDriver` trait: `fn configure(&mut self, port: char, pin: u8, mode: PinMode);`
- `initial_safe_pins!` macro: `initial_safe_pins![('B', 15, OutputLow), ('B', 12, InputPullUp)]`.

## 2. `barefoc.toml` Schema & Polarity Inversion (`crates/bareboot-engine/src/config.rs`)
- **Board & Memory**:
  - Target resolution: `[bootloader].target` → `[mcu].target`/`family` → `[board].mcu`/`target`/`family`.
  - Memory resolution: `flash_start` or `flash_origin`; `app_base` or `app_origin`; `app_end` or `app_origin + app_size`; `nvm_base` or `nvm_origin`.
- **Explicit `initial_safe_pins`**:
  - `Vec<String>` of Klipper-style pin specs (e.g. `["!PA9", "!PB5", "^PB12"]`), validated via `SafePin::parse`.
- **Auto-Derivation from `[gate_driver]`** (when `[bootloader].initial_safe_pins` is omitted):
  - **Polarity rule for driver outputs** (`sleep_pin`, `enable_pin`, `en_pin`, `inl_a..c`, `inh_a..c`):
    - Active-high pin in `[gate_driver]` (no `!`, e.g. `sleep_pin = "PA9"`): safe inactive state is **low** → emit `"!PA9"` (`OutputLow`).
    - Active-low pin in `[gate_driver]` (`!` prefix, e.g. `enable_pin = "!PB1"`): safe inactive state is **high** → emit `"PB1"` (`OutputHigh`).
    - Strip `^`/`~` if present on an output pin before applying polarity inversion.
  - **Input pins** (`fault_pin`):
    - If `^` is present (e.g. `"^!PB12"`) → emit `"^PB12"` (`InputPullUp`).
    - If `~` is present → emit `"~PB12"` (`InputPullDown`).
    - If bare (no pull modifier), omit from output driving.

## 3. Build Codegen & Target Startup
- `bareboot-engine/src/build.rs`:
  - Exports `BAREBOOT_SAFE_PINS` (comma-separated Klipper strings, e.g. `"!PA9,!PB5,^PB12"`) and `CONFIG_INITIAL_PINS` (for Katapult C, which uses identical Klipper syntax).
- `targets/stm32h503-i3c/build.rs`:
  - Reads `BAREBOOT_CONFIG` (if pointing to `barefoc.toml` / `bareboot.toml`) or `BAREBOOT_SAFE_PINS`, parses each entry with `SafePin::parse`, and writes `$OUT_DIR/safe_pins_generated.rs`.
- `targets/stm32h503-i3c/src/main.rs`:
  - `init_hardware()` enables `RCC_AHB2ENR` clocks for all ports in `INITIAL_SAFE_PINS` and runs `apply_safe_pins()` (`MODER`, `PUPDR`, `BSRR`) before configuring I3C2 or starting the boot window.
