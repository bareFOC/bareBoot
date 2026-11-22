# web-flasher Specification

## Purpose

Enables web browser execution of the BareBoot GUI application via WebAssembly, supporting direct USB-CDC device flashing over the WebSerial API without native desktop installations.

## Requirements

### Requirement: WebAssembly browser runtime initialization
The system SHALL compile to `wasm32-unknown-unknown` and render the complete graphical interface inside an HTML canvas via `eframe::WebRunner`.

#### Scenario: User loads web application
- **WHEN** the user opens the web application URL in a WebGL-capable browser
- **THEN** the interface initializes in the browser canvas and displays the Target Configuration, Documentation, and Flasher views

### Requirement: WebSerial bus transport
The system SHALL implement the HostBus abstraction over the browser WebSerial API to exchange Katapult frames with attached USB serial devices.

#### Scenario: Browser requests serial port connection
- **WHEN** the user initiates a connection on a supported Chromium browser
- **THEN** the system triggers `navigator.serial.requestPort()` and establishes a bidirectional reader and writer stream for Katapult frame transfers

### Requirement: Drag-and-drop firmware upload in web
The web application SHALL allow users to upload firmware binaries into memory via drag-and-drop or standard file selection inputs.

#### Scenario: User drops binary onto web application
- **WHEN** the user drags and drops a `.bin` or `.uf2` file onto the browser viewport
- **THEN** the web application captures the file bytes, displays the file name and size in the flasher panel, and prepares it for flashing
