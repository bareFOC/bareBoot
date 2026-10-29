# tui-forms Specification

## Purpose
TBD - created by archiving change tui-polish-lazygit. Update Purpose after archive.
## Requirements
### Requirement: Structured config form

Config editing SHALL use labeled fields with inline validation instead of a raw text buffer.

#### Scenario: Invalid field

- **WHEN** a field fails validation
- **THEN** an inline error is shown and save is blocked

### Requirement: Flash wizard fields

The flash wizard SHALL render per-step input fields for backend params and run options.

#### Scenario: Step focus

- **WHEN** a wizard step is active
- **THEN** its fields are editable and validated inline

