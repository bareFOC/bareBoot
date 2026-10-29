## Why

The TUI reached functional parity with the CLI, but it remains visually flat and interaction-shallow: raw text buffers instead of forms, monochrome logs, no context panes, and a help line that doesn't reflect the current tab. Borrowing lazygit's three-pane layout, semantic colors, single-overlay popups, and context-sensitive help will make it feel like a real cockpit while keeping the keyboard+mouse hybrid workflow.

## What Changes

- Introduce a semantic theme palette (error red, success green, active yellow, muted gray, accent cyan) applied consistently.
- Replace the single full-width pane with a three-pane layout (nav / main / context) with responsive sizing.
- Add structured forms for config editing and flash wizard fields, replacing raw text buffers.
- Add single-overlay popups: help, confirm-delete, profile picker; Esc closes.
- Add a top status bar (connection/profile/flash-step) and a context-sensitive help footer.
- Add semantic coloring to logs (ERROR/DONE/progress) and detail panes for selected node/profile.

## Capabilities

### New Capabilities
- `tui-theme`: Semantic color palette and styling helpers.
- `tui-layout`: Three-pane responsive layout with mouse hit-testing.
- `tui-forms`: Structured field editors for config and flash wizard.
- `tui-overlays`: Single-overlay popup system (help, confirm, picker).
- `tui-status-bar`: Top status line + context-sensitive help footer.

### Modified Capabilities
- `interactive-tui`: Layout, theming, and overlay requirements added.

## Impact

- Affected crate: `bareboot-tui` (main.rs, new theme/layout/form/overlay modules).
- New modules: `theme`, `layout`, `form`, `overlay`.
- Dependencies: no new crates expected (ratatui already provides widgets).
- Risk: larger render code; keep rendering on UI thread.