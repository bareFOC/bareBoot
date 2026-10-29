## ADDED Requirements

### Requirement: Build and flash experimental USB bootloader

A CH32V203 USB-capable image SHALL be built using a host for iterative bring-up and shall fit a developer-selected 8 or 16 KiB size limit.

#### Scenario: Build target

- **WHEN** the config selects transports `["usb"]` and an `8 KiB` offset
- **THEN** the resulting image is mountable and flashable via WCH-Link in RV mode

### Requirement: USB endpoint bring-up

The target SHALL enumerate as a CDC serial bridge and support packet-level read, write, and interrupt parsing on `PA11` and `PA12`.

#### Scenario: Connect frame accepted

- **WHEN** the host sends `CONNECT` to the CDC serial adapter
- **THEN** the firmware returns an ACK with valid Katapult metadata
