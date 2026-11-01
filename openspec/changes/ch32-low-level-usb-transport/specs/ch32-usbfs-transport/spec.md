## ADDED Requirements

### Requirement: Low-level USB CDC

The CH32 target SHALL support the USB CDC descriptor and bulk endpoints needed for Katapult-compatible byte exchange.

#### Scenario: USB boot enumerates

- **WHEN** a fresh CH32 deployment is flashed
- **THEN** the host detects a CDC ACM device whose VID/PID and Major serial number are discoverable and whose serial bytes are a valid Katapult transport

### Requirement: 256-byte page flash align

The low-level target MUST integrate the CH32 256-byte flash page pair through `write_page_bytes` and `erase_page` before declaring a successful transport.

#### Scenario: Flash test pipeline

- **WHEN** a 64-byte frame is written at a known page offset
- **THEN** the firmware erases the aligned 256-byte page, schedules the 64-byte portion, and the host confirms the vector bytes match
