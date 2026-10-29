## ADDED Requirements

### Requirement: Top status bar

The TUI SHALL display a top status bar showing connection state, active profile, and flash step.

#### Scenario: Connected

- **WHEN** a backend is connected
- **THEN** the status bar shows connected state

### Requirement: Context help footer

The bottom help line SHALL be context-sensitive to the active tab and overlay.

#### Scenario: Tab change

- **WHEN** the active tab changes
- **THEN** the help footer updates to show relevant keys