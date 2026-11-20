# Multidrop In-Band Flashing Specification

## ADDED Requirements

### Requirement: Selective In-Band Bootloader Entry
The system SHALL support entering bootloader mode on a specific node without resetting or interrupting peer nodes on the shared bus.

#### Scenario: Single-byte strobe on addressed target
- **GIVEN** multiple nodes on an I2C or I3C bus (addresses `0x40`, `0x41`, `0x42`)
- **WHEN** the host sends single byte `0xF0` addressed to `0x41`
- **THEN** node `0x41` writes `0x5984E3FA6CA1589B` to its SRAM mailbox and resets into BareBoot, while nodes `0x40` and `0x42` ignore the message and continue operating

### Requirement: Python Multi-Drop Batch Flasher
The host toolchain SHALL provide a multi-drop orchestrator script that discovers nodes, commands bootloader entry, flashes binary images sequentially, and verifies application integrity.

#### Scenario: Updating a 3-joint bus
- **GIVEN** a bus with three nodes configured in a YAML/JSON topology file
- **WHEN** `multidrop_flashtool.py` executes
- **THEN** each node is flashed and verified in sequence, culminating in a global verification summary
