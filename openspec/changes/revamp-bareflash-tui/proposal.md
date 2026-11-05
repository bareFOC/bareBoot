## Why

The current terminal UI for `bareflash` (`bareboot-tui`) suffers from critical usability and architectural flaws: a vertical constraint bug squishes all central views into a 3-row strip leaving the screen mostly empty, raw keyboard shortcuts clash with text input in forms and wizards, and disconnected tabs isolate the flashing process from the real-time bus log. 

Revamping `bareflash` now aligns it with modern TUI design standards established by references like `nightlightd` and `aven`: introducing a resilient sidebar-driven cockpit, a derived tone-ladder theme system with Ember glow, isolated modal pickers, and real-time protocol packet decoding.

## What Changes

- **Sidebar-Driven View Navigation**: Replace disjointed 5-tab layout with a persistent sidebar switching between focused surfaces (Flash Studio, Protocol Bus Analyzer, Target Config, and Build & Probe Runner).
- **Tone-Ladder Theming Engine**: Replace static 16-color ANSI with mathematically derived palettes (`bg`, `surface`, `raised`, `border`, `text`, `muted`, `faint`) featuring default **Ember / Amber** glow, live `T`-key cycling (Nord, Gruvbox, Tokyo Night, Synthwave), and preserved `ok` (green) / `err` (red) hardware status signals.
- **Fixed Responsive Layout**: Eliminate the 3-row layout constraint bug; introduce centered max-width container (up to 120 cols) on ultrawide displays and compact mode on small terminals (<76 cols, <22 rows).
- **Global Command Palette (`:` / `Ctrl+P`)**: Searchable floating modal to trigger actions (flash, scan bus, switch target, run probe, toggle theme) without navigating away.
- **Modal Binary & Profile Picker**: Dedicated file browser overlay with cursor navigation (`j`/`k`, `Enter`, `Esc`), isolating text input from navigation shortcuts.
- **Real-Time Protocol Packet Decoder**: Parse and display CAN, I2C, and UART bootloader frames with timestamps, node IDs, DLC, hex dump, and decoded protocol opcodes (`PING`, `INFO`, `WRITE_CHUNK`, `VERIFY`, `ACK`, `NAK`).
- **Codebase Restructure**: Clean contaminated `ui.rs` and decouple monolithic `main.rs` into modular components (`theme`, `views`, `overlays`, `protocol`).

## Capabilities

### New Capabilities
- `tui-protocol-decoder`: Real-time packet parsing and human-readable decoding of bootloader frame transactions across CAN, I2C, and Serial buses.
- `tui-command-palette`: Quick-action search palette (`:` / `Ctrl+P`) for keyboard-first navigation and operation triggers.

### Modified Capabilities
- `tui-layout`: Replace the broken 3-line tab layout with an Aven-style sidebar view navigation, responsive viewport scaling, and breadcrumb header.
- `tui-theme`: Introduce Nightlightd-style derived tone ladder palettes, Ember default theme, and live cycling while guaranteeing semantic state colors.
- `tui-overlays`: Add dedicated modal binary/profile pickers, command palette, and help overlays over dimmed background surfaces.
- `tui-flash-wizard`: Transform the step wizard into a dedicated Flash Studio surface with braille progress bar, throughput (KB/s), block verification tracking, and safe modal parameter configuration.

## Impact

- **Affected Crate**: `bareflash/crates/bareboot-tui`.
- **Dependencies**: Uses `ratatui` (0.30) and `crossterm` (0.28). No new heavy external dependencies needed.
- **Compatibility**: Retains full compatibility with `bareboot-client`'s `HostBus` abstractions, session management, and `bareboot.toml` configurations.
