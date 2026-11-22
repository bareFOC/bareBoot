# Spec Delta

## Purpose

Provides a unified cross-platform graphical cockpit for managing, configuring, compiling, and flashing BareBoot microcontroller targets on desktop and mobile operating systems.

## ADDED Requirements

### Requirement: Multi-panel docking workspace layout
The GUI application SHALL render a dockable, configurable multi-panel workspace supporting Target Configuration, Makeflash Action Pipeline, Live Bus Sniffer, and Compiler Console views.

#### Scenario: User navigates workspace panels
- **WHEN** the user opens the application on desktop
- **THEN** dockable split panels for configuration, flashing controls, and telemetry are displayed with resizable splitters

### Requirement: Visual target configuration editor
The system SHALL provide an interactive visual editor to read, modify, and save BareBoot board definitions and target configuration profiles.

#### Scenario: Save valid board configuration
- **WHEN** the user edits target MCU family, CAN bitrate, and pin mappings and triggers save
- **THEN** the system validates parameters and writes the updated configuration atomically to persistent storage

#### Scenario: Reject invalid boot window timing
- **WHEN** the user enters a boot window timeout value lower than 50 milliseconds
- **THEN** the system marks the field as invalid and prevents saving

### Requirement: Desktop Katapult Makeflash pipeline
The system on desktop platforms SHALL orchestrate automated Cargo builds of target bootloaders and user applications, followed by immediate flashing via the selected deployment transport.

#### Scenario: Successful build and flash sequence
- **WHEN** the user triggers the Makeflash action on a selected target project
- **THEN** the system compiles the target firmware via Cargo, discovers the output artifact in target directories, and transmits blocks to the target MCU

#### Scenario: Compilation failure handling
- **WHEN** the underlying Cargo build fails with compiler errors
- **THEN** the system aborts the flashing stage, highlights errors in the console panel, and leaves the target device unmodified

### Requirement: Live bus traffic analyzer and frame logger
The system SHALL display real-time traffic frames captured across CAN, Serial, and I2C buses, decoding BareBoot Katapult opcodes, directions, node IDs, and hex payloads.

#### Scenario: Real-time frame decoding
- **WHEN** a Katapult frame is transmitted or received on the monitored bus
- **THEN** the table displays a new entry showing timestamp, direction, bus type, decoded opcode name, and hex payload data
