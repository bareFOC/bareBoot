# tui-config-editor Specification

## Purpose
TBD - created by archiving change tui-full-cli-parity. Update Purpose after archive.
## Requirements
### Requirement: Editable config

The Config tab SHALL allow editing target, transport, and boot_window_ms in place.

#### Scenario: Edit field

- **WHEN** a user edits a field and saves
- **THEN** the value is validated and persisted to bareboot.toml

### Requirement: Validation

Invalid config (empty target, unsupported transport, boot_window_ms < 50) SHALL be rejected with a clear error before saving.

#### Scenario: Invalid transport

- **WHEN** a user enters transport "zigbee"
- **THEN** save is blocked and an error is shown

