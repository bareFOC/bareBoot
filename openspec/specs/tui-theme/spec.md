# tui-theme Specification

## Purpose
TBD - created by archiving change tui-polish-lazygit. Update Purpose after archive.
## Requirements
### Requirement: Semantic color palette

The TUI SHALL define a central Theme with semantic colors (error, success, warning, info, active, muted, accent) used by all widgets.

#### Scenario: Error styling

- **WHEN** an error log line is rendered
- **THEN** it uses the error color

### Requirement: Consistent styling

All widgets SHALL read colors from the Theme rather than hardcoded values.

#### Scenario: Theme change

- **WHEN** the palette is adjusted
- **THEN** all widgets reflect the new colors without code changes elsewhere

