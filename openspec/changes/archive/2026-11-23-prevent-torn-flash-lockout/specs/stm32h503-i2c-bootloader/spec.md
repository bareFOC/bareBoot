# Spec Delta

## MODIFIED Requirements

### Requirement: Boot Window and Application Handoff
The bootloader SHALL wait for Katapult commands during a configurable boot window; if no valid command arrives before expiration, it SHALL jump to the application entry at `0x08004000` (or configured `APP_BASE`) only if the application vector table, entry opcode, and commit state are fully verified.

#### Scenario: Boot window timeout
- **WHEN** no Katapult I2C activity occurs within the boot window and a valid application with verified entry code exists
- **THEN** the bootloader updates VTOR to the application base, loads MSP, and executes the application reset vector

#### Scenario: Torn flash traps permanently in recovery mode
- **WHEN** no Katapult I2C activity occurs within the boot window but the application slot contains unprogrammed opcodes (`0xFFFFFFFF`) or uncommitted status
- **THEN** the bootloader cancels the launch timer and remains permanently in I2C recovery mode at address 0x40
