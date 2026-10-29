## Context

`main.rs` mixes event loop, layout, rendering, input. Split into `ui/` modules mirroring aven.

## Goals / Non-Goals

**Goals:** Modular render/input, testable layout helpers, smaller main.rs.
**Non-Goals:** New features; visual changes deferred to later phases.

## Decisions

### D1: Module layout
`ui/{mod, header, footer, nav, main_pane, context, overlays}`. Each exposes `render(f, app, area)`.

### D2: App state stays in main.rs
Modules borrow `&App` / `&mut App` for rendering and input.

## Risks / Trade-offs

- [Refactor risk] → keep build green after each module extraction.