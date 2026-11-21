# katapult-single-slot-engine Specification

## Purpose

Provides a single-slot in-situ Katapult flashing engine that eliminates A/B partition overhead while guaranteeing recovery resilience using monotonic bit-flip commit records, continuous watchdog servicing, safe processor cache invalidation, and strict non-volatile memory (NVM) retention across updates.

## Requirements

### Requirement: Single-Slot Direct In-Situ Programming
The bootloader SHALL program incoming Katapult flash blocks directly into a single target application partition without requiring secondary staging partitions, A/B slots, or flash swap operations.

#### Scenario: Direct flash write
- **WHEN** a valid Katapult `SEND_BLOCK` (`0x12`) command is received
- **THEN** the bootloader writes the block into the single application slot at the specified address

#### Scenario: Maximum flash availability
- **WHEN** the partition table is configured
- **THEN** the application partition occupies all flash memory above the bootloader and commit sectors up to the file system boundary

### Requirement: NVM Partition Preservation Across Updates
The bootloader SHALL strictly bound all flash erase and program operations to the designated application slot `[APP_BASE, APP_END)` and SHALL NOT modify sectors designated for non-volatile storage or user file systems.

#### Scenario: Write within bounds accepted
- **WHEN** a block write target address is within `[APP_BASE, APP_END)`
- **THEN** the bootloader programs the block into flash

#### Scenario: Write outside bounds rejected
- **WHEN** a block write attempts to target addresses at or above `APP_END` (e.g. `H5FlashStorage` sectors `0x0801_C000..0x0802_0000`)
- **THEN** the bootloader rejects the write with `COMMAND_ERROR` (`0xf2`), leaving the NVM storage untouched

#### Scenario: Calibration retention across update
- **WHEN** an application update completes and the device reboots
- **THEN** previously persisted configuration data in NVM remains identical and valid

### Requirement: Monotonic Bit-Flip Commit Record Tracking
The bootloader SHALL persist application update states within a dedicated commit flash sector using sequential monotonic bit-flipping writes without erasing the sector during state transitions.

#### Scenario: Update initiated marks in progress
- **WHEN** the first block of an update is written
- **THEN** the bootloader records `UPDATE_IN_PROGRESS` by programming the next available word in the commit sector without performing an erase

#### Scenario: Update completion marks valid
- **WHEN** the host issues `COMPLETE` (`0x15`) following successful transfer and verification
- **THEN** the bootloader records `STATE_VALID` by programming the subsequent word in the commit sector

#### Scenario: Corrupt or interrupted state traps recovery
- **WHEN** power is lost mid-update and the latest record is `UPDATE_IN_PROGRESS` or uncommitted
- **THEN** the bootloader remains in Katapult recovery mode across subsequent resets and does not boot the application

### Requirement: Automated Watchdog Servicing During Flash Operations
The bootloader SHALL feed active hardware watchdogs before and during every flash page erase and block write operation.

#### Scenario: Multi-sector erase loop
- **WHEN** the bootloader executes consecutive page erases or extended flash writes
- **THEN** the hardware watchdog is serviced prior to each hardware flash operation, preventing watchdog resets

### Requirement: Safe Processor Handoff and Cache Invalidation
The bootloader SHALL invalidate the processor instruction cache and de-initialize active communication peripherals before branching to the application.

#### Scenario: Application jump on Cortex-M7 and Cortex-M33
- **WHEN** the boot decision selects application launch
- **THEN** the bootloader invalidates the instruction cache, updates the vector table offset register, and executes the application entry vector
