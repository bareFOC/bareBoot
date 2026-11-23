# Design

## Context
See `proposal.md` for motivation. `bareBoot` previously maintained `host/bareboot-gui`, an `egui`/`craft-ui` graphical app. With the decision to combine BareBoot and BareFOC configurators into `bareConfig` in `bareFOC`, `bareboot-gui` is deprecated in `bareBoot` to eliminate duplicate UI maintenance, while `bareboot-engine` and `bareboot-client` are preserved and exposed as clean headless libraries.

## Goals / Non-Goals

**Goals:**
- Remove `host/bareboot-gui` from the root workspace `Cargo.toml`.
- Ensure `crates/bareboot-engine` and `host/bareboot-client` compile cleanly with zero GUI dependencies.
- Update documentation and build scripts to direct users to `bareConfig`.

**Non-Goals:**
- Modifying target bootloader firmware code in `targets/` (RP2350, CH32V, STM32H5 targets remain intact).
- Modifying `bareboot-tui` (TUI cockpit remains as a lightweight terminal tool).

## Decisions

### Decision 1: Deprecate `bareboot-gui` crate
- **Rationale**: `bareConfig` in `bareFOC` incorporates all panels (`config_panel`, `makeflash_panel`, `traffic_panel`, `console_panel`) alongside BareFOC motor control, making a separate GUI in `bareBoot` redundant.
- **Alternatives Considered**: Keeping both GUIs in sync (rejected due to maintenance overhead).

### Decision 2: Clean API Boundary for `bareboot-engine`
- **Rationale**: `bareboot-engine` already encapsulates non-blocking worker channels (`WorkerHandles`, `WorkerCommand`, `UiEvent`), cargo streaming, probe-rs flashing, and atomic TOML profile persistence via `craft-atomic`. Making it a cleanly reusable headless crate allows `bareConfig` to depend on it directly via path or git dependency.

## Risks / Trade-offs

- **[Risk]** Existing documentation or web links pointing to `bareboot-gui`.
  - **Mitigation**: Update `README.md` and docs to point users to `bareConfig`.
