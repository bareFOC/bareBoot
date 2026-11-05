## MODIFIED Requirements

### Requirement: Single overlay system

The TUI SHALL support a single active modal overlay (help, confirmation, command palette, binary picker) rendered on an elevated surface (`raised`) with a dimmed background backdrop.

#### Scenario: Open help overlay

- **WHEN** the user presses `?`
- **THEN** a centered modal keybinding reference opens with elevated background, and `Esc` or `?` dismisses it

#### Scenario: Overlay input isolation

- **WHEN** a modal overlay is active
- **THEN** all keystrokes are routed exclusively to the overlay and background navigation hotkeys are disabled

## ADDED Requirements

### Requirement: Modal binary file picker

The TUI SHALL provide a modal file browser to navigate directory paths, inspect file sizes, and select firmware binaries (`.bin`, `.elf`, `.hex`) using `j`/`k`, `Enter`, and `Esc`.

#### Scenario: Firmware file selection

- **WHEN** user triggers the binary picker and presses Enter on a `.bin` file
- **THEN** the firmware path updates in the active flash profile and the modal closes

### Requirement: Modal profile manager

The TUI SHALL support creating, switching, and deleting target profiles through a dedicated picker overlay without key collisions.

#### Scenario: Switch profile

- **WHEN** user opens profile picker and selects a profile
- **THEN** profile settings load into active memory and toast confirmation is displayed
