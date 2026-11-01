## ADDED Requirements

### Requirement: Portable byte contract

The host CDC class driver SHALL only use packets and byte offsets, independent of endpoint addresses or descriptors.

#### Scenario: Frame digest

- **WHEN** a USB packet is fragmented into multiple STEM steps
- **THEN** transport-level framing remains unchanged
