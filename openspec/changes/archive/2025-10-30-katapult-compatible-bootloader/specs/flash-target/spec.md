## ADDED Requirements

### Requirement: Flash trait contract

Each target SHALL implement page size, page erase, chunked write, UID readout, and application jump through one common target trait. Host protocol code SHALL not access target registers directly.

#### Scenario: Protocol shares behavior across targets

- **WHEN** a new MCU target is added
- **THEN** the same protocol and host logic can use it without changing command handlers

### Requirement: Page-buffered writes

The firmware SHALL preserve writes that do not complete a flash page and SHALL commit complete buffers at page boundaries and on EOF.

#### Scenario: Host writes a partial final page

- **WHEN** `SEND_EOF` is received after a partial page
- **THEN** the firmware commits the remaining buffered bytes before reporting page count

### Requirement: Application address protection

The firmware SHALL reject write or read addresses below the configured application start address.

#### Scenario: Invalid block address is rejected

- **WHEN** `SEND_BLOCK` or `REQUEST_BLOCK` uses an address below app start
- **THEN** the firmware returns an error response

### Requirement: UID readout

The firmware SHALL read the target UID into a 12-byte buffer for `GET_CANBUS_ID` and node identification.

#### Scenario: Host verifies UUID

- **WHEN** the host requests UUID after node association
- **THEN** firmware returns the 6 bytes in the ref Katapult byte order
