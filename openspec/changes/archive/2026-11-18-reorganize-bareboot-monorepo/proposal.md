## Why

The `bareBoot` repository has grown organically through multiple MCU bootloader experiments and protocol iterations, leaving the codebase severely fragmented. `bareboot/Cargo.toml` is currently broken due to dangling references to deleted target directories, target implementations are split across disparate root folders (`targets/bareboot-rp2350`, `bareboot-ch32v203-usb`, `i2c-bootloader`, `i3c-i2c-bootloader`), host tooling is isolated under `bareflash/`, flashing scripts are scattered in multiple locations, and uncommitted or zero-byte files clutter the repository root. Reorganizing the monorepo establishes a clear, maintainable architecture for adding and maintaining MCU bootloader targets and host flashing tools.

## What Changes

- Consolidate common firmware logic into a top-level `crates/` directory (`bareboot-protocol`, `bareboot-core`).
- Establish a unified `targets/` directory housing all concrete MCU bootloader implementations (`targets/rp2350-usb`, `targets/ch32v203-usb`, `targets/stm32h503`).
- Consolidate Katapult C source, dynamic target driver, and Kconfig patches for STM32H503 under `targets/stm32h503/`.
- Unify host-side tools under `host/` (`bareboot-client`, `bareboot-cli`, `bareboot-tui`), and consolidate Python and shell flashing utilities under `host/scripts/`.
- Move board configuration profiles (`*.toml`) to a centralized `configs/` directory.
- Move hardware verification and test apps into `tests/`.
- Establish a root virtual `Cargo.toml` workspace managing shared protocol/core crates and host tooling with working unit test targets.
- Remove stale zero-byte files (`barefoot`, `barefoot_file`, `test-firmware/ch32-uart-parent`) and relocate prebuilt binary blobs to `artifacts/`.

## Capabilities

### New Capabilities
- `monorepo-structure`: Defines the repository directory taxonomy, target categorization (Rust vs C/Katapult), workspace manifest organization, and script consolidation.

### Modified Capabilities
<!-- No requirement changes to existing functional specifications -->

## Impact

- Directory paths for `crates/bareboot-protocol`, `crates/bareboot-core`, `bareflash/`, and target bootloaders will change.
- Cargo manifests will be updated to reflect the new relative dependency paths.
- Flashing helper scripts will be unified under `host/scripts/`.
- Existing documentation referencing legacy paths will be updated.
