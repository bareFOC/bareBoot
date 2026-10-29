# tui-detail-pane Specification

## Purpose
TBD - created by archiving change tui-phase2-detail-pane. Update Purpose after archive.
## Requirements
### Requirement: Detail pane

The TUI SHALL show a scrollable detail pane for the selected node/profile.

#### Scenario: Selected node

- **WHEN** a node is selected
- **THEN** its details appear in the context pane

### Requirement: Empty states

Panels SHALL show an empty-state message when no data.

#### Scenario: No nodes

- **WHEN** no nodes detected
- **THEN** "No nodes detected" is shown

