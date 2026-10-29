## ADDED Requirements

### Requirement: Shared byte-stream framing

The USB transport SHALL feed received USB packet bytes into the same Katapult frame parser used by UART, CAN, and I2C.

#### Scenario: Host frame arrives over CDC

- **WHEN** a host sends a Katapult frame as one or multiple USB packets
- **THEN** the firmware accumulates bytes and dispatches the frame without transport-specific parsing

### Requirement: Stepwise response handling

The USB host transport SHALL acknowledge one complete request before accepting another framed request.

#### Scenario: Retrying a slow transfer

- **WHEN** a host transaction fails mid-transfer
- **THEN** the host retransmits without leaving the target’s receive buffer ambiguous

### Requirement: Transport-neutral protocol

Firmware and host tooling MUST share one `FlashSession` byte-stream adapter despite the USB topology.

#### Scenario: Adapter parity
- **WHEN** the USB adapter and a reference UART adapter are given the same valid packet order
- **THEN** both produce an equivalent dispatch call and framing result
