## Context

The `bareflash` TUI cockpit (`bareboot-tui`) was previously constructed with a 5-tab system that suffered from severe vertical layout squishing (rendering the 3-column body into a 3-row layout), key collision between text input and navigation hotkeys, and rigid uninspired color palettes.

This design document outlines the architectural transformation of `bareboot-tui` into a modern, responsive, and robust hardware terminal interface inspired by `ref/nightlightd` (tone-ladder palettes, responsive centering, and constant status signals) and `ref/aven` (sidebar-driven views, breadcrumb header, modal overlays, and command palette).

## Goals / Non-Goals

**Goals:**
- Provide an Aven-style sidebar navigation driving 4 dedicated views: Flash Studio, Protocol Bus Analyzer, Target Config, and Build & Probe Runner.
- Fix all terminal layout bugs, correctly utilizing 100% of available vertical height while enforcing a 120-column max-width cap for ultrawide displays.
- Implement Nightlightd's derived tone-ladder theming system with default **Ember / Amber** glow, live cycling (`T`) through Nord, Gruvbox, Tokyo Night, and Synthwave, and immutable green (`ok`) and red (`err`) status indicators.
- Introduce modal input isolation (file browser and profile picker) to prevent typing from misfiring global hotkeys.
- Add a global command palette (`:` / `Ctrl+P`) for keyboard-driven discoverability.
- Add real-time bootloader packet decoding for CAN, I2C, and UART transactions.
- Clean up the contaminated `ui.rs` and modularize `bareboot-tui` into clean, testable submodules.

**Non-Goals:**
- Rewriting underlying low-level transport drivers in `bareboot-client` (SocketCAN, Serial, I2C logic remains unchanged).
- Modifying micro-controller firmware bootloader images or protocol framing wire formats.
- Adding heavy web-based UI or non-terminal graphical frontends.

## Decisions

### 1. View Model: Sidebar Navigation vs. Unified Megascreen vs. Tabs
- **Decision**: Adopt a persistent sidebar view switcher (Aven style) that toggles between 4 primary workspaces:
  1. *Flash Studio*: Target details, binary inspector, braille/solid progress gauge, KB/s telemetry, verify metrics.
  2. *Protocol Bus Analyzer*: Live packet feed with timestamp, node, DLC, hex, and decoded opcodes.
  3. *Target Config*: `bareboot.toml` editor and parameter validation.
  4. *Build & Probe Runner*: Cargo build streaming & probe-rs listing.
- **Alternatives Considered**: 
  - *Unified megascreen*: Cramming all 4 features into one screen created excessive cognitive load and required >= 160 columns to be legible.
  - *Status-quo tabs (1-5)*: Tabs broke context and lacked visual hierarchy.
- **Rationale**: The sidebar preserves spatial orientation and allows each primary task to command full screen real estate while remaining 1 keystroke away (`1-4` or `Tab`).

### 2. Theming: Mathematical Tone-Ladder System
- **Decision**: Implement a `Palette` struct where all UI surfaces are tinted relative to the active theme base:
  - `bg`: Dark tinted canvas base (not harsh black).
  - `surface`: Panel and card background (one step higher contrast).
  - `raised`: Dialogs, command palette, and modal overlays.
  - `accent` / `accent_dim`: Brand emphasis (Ember amber `#FF9E3B`).
  - `ok` (`#2ECC71`) and `err` (`#E74C3C`): Fixed semantic status signals that never adapt to theme hue, preventing safety-critical hardware indicators from being lost.
- **Alternatives Considered**: Standard Ratatui 16 ANSI colors. Rejected because it produces an amateur, visually fragmented look without elevation or depth.

### 3. Input Routing: Dedicated Overlay State Machine
- **Decision**: Centralize modal overlays into a dedicated enum:
  ```rust
  pub enum ActiveOverlay {
      None,
      CommandPalette(CommandPaletteState),
      FilePicker(FilePickerState),
      ProfilePicker(ProfilePickerState),
      Help,
      Confirm { title: String, message: String, action: PendingAction },
  }
  ```
  When an overlay is active, the event dispatcher routes all raw keystrokes strictly to the overlay's handler. Global hotkeys (`q`, `s`, `1-4`, `Space`) are completely silenced until the overlay is closed via `Esc` or `Enter`.
- **Alternatives Considered**: Inline text boxes inside the main render loop. Rejected because inline input invariably leads to key-interception bugs.

### 4. Modular Code Organization
- **Decision**: Refactor `bareboot-tui/src/` into a clean modular hierarchy:
  - `app.rs`: Application model, view state, active overlay, worker event polling.
  - `theme.rs`: Palette definitions, tone ladder computation, theme cycler.
  - `protocol.rs`: Packet parser decoding bootloader commands (PING, WRITE_CHUNK, VERIFY, ACK, NAK).
  - `ui/`:
    - `header.rs`: Top status breadcrumb bar.
    - `sidebar.rs`: View tabs, profile list, probe chip.
    - `footer.rs`: Context-sensitive hotkey hints.
    - `views/`: `flash_studio.rs`, `bus_analyzer.rs`, `config_editor.rs`, `build_probe.rs`.
    - `overlays/`: `command_palette.rs`, `file_picker.rs`, `help.rs`, `confirm.rs`.

## Risks / Trade-offs

- **[Risk] High-speed CAN traffic (1 Mbps) overwhelming the TUI rendering thread**  
  → *Mitigation*: The worker thread buffers packets into an internal ring buffer (capped at 500 frames) and drains up to 30 frames per render tick (100ms) with a pause toggle (`Space`).
- **[Risk] Small terminal viewports (<80x24)**  
  → *Mitigation*: Implement responsive fallback in `ui/mod.rs` to collapse the right inspector when width < 90, and render a compact status-first view when height < 22 rows.
- **[Risk] Incomplete ANSI color support on older terminals**  
  → *Mitigation*: The `Palette` defaults to standard 24-bit TrueColor RGB, but degrades gracefully via crossterm's backend mapping.
