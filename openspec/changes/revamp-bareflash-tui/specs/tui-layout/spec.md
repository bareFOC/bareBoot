## MODIFIED Requirements

### Requirement: Three-pane layout

The TUI SHALL render an Aven-style sidebar (left), active main workspace surface (center), and contextual inspector (right) with full terminal vertical height allocation.

#### Scenario: Wide terminal

- **WHEN** terminal width >= 100 columns and height >= 24 rows
- **THEN** sidebar (left ~22 cols), main workspace surface (center flex), and inspector (right ~28 cols) are rendered across full available body height

### Requirement: Responsive collapse

On narrower terminals or compact viewports, the layout SHALL adapt gracefully by collapsing secondary panes and capping maximum width on ultrawide displays.

#### Scenario: Narrow terminal

- **WHEN** terminal width < 90 columns
- **THEN** the right inspector pane collapses and the main surface expands to fill remaining space

#### Scenario: Ultrawide display

- **WHEN** terminal width > 120 columns
- **THEN** the cockpit width is capped at 120 columns and horizontally centered with balanced background margins

#### Scenario: Compact height terminal

- **WHEN** terminal height < 22 rows or width < 76 columns
- **THEN** a condensed high-density layout renders essential flashing status without overflow

## ADDED Requirements

### Requirement: Breadcrumb header bar

The TUI SHALL render an interactive top header with product wordmark, version, active target chip, bus transport state, node ID, and live connection status indicator.

#### Scenario: Connected bus state

- **WHEN** a bus interface is active and node is detected
- **THEN** header displays target name, transport type, bitrate, and a highlighted [ONLINE] chip

### Requirement: Sidebar view switcher

The sidebar SHALL provide primary navigation switching between four dedicated surfaces: Flash Studio, Protocol Bus Analyzer, Target Config, and Build & Probe Runner.

#### Scenario: Switch view via keyboard

- **WHEN** the user presses keys 1-4 or Tab in navigation mode
- **THEN** the active surface switches instantly to the selected view
