## Context

The TUI is a ratatui app (`bareboot-tui`) with three tabs (Bus, Config, Build) and a worker thread connected by mpsc channels. The CLI (`bareboot-cli`) implements the full flash loop using `bareboot_client::FlashSession` and `HostBus` backends. Today the TUI shares none of that logic: config is read-only, there is no flash run, no profiles, and factory flashing is a bare `probe-rs list` invocation. The existing `interactive-tui` spec only requires worker-thread architecture, tabs, and probe-based factory mode.

Constraints: keep rendering on the UI thread and blocking I/O on the worker thread; serial/i2c/can backends are Linux-only; firmware config (`bareboot.toml`: target/transport/boot_window_ms) is distinct from host flash parameters.

## Goals / Non-Goals

**Goals:**
- Reach full CLI parity in the TUI: all backends, all run options, full flash loop with progress.
- Guided wizard under a single `Flash` tab; profiles stored project-locally.
- Editable, validated `bareboot.toml`.
- Factory flashing via probe-rs and serial.
- Auto-discover firmware from cargo build output.

**Non-Goals:**
- Rewriting the wire protocol or `FlashSession`.
- Cross-platform serial/i2c/can (Linux only, as in CLI).
- GUI/TUI beyond terminal (ratatui only).
- Remote/SSH profile sync.

## Decisions

### D1: Extract shared flash runner from CLI

Move the connect→write→verify→finish→complete loop into a reusable function in `bareboot-client` (or a small `run_flash(profile, event_tx)` helper) so CLI and TUI share one implementation. The TUI worker calls it; CLI calls it too. Avoids drift.

Alternative: duplicate loop in TUI. Rejected: two code paths for the same protocol would diverge.

### D2: Separate `FlashProfile` from `BarebootConfig`

`BarebootConfig` stays firmware-side (target/transport/boot_window_ms). New `FlashProfile` struct holds host-side params (backend, device, baud, address, interface, node_id, firmware path, tries, timeout, verify). Profiles are TOML files under `./profiles/`.

Alternative: extend `BarebootConfig` with host fields. Rejected: conflates firmware compile-time config with host connection state; breaks config validation semantics.

### D3: Wizard as sub-steps inside one Flash tab

Top tabs remain Bus/Config/Build/Flash/Profiles (5). Inside Flash, a state machine (`FlashStep`: Backend, Device, Firmware, Options, Run) with back/next. Keeps tab bar stable and avoids 9+ tabs.

Alternative: one tab per step. Rejected: too many tabs, cramped on small terminals.

### D4: Profile storage project-local

`./profiles/<name>.toml`. Created on demand. Auto-gitignored? No—let user decide; document it. Per-user config deferred to a future option.

### D5: Worker command extensions

Add `WorkerCommand` variants: `Flash { profile }`, `LoadProfiles`, `SaveProfile { profile }`, `DeleteProfile { name }`, `WriteConfig { text }`, `FactoryFlash { method, ... }`. Worker emits existing `UiEvent`s plus a new `ProfileList { names }`.

### D6: Firmware auto-discovery

After a successful cargo build, scan `target/` (and `target/<profile>/`) for the most recent `.bin`/`.elf` matching the configured target; prefill firmware field. Fall back to manual path.

### D7: Factory flashing methods

Probe-rs (existing) plus serial. Serial factory uses `SERIAL_BL_REQ` handshake then delegates to the normal flash loop over serial backend. Both independent of bus-protocol flashing.

## Risks / Trade-offs

- [Flash loop extraction touches CLI] → keep CLI behavior identical; add tests.
- [Profile schema evolution] → version field in profile TOML; reject unknown with clear error.
- [Linux-only backends on other OS] → cfg-gate; TUI shows "not available on this platform".
- [Wizard state complexity] → single `FlashWizard` struct with step enum and validation per step.
- [Concurrent worker commands] → worker processes one command at a time (existing pattern); queue not needed.

## Migration Plan

1. Extract flash runner; CLI uses it (no behavior change).
2. Add `FlashProfile` + profile module.
3. Add Flash tab + wizard + Profiles tab.
4. Make Config tab editable.
5. Add factory flash (probe-rs + serial).
6. Auto-discover firmware.

## Open Questions

- Should profiles be shareable/exportable as a single file? (Deferred)
- Cancellation mid-flash: send `Shutdown`? Add `Cancel` command later.