## ADDED Requirements

### Requirement: Full frame staging

Every ACK SHALL be queued as a complete frame before requesting the next host packet.

#### Scenario: EOF response

- **WHEN** the host sends a `RX_EOF`
- **THEN** the target dispatches, verifies the flash, and returns one complete Katapult ACK frame before accepting another command
