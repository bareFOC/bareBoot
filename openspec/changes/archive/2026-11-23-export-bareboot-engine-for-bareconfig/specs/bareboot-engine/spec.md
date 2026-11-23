# Spec Delta

## ADDED Requirements

### Requirement: Headless export for external configurator integration
The `bareboot-engine` crate SHALL expose its worker runner, profile persistence, and `makeflash` streaming APIs as a clean library interface without graphical UI dependencies, allowing external orchestrators such as `bareConfig` to embed bootloader workflows directly.

#### Scenario: External configurator initializes bareboot engine worker
- **WHEN** an external application invokes `WorkerHandles::spawn()` from `bareboot-engine`
- **THEN** the engine initializes background execution threads and returns command and event channels for driving Katapult flashing and Cargo builds

### Requirement: Dual TOML configuration support
The `bareboot-engine` and `bareboot-core` crates SHALL parse bootloader target definitions from either a standalone `bareboot.toml` or from the `[bootloader]`, `[mcu]`, and `[memory]` sections of a `barefoc.toml`, eliminating dual-file maintenance for BareFOC hardware targets.

#### Scenario: Parse bootloader settings from barefoc.toml
- **WHEN** the engine is pointed to a board definition containing `barefoc.toml`
- **THEN** it extracts MCU target, flash memory bounds, initial safe pins, and transport settings transparently

#### Scenario: Parse standalone bareboot.toml
- **WHEN** the engine is pointed to a standalone `bareboot.toml`
- **THEN** it parses the root bootloader configuration for non-motor targets

### Requirement: Initial safe pin execution on reset
The `bareboot-core` target runtime SHALL apply configured `initial_safe_pins` immediately upon reset before starting peripheral clocks or transport listeners, ensuring motor gate driver charge pumps and bridges are held in safe inactive states.

#### Scenario: Safe pins driven on startup
- **WHEN** the microcontroller resets into BareBoot bootloader mode
- **THEN** all pins configured under `initial_safe_pins` (e.g. `DRV_SLEEP` low, half-bridge enables low) are driven to their safe states immediately

### Requirement: Multi-interface transport arbitration
The `bareboot-core` runtime SHALL support multi-interface communication arbitration with both concurrent polling across independent physical peripherals and time-sliced multiplexing across shared physical pins.

#### Scenario: Directed bootloader jump via RAM mailbox
- **WHEN** an in-app soft reset provides an interface hint in the RAM mailbox
- **THEN** BareBoot skips arbitration and initializes only the requested communication transport with an infinite boot window

#### Scenario: Time-sliced arbitration on shared pins
- **WHEN** the board cold-boots with shared physical pins (e.g. UART and I2C on PB6/PB7)
- **THEN** BareBoot time-slices listening intervals between the protocols until a valid connection frame is received or the boot window elapses
