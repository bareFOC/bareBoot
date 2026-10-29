## Why

The TUI (`bareboot-tui`) only exposes a small slice of what `bareboot-cli` can do: bus scanning, cargo builds, and probe-rs listing. Users who want to flash firmware must drop to the CLI and re-enter backend, device, and run parameters by hand. The read-only config preview and lack of saved connection profiles make the TUI a "viewer" rather than a cockpit, which defeats the goal of a Rust-native host experience. This change turns the TUI into a full peer of the CLI: configure, save, and flash (application firmware and bootloader) from one place.

## What Changes

- Add a `Flash` tab with a guided wizard (backend → device params → firmware → options → run) whose steps are navigable sub-views under the single tab.
- Add a `Profiles` tab for saving/loading named flash profiles stored project-locally (`./profiles/*.toml`).
- Expose every CLI flash option in the TUI: backend (memory/serial/i2c/can), baud, i2c address, can interface, node id, tries, timeout, verify toggle, firmware path.
- Auto-discover firmware artifacts from cargo build output and prefill the firmware field.
- Make the Config tab editable with validation and save (target/transport/boot_window_ms).
- Run the full flash loop (connect → write blocks → verify → finish → complete) in the worker thread, emitting progress/logs to the UI.
- Add factory (bootloader) flashing via both probe-rs and serial, kept independent of bus-protocol flashing.
- Keep firmware-side `bareboot.toml` separate from host-side flash profiles; profiles carry the host backend.

## Capabilities

### New Capabilities

- `tui-flash-wizard`: Guided, sub-step-based flash flow exposing all CLI options, with progress and cancellation.
- `tui-profiles`: Named, project-local flash profiles (save/load/list/delete) that make a flash run reproducible.
- `tui-config-editor`: Editable, validated `bareboot.toml` editor in the TUI.
- `tui-factory-flash`: Probe-rs and serial based bootloader installation, independent of bus flashing.

### Modified Capabilities

- `interactive-tui`: Extend the worker thread command/event protocol to cover flash, profile, and config operations; add flash/profile/config tabs and wizard state.

## Impact

- Affected crates: `bareboot-tui` (main UI, worker, new profile module), `bareboot-client` (reuse `FlashSession`, `HostBus`, config types), `bareboot-cli` (shared flash loop logic ideally extracted for reuse).
- New module in TUI: `profile` (load/save/list flash profiles). New worker commands: `Flash`, `LoadProfile`, `SaveProfile`, `DeleteProfile`, `WriteConfig`.
- Profile storage: `./profiles/*.toml` (project-local), created on demand.
- Dependencies: no new external crates expected beyond existing ratatui/crossterm/toml; serial backend guarded by `#[cfg(target_os = "linux")]`.
- Risk: duplicating the flash loop between CLI and TUI; prefer extracting a shared helper in `bareboot-client` or a small shared crate.