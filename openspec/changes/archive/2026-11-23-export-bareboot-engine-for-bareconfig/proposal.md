# Proposal

## Why
The standalone `bareboot-gui` application is being absorbed into the unified `bareConfig` workbench in `bareFOC` to provide beginners and embedded developers with a single, frictionless tool for bootloader configuration, target flashing, motor setup, and tuning. `bareBoot` must clean up its workspace by deprecating `host/bareboot-gui` and stabilizing `bareboot-engine` and `bareboot-client` as public headless crates consumable by `bareConfig`.

## What Changes
- **DEPRECATE & REMOVE**: Remove `host/bareboot-gui` from the `bareBoot` virtual workspace members in favor of `bareFOC/bareConfig`.
- **STABILIZE ENGINE**: Ensure `crates/bareboot-engine` and `host/bareboot-client` cleanly export worker handles, non-blocking command/event channels (`WorkerCommand`, `UiEvent`), makeflash cargo build runners, and `probe-rs` runners without GUI couplings.
- **DOCS & CI UPDATES**: Update `bareBoot/README.md` and CI workflows to direct GUI users to `bareConfig`.

## Capabilities

### Modified Capabilities
- `bareboot-gui-app`: Deprecate and remove the standalone `bareboot-gui` crate from `bareBoot` workspace in favor of `bareFOC/bareConfig`.
- `bareboot-engine`: Stabilize headless worker API and export definitions for consumption by external configurators.

## Impact
- `host/bareboot-gui`: Removed from `bareBoot/Cargo.toml` and filesystem.
- `crates/bareboot-engine`: Becomes the primary public headless service layer for bootloader flashing and cargo compilation.
- CI/CD: WebAssembly deployment of GUI moves from `bareBoot` to `bareFOC`.
