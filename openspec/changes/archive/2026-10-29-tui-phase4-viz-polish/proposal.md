## Why

The TUI lacks visual richness: no sparklines for throughput/progress, no semantic log coloring beyond basic, no braille bars. nightlightd's `ratatui-braille-bar` and aven's semantic styling show the target.

## What Changes

- Add `ratatui-braille-bar` sparklines for progress/throughput.
- Semantic log coloring (already partial) completed across all tabs.
- Polish borders, spacing, empty states.

## Capabilities

### New Capabilities
- `tui-viz-polish`: Sparklines and semantic visual polish.

### Modified Capabilities
- `interactive-tui`: Visual polish applied.

## Impact

- Affected: `bareboot-tui` (new dependency `ratatui-braille-bar`).
- Cargo.toml update required.