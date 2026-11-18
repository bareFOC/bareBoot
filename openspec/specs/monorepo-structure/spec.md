# monorepo-structure Specification

## Purpose
TBD - created by archiving change reorganize-bareboot-monorepo. Update Purpose after archive.
## Requirements
### Requirement: Root Virtual Workspace
The repository SHALL provide a root virtual `Cargo.toml` manifest managing all shared core crates and host-side tools.

#### Scenario: Host workspace builds and tests pass
- **WHEN** a developer executes `cargo test --workspace` at the repository root
- **THEN** all tests in `crates/bareboot-protocol`, `crates/bareboot-core`, and `host/*` execute and pass on the host architecture without requiring cross-compilation targets.

### Requirement: Core Crates Organization
The repository SHALL organize platform-independent firmware core libraries under a top-level `crates/` directory.

#### Scenario: Core crates structure
- **WHEN** inspecting the `crates/` directory
- **THEN** it SHALL contain `bareboot-protocol` and `bareboot-core`, free of any target-specific architecture dependencies.

### Requirement: Target Bootloaders Directory Taxonomy
The repository SHALL group all MCU bootloader implementations under a top-level `targets/` directory, organized by target identifier.

#### Scenario: Standalone target compilation
- **WHEN** a target directory (such as `targets/rp2350-usb` or `targets/ch32v203-usb`) is built
- **THEN** its `Cargo.toml` references `bareboot-core` and `bareboot-protocol` via relative path dependencies `../../crates/bareboot-core` and `../../crates/bareboot-protocol`.

#### Scenario: STM32H503 C target consolidation
- **WHEN** inspecting `targets/stm32h503/`
- **THEN** it SHALL contain the dynamic target C implementation, configuration patches, and target configuration files.

### Requirement: Host Tools and Flasher Consolidation
The repository SHALL consolidate all host-side flashing tools, client libraries, and flashing automation scripts under `host/`.

#### Scenario: Host scripts location
- **WHEN** running flasher utilities
- **THEN** `flash_app_over_i2c.py`, `flash_app_over_i3c.py`, and shell flashers are located in `host/scripts/`.

### Requirement: Clean Project Root
The repository root SHALL contain only root project files (`README.md`, `Cargo.toml`, `.gitignore`, `docs/`, `openspec/`) and standard top-level category directories.

#### Scenario: No orphaned artifacts or binaries in root
- **WHEN** listing the root directory
- **THEN** there SHALL be no zero-byte junk files, no loose `.bin` or `.elf` prebuilt binaries, and no loose board configuration `.toml` files.

