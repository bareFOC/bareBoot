## ADDED Requirements

### Requirement: Cross-platform discovery

The host tooling SHALL support identifying and selecting CDC ACM devices, `/dev/ttyACM*`, and `COM*` through a common host transport trait.

#### Scenario: Linux host detects a target

- **WHEN** the host runs device discovery on Linux
- **THEN** the returned endpoint has a concrete `UsbBusTransport` behind a common `FlashSession`

### Requirement: Error visibility

A failed USB transaction SHALL distinguish:
- **Discovery errors**
- **Timeout/transfer errors**
- **Payload/frame errors**

#### Scenario: Verify after connect

- **WHEN** a target replies with an invalid response
- **THEN** the host does not advance beyond the current command and reports the exact class of failure
