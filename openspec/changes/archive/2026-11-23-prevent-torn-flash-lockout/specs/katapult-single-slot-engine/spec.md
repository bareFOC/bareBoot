# Spec Delta

## MODIFIED Requirements

### Requirement: Monotonic Bit-Flip Commit Record Tracking
The bootloader SHALL persist application update states within a dedicated commit flash sector using sequential monotonic bit-flipping writes without erasing the sector during state transitions across all supported MCU platforms including STM32.

#### Scenario: Update initiated marks in progress
- **WHEN** the first block of an update is written
- **THEN** the bootloader records `UPDATE_IN_PROGRESS` by programming the next available word in the commit sector without performing an erase

#### Scenario: Update completion marks valid
- **WHEN** the host issues `COMPLETE` (`0x15`) following successful transfer and verification
- **THEN** the bootloader records `STATE_VALID` by programming the subsequent word in the commit sector

#### Scenario: Corrupt or interrupted state traps recovery
- **WHEN** power is lost mid-update and the latest record is `UPDATE_IN_PROGRESS` or uncommitted
- **THEN** the bootloader remains in Katapult recovery mode across subsequent resets and does not boot the application

### Requirement: Safe Processor Handoff and Cache Invalidation
The bootloader SHALL invalidate the processor instruction cache, verify application vector validity including reset entry opcodes, and de-initialize active communication peripherals before branching to the application.

#### Scenario: Application jump on Cortex-M7 and Cortex-M33
- **WHEN** the boot decision selects application launch
- **THEN** the bootloader invalidates the instruction cache, updates the vector table offset register, and executes the application entry vector

#### Scenario: Torn application entry point rejected
- **WHEN** the application initial SP and Reset PC are within bounds but the opcode at the Reset PC is unprogrammed `0xFFFFFFFF` or zeroed `0x00000000`
- **THEN** the bootloader rejects the application as invalid and remains in bootloader recovery mode
