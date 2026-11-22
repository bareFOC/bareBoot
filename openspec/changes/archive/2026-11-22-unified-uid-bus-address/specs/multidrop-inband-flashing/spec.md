# Spec Delta

## MODIFIED Requirements

### Requirement: Selective In-Band Bootloader Entry
The system SHALL support entering bootloader mode on a specific node without resetting or interrupting peer nodes on the shared bus, preserving active dynamic bus addresses across the reset handoff.

#### Scenario: Single-byte strobe on addressed target
- **GIVEN** multiple nodes on an I2C or I3C bus (addresses `0x40`, `0x41`, `0x42`)
- **WHEN** the host sends single byte `0xF0` addressed to `0x41`
- **THEN** node `0x41` writes `0x5984E3FA6CA1589B` along with active address `0x41` to its SRAM mailbox and resets into BareBoot, while nodes `0x40` and `0x42` ignore the message and continue operating
