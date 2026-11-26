## MODIFIED Requirements

### Requirement: Dual TOML configuration support
The `bareboot-engine` and `bareboot-core` crates SHALL parse bootloader target definitions from either a standalone `bareboot.toml` or from a `barefoc.toml` (supporting `[bootloader]`, `[mcu]` or `[board]`, `[memory]` with `*_base` or `*_origin`/`*_size`, and `[gate_driver]`) using Klipper-style pin syntax (`!`, `^`, `~`).

#### Scenario: Parse bootloader settings from barefoc.toml
- **WHEN** the engine is pointed to a board definition containing `barefoc.toml`
- **THEN** it extracts MCU target, flash memory bounds, initial safe pins, and transport settings transparently

#### Scenario: Parse bootloader settings and memory origins from barefoc.toml
- **WHEN** the engine is pointed to a `barefoc.toml` using `[board]`, `[memory]` (`flash_origin`, `app_origin`, `app_size`, `nvm_origin`), and `[bootloader]`
- **THEN** it extracts MCU target, flash memory bounds (`app_base`, `app_end`, `nvm_base`), initial safe pins, and transport settings transparently

#### Scenario: Parse standalone bareboot.toml
- **WHEN** the engine is pointed to a standalone `bareboot.toml`
- **THEN** it parses the root bootloader configuration for non-motor targets

## ADDED Requirements

### Requirement: Klipper-style SafePin parsing and PinMode execution
`bareboot-core` SHALL parse and apply Klipper-style pin specifications (`!pin` for `OutputLow`, `pin` for `OutputHigh`, `^pin` for `InputPullUp`, `~pin` for `InputPullDown`) without legacy `:low`/`:high` suffixes.

#### Scenario: Parse active-high, active-low, and pull-up Klipper pin specs
- **WHEN** `SafePin::parse` is called with `"!PA9"`, `"PB1"`, `"^!PB12"`, and `"~PC13"`
- **THEN** it returns `OutputLow`, `OutputHigh`, `InputPullUp`, and `InputPullDown` respectively

### Requirement: Auto-derive safe startup pins from `[gate_driver]`
When parsing a `barefoc.toml` without an explicit `[bootloader].initial_safe_pins` list, `bareboot-engine` SHALL automatically derive safe startup `PinMode` entries from `[gate_driver]` (`sleep_pin`, `enable_pin`/`en_pin`, `inl_a..c`, `inh_a..c`, and `fault_pin`), inverting active-high outputs to `OutputLow` and active-low (`!`) outputs to `OutputHigh`.

#### Scenario: Auto-derive safe pins for active-high and active-low driver pins
- **WHEN** `barefoc.toml` defines `[gate_driver]` with `sleep_pin = "PA9"`, `enable_pin = "!PB1"`, `inl_a = "PB5"`, and `fault_pin = "^!PB12"` without `[bootloader].initial_safe_pins`
- **THEN** `parse_target_config` derives `["!PA9", "PB1", "!PB5", "^PB12"]`

#### Scenario: Explicit initial_safe_pins overrides auto-derivation
- **WHEN** `barefoc.toml` defines both `[gate_driver]` and `[bootloader].initial_safe_pins`
- **THEN** `parse_target_config` uses the explicit `[bootloader].initial_safe_pins` list

### Requirement: Target build and startup wiring for safe pins
`bareboot-engine` and BareBoot target firmware (`stm32h503-i3c`, `stm32h503`) SHALL propagate resolved Klipper-style `initial_safe_pins` into the target build and execute `apply_safe_pins` in `init_hardware()` before enabling communication peripherals.

#### Scenario: STM32H503 target drives derived safe pins on reset
- **WHEN** `stm32h503-i3c` is built with `barefoc.toml` / `BAREBOOT_SAFE_PINS` and resets into bootloader mode
- **THEN** `init_hardware()` configures all `INITIAL_SAFE_PINS` (`OutputLow`, `OutputHigh`, `InputPullUp`, `InputPullDown`) before initializing I3C2 or entering the boot window
