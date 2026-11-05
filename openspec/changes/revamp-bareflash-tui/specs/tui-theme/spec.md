## MODIFIED Requirements

### Requirement: Semantic color palette

The TUI SHALL define a central Palette derived from an accent tone ladder (`bg`, `surface`, `raised`, `border`, `text`, `text_muted`, `faint`, `accent`, `accent_dim`) with immutable semantic hardware state colors (`ok` green, `err` red, `warn` amber).

#### Scenario: Hardware state coloring

- **WHEN** an error occurs or connection is verified
- **THEN** `err` renders in distinct red and `ok` renders in distinct green across all active theme palettes

### Requirement: Consistent styling

All widgets and surfaces SHALL read colors exclusively from the active Palette without hardcoded color literals.

#### Scenario: Live theme switching

- **WHEN** the user presses `T` or selects a theme from the command palette
- **THEN** all widgets, surfaces, text, and borders update in real-time to the new theme palette

## ADDED Requirements

### Requirement: Curated theme collection

The TUI SHALL ship with curated named themes including Ember (default amber glow), Nord, Gruvbox, Tokyo Night, and Synthwave.

#### Scenario: Default launch palette

- **WHEN** the TUI starts without explicit configuration
- **THEN** the Ember amber theme is loaded as the active palette
