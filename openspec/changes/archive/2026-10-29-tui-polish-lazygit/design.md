## Context

The TUI is functionally complete (parity with CLI) but visually flat: single color scheme, full-width panes, raw text buffers, static help line. Users want a lazygit-style experience: three panes, semantic colors, structured forms, single-overlay popups, context help.

## Goals / Non-Goals

**Goals:**
- Three-pane layout (nav/main/context) with responsive sizing.
- Semantic theme palette applied consistently.
- Structured forms for config and flash wizard.
- Single-overlay popup system.
- Top status bar + context-sensitive help footer.
- Semantic log coloring.

**Non-Goals:**
- Drag-to-resize panes.
- User-configurable themes (hardcoded palette with hook).
- Stacked overlays.
- Full mouse-driven form editing beyond buttons/lists.

## Decisions

### D1: Three-pane layout with fixed ratios
Nav (20%), main (55%), context (25%) on wide terminals; collapse context pane below 80 cols.

### D2: Overlay as enum state
`Overlay::None | Help | Confirm { msg, on_yes } | ProfilePicker`. Single active overlay; Esc closes. Rendered last (on top).

### D3: Form field abstraction
A `FormField` enum (Text, Select, Toggle, Number) with focus index and validation. Forms render labels + inputs + inline errors.

### D4: Semantic styles via Theme struct
`Theme` holds `Color` values; all widgets read from it. Easy to tweak palette in one place.

### D5: Status bar derives from App state
Top line shows connection status, active profile, flash step; bottom line shows context help based on tab + overlay.

## Risks / Trade-offs

- [Layout complexity] → start with fixed ratios, add responsive breakpoints later.
- [Form state management] → keep a single `Form` struct with field list and focus index.
- [Overlay input routing] → overlay captures keys when active; pass-through otherwise.