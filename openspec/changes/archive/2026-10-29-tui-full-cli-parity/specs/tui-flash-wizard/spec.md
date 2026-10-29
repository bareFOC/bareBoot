## ADDED Requirements

### Requirement: Flash wizard sub-steps

The TUI SHALL provide a guided flash wizard under the Flash tab with ordered sub-steps: Backend selection, Device parameters, Firmware selection, Run options, and Run.

#### Scenario: Step navigation

- **WHEN** a user is on any wizard step
- **THEN** they can move forward with Enter/Next and backward with Back/Esc without losing entered values

### Requirement: Backend parity with CLI

The wizard SHALL allow selecting memory, serial, i2c, or can backends, with backend-specific parameters (baud, i2c address, can interface, node id) exactly matching CLI options.

#### Scenario: Serial backend selected

- **WHEN** the user selects serial
- **THEN** device path and baud fields are shown with default baud 115200

### Requirement: Run options

The wizard SHALL expose tries, timeout (seconds), and a verify toggle (default on), matching CLI defaults (tries 5, timeout 2s, verify on).

#### Scenario: Default options

- **WHEN** the wizard reaches Run options with no prior input
- **THEN** tries=5, timeout=2, verify=true are pre-filled

### Requirement: Firmware auto-discovery

The wizard SHALL auto-discover the most recent built firmware artifact under target/ and prefill the firmware path; manual override SHALL remain possible.

#### Scenario: Build present

- **WHEN** a firmware binary exists under target/
- **THEN** the firmware field is prefilled and editable

### Requirement: Flash run progress

During a run the TUI SHALL display a progress gauge and stream log lines without blocking input rendering.

#### Scenario: Long flash

- **WHEN** a flash operation runs
- **THEN** the UI continues to render progress and logs