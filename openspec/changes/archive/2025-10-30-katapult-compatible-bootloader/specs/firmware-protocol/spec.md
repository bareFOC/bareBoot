## ADDED Requirements

### Requirement: Katapult-compatible frame parsing and response

The firmware SHALL parse and emit byte frames with the 2-byte header `0x01 0x88`, one command byte, one payload word count byte, a payload that is a multiple of 4 bytes, a 2-byte little-endian CRC16-CCITT, and the 2-byte trailer `0x99 0x03`.

#### Scenario: Valid frame is dispatched

- **WHEN** a complete frame ending in the correct CRC and trailer is received
- **THEN** the matching Katapult command handler runs and an acknowledged response is returned

#### Scenario: CRC mismatch is rejected

- **WHEN** a frame has an incorrect CRC
- **THEN** no command handler runs and the invalid portion is discarded

### Requirement: Firmware uses CRC16-CCITT

The firmware SHALL calculate the checksum using the standard Katapult CRC16-CCITT algorithm and SHALL NOT use CRC32 in the Katapult frame.

#### Scenario: Host checksum matches

- **WHEN** a host generates a frame using the Katapult CRC16-CCITT implementation
- **THEN** firmware accepts the frame as valid

### Requirement: CONNECT response metadata

The firmware SHALL respond to `CMD_CONNECT` with origin command, protocol version, application start address, block size, MCU type, and software version.

#### Scenario: Host reads connection metadata

- **WHEN** `CONNECT` succeeds
- **THEN** the host receives block size and application start address and can use them for subsequent transfers

### Requirement: Node identity and administrative commands

The firmware SHALL respond to CAN administrative discovery and shall expose a 6-byte UUID through `GET_CANBUS_ID`.

#### Scenario: Host identifies a node

- **WHEN** the host issues `GET_CANBUS_ID` after association
- **THEN** firmware returns an origin command and the same 6-byte UUID used during association

### Requirement: Busy and command error handling

The firmware SHALL return `ACK_BUSY` when it cannot service a request promptly and SHALL return `ACK_ERROR` when a command cannot be processed.

#### Scenario: Host retries after busy

- **WHEN** a flash operation is still pending
- **THEN** the firmware returns `ACK_BUSY` instead of corrupting the transfer
