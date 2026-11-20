# stm32h503-i2c-bootloader Delta Specification

## ADDED Requirements

### Requirement: Non-Blocking Multidrop Operation and Configured Address Binding
The bootloader SHALL bind its I2C slave peripheral to the node address configured in non-volatile flash (defaulting to `0x40`) and SHALL process received blocks without holding SCL low (eliminating clock stretching) so that peer nodes on the shared bus remain unaffected.

#### Scenario: Operating alongside active peer nodes
- **GIVEN** a shared multidrop bus with active peer nodes
- **WHEN** the host transmits frames to a target node
- **THEN** the target bootloader acknowledges within hardware timing boundaries without asserting clock stretch on SCL
