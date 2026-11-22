# Tasks

## 1. Engine Extraction (crates/bareboot-engine)

- [x] 1.1 Create `crates/bareboot-engine` crate manifest and add it to the root `Cargo.toml` workspace members; verify with `cargo check -p bareboot-engine`
- [x] 1.2 Move worker channel primitives (`WorkerHandles`, `WorkerCommand`, `UiEvent`) from `bareboot-tui` to `bareboot-engine::worker` and verify unit tests pass
- [x] 1.3 Move compilation streaming (`BuildJob`, `stream_cargo_build`, `stream_makeflash`, `discover_firmware`) and probe-rs runners to `bareboot-engine::build` and `bareboot-engine::probe`; verify with unit tests
- [x] 1.4 Refactor `host/bareboot-tui` to import and consume `bareboot-engine`; verify existing TUI builds and passes tests with `cargo test -p bareboot-tui`

## 2. Configuration & Profile Storage Integration

- [x] 2.1 Add `craft-settings` and `craft-atomic` dependencies to `bareboot-engine`
- [x] 2.2 Implement atomic loading, validation, saving, and deletion of `FlashProfile` and `BarebootConfig` using `craft-settings`; verify with round-trip unit tests in `bareboot-engine`
- [x] 2.3 Verify profile directory migration and fallback presets (`configs/bareboot.toml`, `profiles/*.toml`) work under the new engine persistence layer

## 3. Desktop GUI Scaffold & Docking Layout (host/bareboot-gui)

- [x] 3.1 Create `host/bareboot-gui` crate with `eframe`, `egui`, and `craft-ui` dependencies, registered in the workspace; verify with `cargo check -p bareboot-gui`
- [x] 3.2 Implement main `eframe::App` lifecycle and initialize `bareboot-engine::worker::WorkerHandles` on startup
- [x] 3.3 Construct the multi-panel dockable workspace using `craft-ui` docking surfaces (Left: Target & Profile Inspector, Center-Top: Makeflash Control & Gauge, Center-Bottom: Cargo Build Console, Right: Bus Traffic Sniffer); verify layout renders cleanly on desktop

## 4. Makeflash, Target Config & Bus Analyzer Views

- [x] 4.1 Implement the visual Target Configuration Editor panel in `bareboot-gui` allowing form-based editing of MCU family, baud rates, pin allocations, and boot window timings; verify save/reload actions update active state
- [x] 4.2 Connect the Makeflash action button and sub-actions (Cargo Build Only, Flash Only, Target Reset, Chip Erase, Verify toggle) to `WorkerCommand::MakeFlash` and bind progress gauges to `UiEvent::Progress`
- [x] 4.3 Implement the Live Bus Traffic table view using `craft-ui` styled rows, displaying timestamps, direction (TX/RX), bus type (CAN, Serial, I2C), decoded opcodes, and formatted hex payload bytes; verify live events populate from `UiEvent::BusFrame`
- [x] 4.4 Implement the live Cargo build console panel with auto-scrolling log view and color-coded build diagnostics (warnings, errors, success); verify build output streams during a Makeflash run

## 5. In-App Documentation & Manual Viewer

- [x] 5.1 Embed project markdown files (`docs/handsfree-app-updates.md`, `README.md`, protocol commands reference) into `bareboot-gui` using asset inclusion macros
- [x] 5.2 Build an interactive Documentation & Manual panel with a searchable topic tree and rendered Markdown viewer using `egui_commonmark` or `craft-ui` text styling
- [x] 5.3 Verify documentation displays cleanly with table formatting, code snippets, and search filtering across Desktop, Android, and Web viewports

## 6. Android USB Host Transport & Mobile Viewport

- [x] 6.1 Implement `AndroidUsbBus` in `bareboot-client` implementing the `HostBus` trait over native Android USB file descriptors (`RawFd`) via bulk endpoint transfers; verify compilation with `#[cfg(target_os = "android")]`
- [x] 6.2 Add Android packaging configuration (`AndroidManifest.xml` with USB host permissions and intent filters) and JNI bridge to pass `UsbDeviceConnection` file descriptors into Rust
- [x] 6.3 Implement responsive layout adaptation: detect mobile viewport width (< 900px) and present a streamlined tabbed view (Flasher, Config, Docs, Traffic) without multi-dock clutter
- [x] 6.4 Implement Android Storage Access Framework file picker integration and pre-compiled bootloader catalog selector; verify that firmware binaries can be selected and dispatched to `bareboot-client::run_flash`

## 7. WebAssembly Target & WebSerial Transport

- [x] 7.1 Configure WebAssembly build setup (`Trunk.toml`, `index.html`) compiling `bareboot-gui` with `wasm32-unknown-unknown` and `eframe::WebRunner` supporting GitHub Pages deployment (relative public URL `./`)
- [x] 7.2 Implement `WebSerialBus` in `bareboot-client` using `web-sys` to interface with the browser's `navigator.serial` API; verify Katapult frames exchange over WebSerial
- [x] 7.3 Implement drag-and-drop file upload and pre-compiled artifact selector in the web interface; verify that dropping a `.bin` or `.uf2` file loads the firmware image for flashing
- [x] 7.4 Test web application build via `trunk build` and verify that documentation, configuration editor, and WebSerial flasher operate in Chromium browsers
- [x] 7.5 Create GitHub Actions workflow at `.github/workflows/deploy-pages.yml` to build with trunk and deploy `dist/` artifacts to GitHub Pages over HTTPS

## 8. End-to-End Verification & Documentation

- [x] 8.1 Run full workspace tests with `cargo test --workspace` ensuring all crates (`bareboot-protocol`, `bareboot-core`, `bareboot-client`, `bareboot-engine`, `bareboot-cli`, `bareboot-tui`, `bareboot-gui`) build and pass without regressions
- [x] 8.2 Test desktop GUI handsfree flashing workflow in simulated memory mode (`DeployMethod::MemorySim`) and verify progress bar reaches 100% and displays completion metrics
- [x] 8.3 Document the BareBoot GUI architecture, build steps, desktop workflows, WebAssembly hosting, and Android USB OTG field guide in `docs/bareboot-gui.md`

## Workflow follow-up

- Archive the change after implementation and verification are complete.
- Sync delta specs to main specs (`openspec spec sync bareboot-gui-craftlibs`).
