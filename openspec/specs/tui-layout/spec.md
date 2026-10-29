# tui-layout Specification

## Purpose
TBD - created by archiving change tui-polish-lazygit. Update Purpose after archive.
## Requirements
### Requirement: Three-pane layout

The TUI SHALL render nav (left), main (center), and context (right) panes on wide terminals.

#### Scenario: Wide terminal

- **WHEN** terminal width >= 80 columns
- **THEN** three panes are shown with ratios 20/55/25

### Requirement: Responsive collapse

On narrow terminals the context pane SHALL collapse.

#### Scenario: Narrow terminal

- **WHEN** terminal width < 80 columns
- **THEN** only nav and main panes are shown

