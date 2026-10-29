# tui-ui-modules Specification

## Purpose
TBD - created by archiving change tui-phase1-module-refactor. Update Purpose after archive.
## Requirements
### Requirement: Modular UI

Rendering and input handling SHALL be split into ui/ modules.

#### Scenario: Build

- **WHEN** the crate builds
- **THEN** main.rs contains only the event loop

