# BareBoot Graphical Workbench (bareboot-gui)

The BareBoot Graphical Workbench (`bareboot-gui`) is a cross-platform desktop, mobile, and web application designed for configuring, compiling, and flashing microcontrollers running the BareBoot / Katapult bootloader protocol.

---

## Architecture

The workbench is built on a 100% Rust architecture:

```
+----------------------------------------------------------------------------+
|                             bareboot-gui                                   |
|   eframe / egui / craft-ui (DockArea, Tabs) / craft-settings (Atomic TOML) |
+------------------------------------+---------------------------------------+
                                     |
               +---------------------+---------------------+
               |                                           |
               v                                           v
+-----------------------------+             +-------------------------------+
|       bareboot-engine       |             |        bareboot-client        |
|  - WorkerHandles / Channels |             |  - HostBus Abstraction        |
|  - Makeflash Orchestration  |             |  - SerialBus, SocketCanBus    |
|  - Cargo Build Streaming    |             |  - I2cBus, InMemoryBus        |
|  - probe-rs Runners         |             |  - AndroidUsbBus (RawFd)      |
|  - Profile Persistence      |             |  - WebSerialBus (web-sys)     |
+-----------------------------+             +-------------------------------+
               |                                           |
               +---------------------+---------------------+
                                     |
                                     v
                        +-------------------------+
                        |    bareboot-protocol    |
                        | Katapult Framing, CRC32 |
                        +-------------------------+
```

### Key Components

1. **`bareboot-engine` (`crates/bareboot-engine`)**:
   Headless background execution service extracted from the TUI. Owns background thread orchestration (`WorkerHandles`), non-blocking command and event channels (`WorkerCommand`, `UiEvent`), atomic profile loading/saving (`craft-atomic`), target compilation (`stream_cargo_build`, `stream_makeflash`), and SWD/JTAG operations (`probe-rs`).

2. **`craft-ui` Docking Layout**:
   On desktop viewports (width >= 900px), panels are organized using `craft_ui::docking::DockArea`:
   - **Left**: Target Configuration & Profile Inspector.
   - **Center-Top**: Katapult Makeflash Pipeline & Progress Gauge.
   - **Center-Bottom**: Cargo Build & Diagnostics Console.
   - **Right**: Live Bus Traffic Sniffer & Packet Analyzer (tabbed with In-App Documentation).

3. **Mobile Responsive Viewport**:
   When viewport width is under 900px (Android tablets, phones, narrow windows), the dock collapses into a streamlined four-tab interface:
   - `Flasher`: Makeflash controls, progress gauge, and build console.
   - `Config`: Visual target parameters and profile editor.
   - `Manuals`: Offline documentation and pinout reference viewer.
   - `Traffic`: Real-time packet capture table and active bus nodes.

4. **In-App Documentation & Manual Viewer**:
   Bundles all architecture guides, handsfree update specifications, protocol opcode definitions, and hardware pinout tables directly within the binary for offline access in field environments.

---

## Desktop Workflows

### 1. Target Configuration & Profile Management
- Select pre-configured presets (e.g. `STM32F103-CAN`, `STM32F103-UART`, `STM32G0B1-CAN`, `STM32H503-I2C`, `CH32V203-USB`).
- Edit MCU family, backend transport, baud rates, CAN arbitration IDs, pin assignments, and boot window timings.
- Boot window timings are strictly validated to prevent setting intervals shorter than 50 ms.
- Profiles and target configurations are stored atomically to TOML using `craft-atomic` to prevent corruption during unexpected shutdowns.

### 2. Desktop Katapult Makeflash Pipeline
- **Makeflash (Build & Flash)**: Single-click workflow that invokes `cargo build` for the selected embedded target, automatically locates the newest binary in target directories (`thumbv7m-none-eabi`, `riscv32imac-unknown-none-elf`, etc.), and flashes the microcontroller over SWD/JTAG (via `probe-rs`) or over Katapult (via CAN, Serial, or Memory Simulation).
- **Cargo Build Only**: Compiles firmware and streams build diagnostics into the integrated console with warning and error highlighting.
- **Flash Only**: Directly programs the selected firmware binary without rebuilding.
- **Reset Target**: Issues a hardware reset command over probe-rs.
- **Scan Bus**: Broadcasts discovery pings across CAN and I2C interfaces to list attached bootloaders and application nodes.

