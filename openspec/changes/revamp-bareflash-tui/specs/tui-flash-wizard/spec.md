## MODIFIED Requirements

### Requirement: Flash wizard sub-steps

The TUI SHALL provide a unified Flash Studio view presenting target parameters, firmware metadata, backend options, and execution triggers on a single glanceable surface with modal parameter editing.

#### Scenario: Flash studio parameter editing

- **WHEN** user selects backend or options to adjust
- **THEN** parameters can be edited via dedicated modal prompts without colliding with navigation keys

### Requirement: Flash run progress

During a flash run, the Flash Studio SHALL render a high-resolution progress gauge, real-time transfer throughput (KB/s), block verification count, elapsed time, and ETA.

#### Scenario: Real-time telemetry during flash

- **WHEN** flashing firmware to a node
- **THEN** progress percentage, transfer rate in KB/s, and verified block count update continuously on the Flash Studio card
