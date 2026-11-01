## ADDED Requirements

### Requirement: Boot window and dual-interface arbitration

When more than one transport is enabled, the firmware SHALL start in an undecided state and SHALL lock to the first transport that receives a valid Katapult frame within the configured boot window. Once locked, the alternate transport SHALL not be used for the remainder of the session.

#### Scenario: CAN arrives first

- **WHEN** a valid frame arrives on CAN before I2C within the boot window
- **THEN** the firmware selects CAN and ignores I2C for the session

#### Scenario: No frame arrives

- **WHEN** the boot window expires without a valid frame
- **THEN** the firmware abandons further transport polling and boots the application

### Requirement: Non-blocking main loop

The firmware SHALL service transport receive buffers, command dispatch, countdown updates, and flash busy states without requiring an operating system.

#### Scenario: Slow host keeps firmware alive

- **WHEN** the host polls slowly
- **THEN** the firmware continues counting and can return busy responses while work remains pending

### Requirement: Application jump

The firmware SHALL jump to the configured application start address only after a successful `COMPLETE` and the configured idle delay.

#### Scenario: Completed transfer jumps once

- **WHEN** `COMPLETE` succeeds and the delay expires
- **THEN** the firmware performs one application jump and does not return to the bootloader
