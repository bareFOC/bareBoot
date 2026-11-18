## ADDED Requirements

### Requirement: Commit state persists across reset
The RP2350 USB BareBoot target SHALL maintain a persistent commit state in a dedicated flash sector within its own 32 KB region, independent of the application slot, with at least the states `VALID` and `UPDATE_IN_PROGRESS`.

#### Scenario: Commit state survives cold power cycle
- **WHEN** BareBoot sets commit state to `VALID` and the chip is fully power-cycled
- **THEN** BareBoot reads commit state as `VALID` after reset

#### Scenario: Commit state is corruption-detectable
- **WHEN** the commit-state record is read and its state/complement pair does not match
- **THEN** BareBoot treats the state as not `VALID` and stays in recovery

### Requirement: Transfer start marks update in progress
The bootloader SHALL set commit state to `UPDATE_IN_PROGRESS` when the first `SEND_BLOCK` (`0x12`) of a transfer is accepted.

#### Scenario: First block begins transfer
- **WHEN** BareBoot accepts a `SEND_BLOCK` command while commit state is `VALID`
- **THEN** commit state becomes `UPDATE_IN_PROGRESS` before any application flash page is erased

### Requirement: COMPLETE commits a verified image
The bootloader SHALL set commit state to `VALID` only after a `COMPLETE` (`0x15`) command is processed successfully.

#### Scenario: Verified transfer commits
- **WHEN** the host sends `COMPLETE` after a fully written and readback-verified image
- **THEN** BareBoot sets commit state to `VALID` and the application becomes bootable on the next reset

#### Scenario: Interrupted transfer does not commit
- **WHEN** a transfer is aborted before `COMPLETE` (partial blocks written)
- **THEN** commit state remains `UPDATE_IN_PROGRESS` and BareBoot does not boot the application on the next reset

### Requirement: Boot decision requires valid commit state
The boot decision SHALL enter recovery when commit state is not `VALID`, even if the application vector appears valid and no cookie is present.

#### Scenario: Torn image stays in recovery
- **WHEN** commit state is `UPDATE_IN_PROGRESS` and the application SP/PC words look valid
- **THEN** BareBoot enters USB Katapult recovery instead of jumping to the application

#### Scenario: Valid committed image boots
- **WHEN** commit state is `VALID`, no recovery cookie is present, and the application vector is valid
- **THEN** BareBoot clears the cookie (if present) and jumps to the application

### Requirement: No BOOTSEL fallback on flash error
The bootloader SHALL NOT call `reset_to_usb_boot()` or enter ROM USB boot under any runtime error condition.

#### Scenario: Flash write failure keeps BareBoot in its own recovery
- **WHEN** a flash erase or write error occurs during an update
- **THEN** BareBoot replies with `COMMAND_ERROR (0xf2)` and then resets into BareBoot USB recovery, with commit state remaining `UPDATE_IN_PROGRESS`

### Requirement: Katapult-compatible vector validation
The application vector validation SHALL require the reset PC to have the Thumb bit set (`pc & 1 == 1`), matching the Katapult reference `application_check_valid()`.

#### Scenario: Non-Thumb reset vector rejected
- **WHEN** the application reset PC is within the application flash region but has bit 0 clear
- **THEN** BareBoot treats the application as invalid and enters recovery
