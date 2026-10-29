## ADDED Requirements

### Requirement: Multi-bus discovery

The host engine SHALL discover nodes across supported bus types and distinguish nodes by UUID for CAN and by configured address for I2C.

#### Scenario: Multi-node CAN scan

- **WHEN** two nodes are connected on one CAN bus
- **THEN** the host lists both UUIDs and can select one by UUID

### Requirement: Pipelined block transfer

The host SHALL stream firmware blocks, wait for per-block acknowledgment, enforce sequential addresses, and issue EOF before jump.

#### Scenario: Verification failure blocks jump

- **WHEN** read-back verification fails
- **THEN** the host does not send `COMPLETE`

### Requirement: Transport backends

The host engine SHALL provide SocketCAN, I2C device/bridge, and serial backends behind one flashing API.

#### Scenario: I2C flashing works without CAN

- **WHEN** only an I2C node is present
- **THEN** the host can connect, stream blocks, and jump using the I2C backend
