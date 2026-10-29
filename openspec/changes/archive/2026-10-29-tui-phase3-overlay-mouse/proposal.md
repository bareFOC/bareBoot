## Why

Overlays (help/confirm/picker) exist but aren't mouse-usable and lack hit-testing, making them keyboard-only. Aven's overlay mouse handling shows the pattern.

## What Changes

- Add mouse hit-testing for overlays (close on outside click, scroll).
- Confirm dialog for destructive actions (delete profile).
- Profile picker overlay replacing the flat list selection.
- Scroll caps for long overlays.

## Capabilities

### New Capabilities
- `tui-overlay-mouse`: Mouse-usable overlays with hit-testing.

### Modified Capabilities
- `interactive-tui`: Overlay interaction expanded.

## Impact

- Affected: `bareboot-tui` overlay module.
- No new dependencies.