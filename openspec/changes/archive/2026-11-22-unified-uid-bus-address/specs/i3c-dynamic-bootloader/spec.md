# Spec Delta

## MODIFIED Requirements

### Requirement: Dynamic Address Assignment
The bootloader SHALL participate in MIPI I3C Dynamic Address Assignment (`ENTDAA`) and Direct Set Dynamic Address (`SETDASA`) using its hardware unique identifier and active dynamic address synchronized with the application.

#### Scenario: Bus initialization assigns dynamic address
- **WHEN** the I3C controller executes the `ENTDAA` sequence
- **THEN** the target outputs its 48-bit provisional ID derived deterministically from silicon registers and accepts the assigned dynamic 7-bit bus address
