## ADDED Requirements

### Requirement: Single overlay system

The TUI SHALL support a single active overlay (help, confirm, picker) rendered on top of content.

#### Scenario: Open help

- **WHEN** the user presses ?
- **THEN** the help overlay appears and Esc closes it

### Requirement: Confirm dialogs

Destructive actions SHALL show a confirm overlay before executing.

#### Scenario: Delete profile

- **WHEN** a user requests profile deletion
- **THEN** a confirm overlay appears; confirming executes, Esc cancels