## ADDED Requirements

### Requirement: Worker thread architecture

The TUI SHALL run rendering on the UI thread and bus operations in a worker thread or task connected by command and event channels.

#### Scenario: UI remains responsive

- **WHEN** a long flash operation is running
- **THEN** the TUI still renders progress without blocking the interface

### Requirement: Bus, config, and probe tabs

The TUI SHALL provide tabs for bus scanning/progress, TOML-based configuration, probe/compiler integration, flashing, and profiles.

#### Scenario: Bus tab active

- **WHEN** a user views the bus tab
- **THEN** detected nodes, throughput, and Katapult packet logs are shown

### Requirement: Probe-based factory mode

The TUI SHALL support probe-based bootloader installation and compiler invocation, while keeping firmware protocol flashing independent of probe availability.

#### Scenario: Probe unavailable

- **WHEN** no probe is connected
- **THEN** bus protocol flashing and node scanning still work