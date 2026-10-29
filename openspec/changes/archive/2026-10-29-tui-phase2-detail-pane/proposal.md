## Why

The Bus/Profiles tabs lack a detail pane and empty states, so users can't inspect a selected node or profile. Aven's `detail` + `empty_state` modules solve this.

## What Changes

- Add a scrollable detail pane (right column) showing selected node/profile metadata.
- Add empty-state placeholders ("No nodes detected", "No profiles").
- Scroll caps and keyboard scroll (j/k, PgUp/PgDn).

## Capabilities

### New Capabilities
- `tui-detail-pane`: Scrollable detail pane with empty states.

### Modified Capabilities
- `interactive-tui`: Detail pane added to layout.

## Impact

- Affected: `bareboot-tui` ui modules.
- No new dependencies.