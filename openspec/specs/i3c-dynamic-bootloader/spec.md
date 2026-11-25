# i3c-dynamic-bootloader Specification

## Purpose

Enables Katapult wire-protocol flashing over MIPI I3C and I2C target interfaces, featuring dynamic protocol detection, standardized hardware reset via Common Command Codes, and non-blocking multi-drop bus communication.

## Requirements

### Requirement: Dynamic Bus Protocol Detection
The bootloader SHALL monitor the shared communication lines and automatically switch from standard I2C target mode to MIPI I3C target mode upon detecting an I3C controller broadcast header.

#### Scenario: Controller broadcast detected
- **WHEN** the target peripheral detects an I3C broadcast address `0x7E`
- **THEN** the bootloader switches GPIO alternate functions and activates the I3C target controller

#### Scenario: Standard I2C traffic
- **WHEN** standard I2C master frames are received addressed to the configured target address
- **THEN** the bootloader remains in I2C target mode and processes the transaction

### Requirement: MIPI CCC RSTACT and Target Reset Pattern Entry
The bootloader and application runtime SHALL intercept MIPI Common Command Code `RSTACT` (`0x2A` broadcast, `0x9A`/`0xAA` direct) with defining byte `0x02` and hardware I3C Target Reset Pattern (`EVR_RSTF`, bit 27) to clear `BootLoopGuard` and trigger a system reset into BareBoot recovery mode.

#### Scenario: Broadcast or direct RSTACT received
- **WHEN** an I3C controller issues CCC `RSTACT` (`0x2A`, `0x9A`, or `0xAA`) with parameter `0x02`
- **THEN** the target clears `BootLoopGuard`, saves the bootloader recovery cookie to the SRAM mailbox, and triggers an immediate system reset

#### Scenario: Target Reset Pattern received
- **WHEN** an I3C controller drives the Target Reset Pattern on `SDA + SCL` triggering `EVR_RSTF`
- **THEN** the application disarms drivers and resets into BareBoot recovery (unless `DEVR0.RSTVAL=1` and `RSTACT=0`), and BareBoot clears `BootLoopGuard` and parks in `ENTDAA`-ready recovery mode

### Requirement: Non-Blocking Katapult Packet Processing
The I2C and I3C target transports SHALL process Katapult command packets into RAM buffers without holding SCL low for flash erase or write cycles, avoiding bus clock stretching.

#### Scenario: Host writes command frame
- **WHEN** an I2C/I3C master writes a Katapult frame
- **THEN** the target acknowledges the transfer into a buffer and executes flash programming asynchronously without clock stretching

#### Scenario: In-Band Interrupt notification
- **WHEN** an asynchronous flash operation completes on an I3C bus
- **THEN** the target asserts an In-Band Interrupt (IBI) to notify the controller that the Katapult response frame is ready for readout

### Requirement: Dynamic Address Assignment
The bootloader SHALL participate in MIPI I3C Dynamic Address Assignment (`ENTDAA`) and Direct Set Dynamic Address (`SETDASA`) using its hardware unique identifier and active dynamic address synchronized with the application.

#### Scenario: Bus initialization assigns dynamic address
- **WHEN** the I3C controller executes the `ENTDAA` sequence
- **THEN** the target outputs its 48-bit provisional ID derived deterministically from silicon registers and accepts the assigned dynamic 7-bit bus address

#### Scenario: Direct SETDASA received
- **WHEN** an I3C controller issues CCC `SETDASA` (`0x87`) with a valid 7-bit dynamic address for the target
- **THEN** the bootloader updates `I3C2_DEVR0` (`DAVAL=1`) and cancels any auto-boot countdown timer
