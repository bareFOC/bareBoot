## ADDED Requirements

### Requirement: Project-local profile storage

The TUI SHALL store flash profiles as TOML files under `./profiles/`, creating the directory on demand.

#### Scenario: First save

- **WHEN** a user saves a profile and `./profiles/` does not exist
- **THEN** the directory is created and the profile file written

### Requirement: Profile CRUD

The TUI SHALL support creating, listing, loading, and deleting named profiles.

#### Scenario: Load profile

- **WHEN** a user selects a saved profile
- **THEN** all wizard fields are populated from the profile

### Requirement: Profile self-containment

A profile SHALL include backend, device params, firmware path, and run options so a run is reproducible without other inputs.

#### Scenario: Run from profile

- **WHEN** a user runs a loaded profile
- **THEN** no additional fields are required