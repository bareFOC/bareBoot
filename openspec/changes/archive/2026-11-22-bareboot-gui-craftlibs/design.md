# Design

## Context

BareBoot's host architecture currently contains `bareboot-client` (session management, CRC verification, and `HostBus` abstractions) and two CLI/TUI frontends (`bareboot-cli`, `bareboot-tui`). In `bareboot-tui`, asynchronous compilation (`build::stream_cargo_build`, `build::stream_makeflash`), target detection (`build::discover_firmware`), probe-rs integration (`probe::run_probe_command`), and worker channel loops (`worker::WorkerHandles`) are bundled directly within the terminal crate.

To introduce a graphical user interface based on `storytold/craft-libs` that supports Desktop (Linux, macOS, Windows), Android mobile devices, and WebAssembly (WASM) in modern web browsers, the background execution and configuration management must be decoupled into an independent engine crate (`crates/bareboot-engine`), the `HostBus` transport layer must be extended to interface with Android USB Host and browser WebSerial APIs, and an in-app documentation browser must be embedded for unified offline/online manuals.

See `proposal.md` for motivation and high-level requirements.

## Goals / Non-Goals

**Goals:**
- Extract background worker threads, profile persistence, and Makeflash orchestration into `crates/bareboot-engine` so CLI, TUI, and GUI frontends share identical execution engines.
- Create `host/bareboot-gui` using `eframe` (`egui`) styled and laid out with `craft-ui` docking surfaces, tab bars, toolbars, and theme states across Desktop, Android, and WebAssembly.
- Integrate `craft-settings` and `craft-atomic` for robust, crash-safe TOML profile and board definition storage.
- Implement an Android USB OTG transport (`AndroidUsbBus`) that operates over the Android USB Host API without root access.
- Implement a WebAssembly browser target (`WebSerialBus`) enabling direct Katapult flashing in Chromium browsers without any local software installation.
- Provide an integrated In-App Documentation & Manual Viewer embedding all project guides, pinout maps, and protocol references.
- Implement a tri-platform capability model: Desktop provides full local Cargo compilation and multi-transport flashing; Android and Web operate as field flasher/configurators relying on pre-compiled catalogs and file upload/drag-and-drop.

**Non-Goals:**
- Local target firmware compilation on Android or Web (compilation is strictly desktop-only; mobile and web rely on pre-compiled catalogs or local file imports).
- Web browser SocketCAN support (browsers have no access to raw CAN hardware sockets; web flashing focuses on WebSerial USB-CDC and WebUSB).
- Replacing `bareboot-tui` or `bareboot-cli` (the TUI and CLI remain fully functional and share the extracted engine).

## Decisions

### Decision 1: GUI Stack: `storytold/craft-libs` on `egui`/`eframe`
- **Choice**: Use `craft-ui` and `craft-settings` from `storytold/craft-libs` over `eframe`.
- **Rationale**:
  - `craft-ui` provides an established design system tailored for industrial, CAD, and DAWs with dockable surfaces, tab fitting, split layouts, and SVG icon systems.
  - Keeps the entire stack 100% Rust, avoiding JS/Webview (Tauri) or Dart (Flutter) runtimes.
  - Native WebAssembly compatibility: `egui` and `craft-ui` render directly into WebGL/WebGPU canvases with 60 FPS performance and identical look-and-feel across Desktop, Android, and Web.
- **Alternatives Considered**:
  - *Tauri v2*: Lacks native web deployment without separate web hosting and requires different runtime wrappers for Android and Web.
  - *Slint*: WebAssembly backend exists but has larger web bundles and lacks dockable pane systems.

### Decision 2: Extraction of `bareboot-engine`
- **Choice**: Extract `worker.rs`, `build.rs`, `probe.rs`, `protocol.rs`, and `profile.rs` from `host/bareboot-tui` into `crates/bareboot-engine`.
- **Rationale**: Prevents duplicating business logic (compilation parsing, firmware artifact discovery, flashing session coordination, packet logging) between TUI and GUI. Both frontends communicate over the same `mpsc::Sender<WorkerCommand>` and `mpsc::Receiver<UiEvent>` channels. On WebAssembly, non-blocking channels map to browser event loops without blocking web workers.

### Decision 3: Profile Persistence via `craft-settings`
- **Choice**: Manage `FlashProfile` and `BarebootConfig` using `craft-settings` and `craft-atomic`.
- **Rationale**: Guarantees atomic file writing with directory `fsync` fallback on desktop/mobile, and delegates to browser `localStorage` on WebAssembly.

### Decision 4: Web Flashing via WebSerial API
- **Choice**: Implement `WebSerialBus` in `bareboot-client` using `web-sys` bindings to `navigator.serial`.
- **Rationale**:
  - Chromium browsers (Chrome, Edge, Opera, Chrome for Android) support WebSerial natively.
  - Allows users to plug in a USB-CDC bootloader (RP2350, STM32, CH32V203) or USB-to-UART adapter and flash directly from a static website with zero driver or software installation.
- **Alternatives Considered**:
  - *WebUSB*: Useful for raw vendor interfaces, but CDC-ACM serial drivers are typically claimed by the OS kernel, making WebSerial more universal for bootloaders enumerating as serial devices.

### Decision 5: Embedded In-App Documentation Viewer
- **Choice**: Compile project documentation (`docs/*.md`, `README.md`, protocol references) into the application using `include_str!` or a build script catalog, rendered via `egui_commonmark` or a styled `craft-ui` text layout.
- **Rationale**:
  - Eliminates context switching to an external browser during hardware bringup or field flashing.
  - Provides instant offline documentation access on field tablets and desktop benches with no network connection.
  - On the web target, it turns the web app into an interactive documentation site with live flashing capabilities built-in.

## Risks / Trade-offs

- **[Risk]** WebSerial browser support limitations (Safari and Firefox do not implement WebSerial).
  - **Mitigation**: Detect WebSerial support at runtime. If unsupported, display a clear technical banner explaining that WebSerial flashing requires a Chromium-based browser (Chrome, Edge), while keeping Target Configuration, Documentation, and Simulator views active.
- **[Risk]** Binary size overhead of embedding documentation and SVG assets.
  - **Mitigation**: Markdown documentation compresses to a few tens of kilobytes; total WASM binary size remains under 8 MB compressed.
- **[Risk]** Android touch UX with desktop-oriented docking panels.
  - **Mitigation**: Implement a responsive viewport check: on desktop (viewport width >= 900px), show multi-panel dockable layouts; on mobile viewports (< 900px), collapse into single-panel navigation tabs (Flasher, Config, Docs, Traffic).

## Migration Plan

1. Create `crates/bareboot-engine` and migrate worker/profile logic.
2. Update `host/bareboot-tui` to use `bareboot-engine` and verify all tests pass.
3. Add `host/bareboot-gui` and build the desktop GUI application.
4. Implement the embedded documentation viewer panel.
5. Add the Android target wrapper and `AndroidUsbBus` transport.
6. Add the WebAssembly target (`Trunk.toml`) and `WebSerialBus` transport.
