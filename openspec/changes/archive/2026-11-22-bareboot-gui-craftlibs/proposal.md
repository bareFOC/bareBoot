# Proposal

## Why

BareBoot currently relies on the `bareboot-cli` tool and `bareboot-tui` Ratatui cockpit for device management, configuration, compilation, and flashing. While effective in terminal environments, developers, field technicians, and web users lack a visual, cross-platform graphical workbench that supports rich multi-panel layouts (visual target configuration, compilation consoles, live packet sniffers, and flash progress) across desktop operating systems (Linux, macOS, Windows), portable mobile devices on Android over USB OTG, and modern web browsers via WebAssembly.

Furthermore, developers frequently consult technical documentation, register definitions, pinout diagrams, and handsfree update procedures while configuring devices. Embedding documentation and user manuals directly within the GUI application provides an offline, self-contained reference and unifies docs delivery on desktop, mobile, and the web.

Integrating `storytold/craft-libs` (`craft-ui` and `craft-settings` on top of `egui`/`eframe`) delivers a unified 100% Rust GUI architecture across desktop, mobile, and web without foreign language bridges, JavaScript runtimes, or heavy web engines.

## What Changes

- Extract core background execution, worker channel orchestration (`WorkerCommand`, `UiEvent`), and profile persistence from `host/bareboot-tui` into a shared, reusable `crates/bareboot-engine` crate.
- Introduce `host/bareboot-gui`, an `eframe`/`egui` graphical application utilizing `storytold/craft-libs`:
  - `craft-ui`: Dockable multi-panel layouts, splitters, tab bars, SVG iconography, and cohesive dark/light themes.
  - `craft-settings`: Crash-safe, atomic TOML profile and configuration storage (`craft-atomic`).
- Implement visual Target Configuration Editor for offline board definitions (`configs/*.toml`) and live parameter inspection/updating over the wire.
- Implement Desktop Katapult "Makeflash" Pipeline: Local invocation of `cargo build` for embedded target architectures (`thumbv7m`, `thumbv8m`, `riscv32imac`), automatic output artifact discovery, and immediate deployment via Probe-rs, Serial, or CAN.
- Implement Android USB OTG Transport (`AndroidUsbBus` implementing `HostBus`) in `bareboot-client`/`bareboot-engine`:
  - Interfacing with Android's `UsbManager` and `UsbDeviceConnection` to acquire native USB file descriptors without root permissions.
  - Driving USB-CDC serial bootloaders and USB-CAN adapters (CandleLight / GS_USB, SLCAN) directly on mobile devices.
- Implement WebAssembly (WASM) Web Application Target:
  - Compiling `bareboot-gui` to `wasm32-unknown-unknown` hosted via `eframe::WebRunner` and Trunk.
  - Implementing `WebSerialBus` over the WebSerial API (`navigator.serial`) in Chromium browsers for direct browser-to-MCU flashing.
  - Scoping Web, like Android, to field configuration, diagnostics, and binary flashing (pre-compiled catalog and file upload/drag-and-drop), without local compilation.
- Implement In-App Documentation & Manual Viewer:
  - Embedding project markdown documentation, protocol specs, architecture guides, and pinout manuals directly into the GUI application.
  - Providing an integrated offline documentation browser on desktop, mobile, and web.
- Implement Live Bus Traffic Sniffer & Packet Inspector displaying decoded Katapult frames, opcodes, directions, and hex payloads in real time.

## Capabilities

### New Capabilities
- `bareboot-gui-app`: Cross-platform graphical cockpit for managing BareBoot devices built on `craft-ui` (`egui`) and `craft-settings`, providing dockable workspaces, visual configuration, makeflash pipelines, and traffic inspection.
- `bareboot-engine`: Shared headless background execution and profile orchestration service extracted from `bareboot-tui` for reuse across CLI, TUI, and GUI frontends.
- `android-usb-flasher`: Android USB Host OTG transport and field flashing capability enabling direct flashing and register configuration from mobile devices without root.
- `web-flasher`: WebAssembly browser target providing WebSerial-based MCU flashing, drag-and-drop firmware upload, and web-native Katapult frame transfers.
- `in-app-documentation`: Integrated markdown documentation viewer and user manual browser embedded directly within the application layout.

### Modified Capabilities
<!-- No requirement changes to existing specs -->

## Impact

- **Affected Crates**:
  - `host/bareboot-tui`: Refactored to consume shared worker and profile models from `crates/bareboot-engine`.
  - `host/bareboot-client`: Extended with Android USB Host and WebSerial bus implementations.
  - `host/bareboot-gui`: New workspace member implementing the GUI application targeting desktop, Android, and WebAssembly.
  - `crates/bareboot-engine`: New workspace crate owning worker threads, channels, and profile management.
- **External Dependencies**:
  - `craft-ui` (git: `https://github.com/storytold/craft-libs`)
  - `craft-settings`
  - `craft-atomic`
  - `egui` and `eframe` (v0.31+)
  - `web-sys` / `wasm-bindgen` for WebSerial and WebAssembly runtime
  - `egui_commonmark` or embedded markdown renderer for documentation
- **APIs**:
  - Extends `HostBus` trait coverage to Android USB file descriptors and WebSerial streams.
  - Standardizes `WorkerCommand` and `UiEvent` into public engine APIs.
