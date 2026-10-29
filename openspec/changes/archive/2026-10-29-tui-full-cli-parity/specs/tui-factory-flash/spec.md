## ADDED Requirements

### Requirement: Probe-rs factory flash

The TUI SHALL support installing the bootloader via probe-rs, independent of bus-protocol flashing.

#### Scenario: No probe

- **WHEN** no probe is connected
- **THEN** bus protocol flashing and scanning remain available

### Requirement: Serial factory flash

The TUI SHALL support serial-based bootloader installation using the serial bootloader handshake, then flashing over serial backend.

#### Scenario: Serial handshake

- **WHEN** a user starts serial factory flash
- **THEN** the SERIAL_BL_REQ handshake is sent before flashing