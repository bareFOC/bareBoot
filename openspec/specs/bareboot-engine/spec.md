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
