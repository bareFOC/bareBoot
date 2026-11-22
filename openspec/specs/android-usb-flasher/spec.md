# android-usb-flasher Specification

## Purpose

Provides field-level microcontroller flashing and device configuration capabilities on Android mobile devices over USB OTG without requiring root operating system privileges.

## Requirements

### Requirement: Android USB Host bus transport
The system SHALL implement the HostBus abstraction over Android USB Host file descriptors to exchange Katapult protocol frames via USB bulk endpoints.

#### Scenario: Frame exchange over USB bulk endpoint
- **WHEN** a Katapult request frame is dispatched to an attached USB target on Android
- **THEN** the system transmits the frame via the USB bulk OUT endpoint and reads the corresponding response from the bulk IN endpoint

### Requirement: Permission-guarded USB device connection
The application SHALL request Android USB device permission for connected USB CDC serial adapters and USB-CAN dongles before initiating communication.

#### Scenario: USB permission granted by user
- **WHEN** the user approves the Android system USB connection dialog
- **THEN** the application claims the USB interface, acquires the native file descriptor, and transitions the transport state to ready

#### Scenario: USB device detached during transfer
- **WHEN** an attached USB target is physically disconnected during an active session
- **THEN** the transport terminates gracefully and surfaces an error event to the user interface

### Requirement: Mobile firmware selection and pre-compiled catalog
The application on Android SHALL support selecting user-provided firmware binaries through the system file picker and provide an offline catalog of pre-compiled target bootloader images.

#### Scenario: User selects external firmware binary
- **WHEN** the user selects a binary using the Android Storage Access Framework picker
- **THEN** the application loads the binary payload into memory and validates its size against the target flash boundaries
