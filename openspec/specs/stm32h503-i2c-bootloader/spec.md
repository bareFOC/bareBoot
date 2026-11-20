# stm32h503-i2c-bootloader Specification

## Purpose
Provides Katapult-compatible I2C target peripheral initialization, master write command reception, master read response staging, and application handoff for STM32H503 microcontrollers.

## Requirements

### Requirement: I2C Target Peripheral Initialization
The bootloader SHALL configure I2C2 on PB3 (SCL, AF8) and PB4 (SDA, AF8) as an I2C slave operating at standard 7-bit target address `0x40` with open-drain outputs and pull-ups.

#### Scenario: Peripheral clock and pins configured
- **WHEN** the bootloader boots up
- **THEN** GPIOB and I2C2 clocks are enabled, PB3 and PB4 are configured in AF8 open-drain mode, and I2C2 is enabled with own address 0x40

### Requirement: Katapult I2C Frame Reception
The bootloader SHALL receive Katapult commands sent as master write transactions on I2C address 0x40, buffering bytes into `I2C_RX_BUF` and signaling packet arrival upon detection of an I2C STOP condition.

#### Scenario: Master write command received
- **WHEN** the host writes a Katapult frame over I2C and issues a STOP condition
- **THEN** the bootloader copies the received frame to the dispatch buffer and triggers protocol execution

### Requirement: Katapult I2C Frame Response
The bootloader SHALL stage response bytes into `I2C_TX_BUF` and transmit them when the host performs a master read transaction on I2C address 0x40.

#### Scenario: Master read response retrieved
- **WHEN** the host reads bytes from I2C address 0x40 following a command
- **THEN** the bootloader transmits the staged response bytes via `TXIS` interrupts

### Requirement: Boot Window and Application Handoff
The bootloader SHALL wait for Katapult commands during a configurable boot window; if no valid command arrives before expiration, it SHALL jump to the application entry at `0x08004000`.

#### Scenario: Boot window timeout
- **WHEN** no Katapult I2C activity occurs within the boot window
- **THEN** the bootloader updates VTOR to 0x08004000, loads MSP, and executes the application reset vector

### Requirement: Non-Blocking Multidrop Operation and Configured Address Binding
The bootloader SHALL bind its I2C slave peripheral to the node address configured in non-volatile flash (defaulting to `0x40`) and SHALL process received blocks without holding SCL low (eliminating clock stretching) so that peer nodes on the shared bus remain unaffected.

#### Scenario: Operating alongside active peer nodes
- **GIVEN** a shared multidrop bus with active peer nodes
- **WHEN** the host transmits frames to a target node
- **THEN** the target bootloader acknowledges within hardware timing boundaries without asserting clock stretch on SCL
