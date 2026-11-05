## ADDED Requirements

### Requirement: Bootloader packet decoding

The TUI SHALL capture bus frames across CAN, I2C, and Serial and decode bootloader transactions into human-readable operations (`PING`, `INFO`, `ERASE`, `WRITE_CHUNK`, `VERIFY`, `JUMP_APP`, `ACK`, `NAK`, `BUSY`).

#### Scenario: Decode CAN frame

- **WHEN** a CAN frame with ID 0x100 containing opcode 0x01 is received
- **THEN** the bus analyzer displays `PING` with payload bytes and node ID

#### Scenario: Decode ACK response

- **WHEN** a response frame containing byte 0x06 is captured
- **THEN** the bus analyzer tags the entry with `ACK` styled with the `ok` color

### Requirement: Protocol bus stream filtering

The Protocol Bus Analyzer SHALL support pausing the packet feed, clearing the buffer, and filtering by direction (TX/RX) or node ID.

#### Scenario: Pause bus feed

- **WHEN** user presses `Space` on the Protocol Bus view
- **THEN** incoming frame scrolling pauses while packets continue buffering in memory
