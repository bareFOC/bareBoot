## ADDED Requirements

### Requirement: Searchable command palette

The TUI SHALL provide a global command palette triggered by `:` or `Ctrl+P` allowing fuzzy filtering and instant execution of all application actions.

#### Scenario: Open command palette

- **WHEN** user presses `:` or `Ctrl+P`
- **THEN** a centered command palette modal appears with a text prompt and filtered command catalog

#### Scenario: Execute action from palette

- **WHEN** user types "flash" and presses Enter on "Run Flash"
- **THEN** the command executes immediately and the palette dismisses

### Requirement: Command catalog discoverability

The command palette catalog SHALL list all core actions, including flashing, bus scanning, profile switching, firmware file browsing, theme changing, and probe status checks, along with their keyboard shortcuts.

#### Scenario: Palette shortcut hints

- **WHEN** the command palette is visible
- **THEN** each action row displays its human-readable title, category, and direct hotkey if assigned
