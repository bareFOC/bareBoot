## Why

`main.rs` is a 300+ line monolith mixing event loop, layout, rendering, and input handling. This blocks reuse and testing. Aven's proven `ui/` module split (header/footer/sidebar/detail/overlays) is the model.

## What Changes

- Split `main.rs` render/input into `ui/{header,footer,nav,main_pane,context,overlays}` modules.
- Move `Theme`, `Form`, `Overlay` into their modules.
- Keep event loop in `main.rs` only.

## Capabilities

### New Capabilities
- `tui-ui-modules`: Modular UI rendering with testable layout helpers.

### Modified Capabilities
- `interactive-tui`: Rendering split into modules.

## Impact

- Affected: `bareboot-tui` crate (main.rs shrinks; new ui/ dir).
- No new dependencies.