### 3. Live Bus Traffic Sniffer
- Displays real-time Katapult frames with timestamp, direction (`[TX]` / `[RX]`), bus type (`CAN`, `SERIAL`, `I2C`, `MEM`), opcode name (`CONNECT`, `RX_BLOCK`, `RX_EOF`, `REQ_BLOCK`, `COMPLETE`, `ACK`, `NACK`), data length (DLC), and hex payload bytes.
- Supports search and filter by opcode or node address.

---

## Android USB Host (OTG) Field Flashing

On Android mobile devices, the workbench operates as a field flasher without requiring root operating system privileges:

- **Transport (`AndroidUsbBus`)**: Communicates directly over native USB file descriptors (`RawFd`) via POSIX read/write bulk endpoint transfers.
- **Permission Flow**: Android's `UsbManager` and `UsbDeviceConnection` handle the user permission prompt. The JNI bridge (`Java_org_bareboot_gui_UsbBridge_setNativeFd`) passes the granted file descriptor into Rust.
- **Supported Hardware**: Native USB-CDC serial bootloaders (RP2350, STM32, CH32V203) and USB-CAN adapters (CandleLight / GS_USB, SLCAN).
- **Packaging**: Configured in `host/bareboot-gui/android/AndroidManifest.xml` with `android.hardware.usb.host` required permissions and device intent filters.

---

## WebAssembly & GitHub Pages Deployment

The workbench compiles to WebAssembly (`wasm32-unknown-unknown`) and runs directly in modern web browsers via `eframe::WebRunner`:

### WebSerial Flashing
- In Chromium-based browsers (Google Chrome, Microsoft Edge, Opera), users can connect directly to USB serial bootloaders without installing native software.
- The `WebSerialBus` transport exchanges Katapult frames over the browser's `navigator.serial` streams.
- Secure Context requirement: WebSerial requires HTTPS (or `localhost`). Hosting on GitHub Pages provides HTTPS out-of-the-box.

### Drag-and-Drop Firmware Upload
- Users can drag and drop compiled `.bin` or `.uf2` binaries directly onto the browser canvas.
- The file payload is read into memory and staged immediately for flashing.

### GitHub Pages Setup
- Configured in `host/bareboot-gui/Trunk.toml`:
  ```toml
  [build]
  target = "index.html"
  dist = "dist"
  public_url = "./"
  ```
  Setting `public_url = "./"` ensures all generated scripts and WebAssembly assets use relative paths, functioning correctly under repository subpaths such as `https://<owner>.github.io/<repo>/`.

### Automated GitHub Actions CI/CD
The repository includes `.github/workflows/deploy-pages.yml` which automatically:
1. Sets up the Rust toolchain with the `wasm32-unknown-unknown` target.
2. Installs Trunk.
3. Builds the WebAssembly application: `trunk build --release`.
4. Uploads and deploys the `dist/` directory to GitHub Pages over HTTPS.

---

## Building and Running

### Desktop (Linux, macOS, Windows)
```bash
# Check compilation
cargo check -p bareboot-gui

# Run desktop workbench
cargo run -p bareboot-gui

# Run all workspace tests
cargo test --workspace
```

### WebAssembly (Trunk)
```bash
# Install trunk if not already installed
cargo install trunk

# Add WebAssembly target
rustup target add wasm32-unknown-unknown

# Serve locally for development
cd host/bareboot-gui
trunk serve

# Build release artifacts for hosting
trunk build --release
```

---

## Project Standards & Zero Emoji Policy

In strict accordance with project rules, all user interface elements, console logs, error messages, code comments, tests, and documentation remain completely emoji-free. All statuses are conveyed through standard uppercase technical labels (e.g. `[READY]`, `[BUSY]`, `[DONE]`, `[ERROR]`, `[TX]`, `[RX]`, `[CAN]`, `[SERIAL]`).
