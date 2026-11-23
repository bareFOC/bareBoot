# bareboot-engine Specification

## Purpose

Provides a headless background service engine that orchestrates asynchronous operations, target build jobs, flashing sessions, and persistent profile management across all BareBoot frontends.

## Requirements

### Requirement: Asynchronous worker channel orchestration
The engine SHALL decouple user interface frontends from blocking I/O, MCU flashing loops, and compiler executions by exposing non-blocking command and event channels.

#### Scenario: UI sends non-blocking flash command
- **WHEN** a frontend sends a flash command with a target profile to the engine
- **THEN** the engine spawns an isolated execution task and periodically emits progress and status events across the event channel

### Requirement: Structured profile persistence
The engine SHALL load, validate, save, and delete named deployment profiles including MCU family, transport backend, address offsets, and verification flags.

#### Scenario: Load existing flash profile
- **WHEN** the engine receives a profile load request for a valid profile name
- **THEN** the engine parses the stored TOML profile file and returns the structured profile representation to the caller

#### Scenario: Atomic profile save
- **WHEN** the engine saves an updated profile to disk
- **THEN** the profile data is written atomically using a temporary file and renamed into place to prevent corruption

### Requirement: Uniform firmware artifact discovery
The engine SHALL automatically inspect candidate build directories to locate the most recently modified firmware binary, ELF, or UF2 image matching the selected build target.

#### Scenario: Firmware binary discovered
- **WHEN** a target build finishes successfully
- **THEN** the engine locates the newest binary in target release or debug directories and reports the resolved path in the completion event

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
