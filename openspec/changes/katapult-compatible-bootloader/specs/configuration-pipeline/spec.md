## ADDED Requirements

### Requirement: Declarative target configuration

The build system SHALL consume `bareboot.toml` and emit linker scripts and generated constants for MCU, clock, pins, CAN bitrate, I2C address, timeouts, and application base.

#### Scenario: Changing a timeout

- **WHEN** a user edits the boot window value in `bareboot.toml`
- **THEN** the generated constants reflect it without editing source code

### Requirement: Feature selection in build scripts

`build.rs` SHALL translate configuration values into Cargo features for selected transports and SHALL not require menuconfig.

#### Scenario: Disabled I2C removes its driver

- **WHEN** I2C is not selected in configuration
- **THEN** the I2C target driver is not compiled into firmware

### Requirement: Size enforcement

CI SHALL fail any firmware build exceeding 4 KB or leaving less than the configured safety margin.

#### Scenario: Overlong firmware fails

- **WHEN** a firmware binary exceeds the limit
- **THEN** CI reports failure rather than publishing it
