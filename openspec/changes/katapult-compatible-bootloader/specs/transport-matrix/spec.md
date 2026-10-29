## ADDED Requirements

### Requirement: Compile-time transport selection

Transport availability SHALL be selected at compile time and unused transport code SHALL be eliminated by dead-code elimination.

#### Scenario: Single-transport image is smaller

- **WHEN** a target is built with only CAN enabled
- **THEN** I2C, UART, and USB code is absent from the final binary

### Requirement: Shared byte-stream protocol input

Every transport SHALL feed received bytes into the same frame parser and shall serialize outgoing Katapult frames the same way.

#### Scenario: I2C uses same frame protocol

- **WHEN** a host sends a framed command over I2C
- **THEN** firmware responds with the Katapult header, CRC, and trailer over I2C

### Requirement: CAN administrative behavior

CAN-enabled firmware SHALL support administrative discovery, node ID assignment by UUID, and command traffic on assigned node IDs.

#### Scenario: Host discovers an unassigned node

- **WHEN** the host queries `CANBUS_ID_ADMIN`
- **THEN** an unassigned node responds with its UUID and assignment command marker

### Requirement: At most two active transports per 4 KB image

A firmware image containing two transports SHALL implement the first-arrival arbiter and SHALL still meet the 4 KB limit; otherwise the build SHALL fail size checks.

#### Scenario: Oversized combined image fails CI

- **WHEN** link output exceeds the configured margin
- **THEN** the build is rejected before release
