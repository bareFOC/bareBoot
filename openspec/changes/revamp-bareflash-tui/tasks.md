## 1. Theme Engine & Architectural Foundation

- [x] 1.1 Clean contaminated `ui.rs` and establish modular directory structure under `crates/bareboot-tui/src/` (`theme.rs`, `protocol.rs`, `app.rs`, `ui/`)
- [x] 1.2 Implement mathematical tone-ladder `Palette` struct with Ember (default), Nord, Gruvbox, Tokyo Night, and Synthwave palettes
- [x] 1.3 Add fixed semantic status colors (`ok` green, `err` red, `warn` amber) and live theme cycling via `T` key
- [x] 1.4 Write unit tests for theme palette creation and cycling

## 2. Protocol Decoder & Telemetry

- [x] 2.1 Implement `protocol.rs` with bootloader frame parsing (CAN IDs, I2C addresses, Serial framing)
- [x] 2.2 Decode bootloader opcodes (`PING`, `INFO`, `ERASE`, `WRITE_CHUNK`, `VERIFY`, `JUMP_APP`, `ACK`, `NAK`, `BUSY`) into structured events
- [x] 2.3 Integrate ring-buffer frame queue in `worker.rs` with pause/resume and rate-limiting support
- [x] 2.4 Add unit tests for opcode decoding and payload formatting

## 3. Layout Engine, Header & Sidebar

- [x] 3.1 Fix root layout constraints to allocate 100% vertical viewport height, replacing squished 3-row constraint
- [x] 3.2 Implement responsive centering container capping width at 120 columns on ultrawide displays and compact mode under 76 cols
- [x] 3.3 Build `header.rs` breadcrumb component showing wordmark, target name, transport type, bitrate, and live status badge
- [x] 3.4 Build `sidebar.rs` view switcher (Flash Studio, Bus Analyzer, Target Config, Build & Probe) with active profiles list and probe status
- [x] 3.5 Build `footer.rs` rendering context-sensitive key hints and activity indicator

## 4. Dedicated Views Implementation

- [x] 4.1 Implement `views/flash_studio.rs` with firmware metadata card, backend selector, braille progress gauge, transfer speed (KB/s), and block verify counters
- [x] 4.2 Implement `views/bus_analyzer.rs` rendering real-time packet table with timestamp, direction, node, hex dump, and decoded opcodes
- [x] 4.3 Implement `views/config_editor.rs` with interactive form fields and TOML syntax preview for `bareboot.toml`
- [x] 4.4 Implement `views/build_probe.rs` with live Cargo build output streaming and probe-rs hardware debugger listing

## 5. Overlays & Input Isolation

- [x] 5.1 Implement `ActiveOverlay` state machine isolating keystrokes from global hotkeys
- [x] 5.2 Implement `overlays/command_palette.rs` (`:` / `Ctrl+P`) with fuzzy filtering and instant action execution
- [x] 5.3 Implement `overlays/file_picker.rs` for selecting firmware binaries (`.bin`, `.elf`, `.hex`) with cursor navigation
- [x] 5.4 Implement `overlays/profile_picker.rs` and `overlays/confirm.rs` for profile management and destructive action guardrails
- [x] 5.5 Implement `overlays/help.rs` (`?`) displaying dynamic keybinding reference table

## 6. Integration & Verification

- [x] 6.1 Wire `App` state machine, worker event receivers, and keyboard/mouse event dispatchers
- [x] 6.2 Verify compilation and test suite with `cargo check` and `cargo test -p bareboot-tui`
- [x] 6.3 Test responsive scaling across varying terminal dimensions (narrow 80x24, standard 100x30, ultrawide 160x50)
- [x] 6.4 Validate flashing simulation, bus packet decoding, and theme toggling end-to-end

## 7. 3-Tab Architecture & Katapult Makeflash Pipeline

- [x] 7.1 Consolidate navigation from 4-view sidebar into 3 focused tabs (`Target Config`, `Flasher`, `Console`)
- [x] 7.2 Implement Katapult-style `make flash` pipeline with build target selection, release/debug mode, probe/Katapult deploy method, chip erase, and verify toggles
- [x] 7.3 Support full mouse hit-testing for tab switching, buttons, and scrolling alongside keyboard shortcuts (`1`, `2`, `3`, `Space`, `b`, `f`, `t`, `m`, `d`, `e`, `v`, `p`, `r`)
- [x] 7.4 Combine Bus Analyzer traffic table and diagnostic console log streams in Tab 3 Console view
- [x] 7.5 Verify all unittests and compilation across all crates

## 8. UX Refinement: Modals, Scroll Isolation & Post-Flash Target Config

- [x] 8.1 Isolate mouse scroll events so wheel navigation scrolls view content (settings, logs, modals) and never switches tabs
- [x] 8.2 Redesign Tab 1 as post-flash over-the-wire bootloader configurator (read/write runtime addresses, enable/disable CAN, UART, USB, I2C, boot timeout, jump to app, reboot target)
- [x] 8.3 Move Katapult menuconfig bootloader build settings (MCU part number, pinout, transports, app offset, LED pin, boot entry) into the Flasher Tab
- [x] 8.4 Implement Profile Save (`S`) and Load (`P`) system with built-in presets and user-saved profiles on disk
- [x] 8.5 Implement interactive `SelectModal` and `InputModal` for all configuration settings in addition to direct cycle hotkeys (`t`, `d`, `m`, `c`, `u`, `e`, `v`)
- [x] 8.6 Verify all tests and automated unit test suite across `bareboot-tui` and `bareboot-client`
