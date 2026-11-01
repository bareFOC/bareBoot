## Context

The STM32H503KBU6 target on the Miniata board connects to the host machine via a CH341 USB-to-I2C adapter. The pins designated for the I2C transport are PB3 (`I2C2_SCL`) and PB4 (`I2C2_SDA`). STM32H503 uses the ST I2C v2 IP core (`0x4000_5800`), which supports slave address match, automatic clock stretching, and interrupt-driven TX/RX buffering.

## Goals / Non-Goals

**Goals:**
- Provide reliable Katapult-compatible packet exchange over I2C slave interface on I2C2.
- Configure PB3 as SCL (AF8) and PB4 as SDA (AF8) in open-drain mode with internal pull-ups.
- Handle master write transactions into `I2C_RX_BUF` and master read requests from `I2C_TX_BUF`.
- Integrate with existing `H503Flash` driver for 8 KB sector erase and 16-byte flash programming.
- Maintain seamless jump to application at `0x08004000`.

**Non-Goals:**
- Supporting multi-master arbitration or SMBus/PMBus extensions.
- Dynamic runtime change of I2C address (fixed to `0x40`).

## Decisions

### 1. Peripheral Selection & Pin Mapping
- **Choice**: I2C2 with PB3 (AF8) and PB4 (AF8).
- **Rationale**: STM32H503 has I2C1 and I2C2. I2C2 maps SCL to PB3 (AF8) and SDA to PB4 (AF8). This matches the user's physical wiring.
- **Alternatives**: I3C2 also maps to PB3/PB4 (AF3), but the host adapter (CH341) is standard I2C, so I2C2 in I2C mode is appropriate.

### 2. Clocking and Timing
- **Choice**: Run I2C2 from PCLK1 (64 MHz) with standard 100 kHz timing (`TIMINGR = 0x30424FC7`).
- **Rationale**: 100 kHz standard mode ensures compatibility with CH341 adapter and long jumper cables.

### 3. Katapult Framing over I2C
- **Choice**: Frame reception triggers on I2C `STOPF` flag.
- **Rationale**: Master writes Katapult frame in one I2C write transaction, ending with a STOP condition. The bootloader dispatches the command and stages the response in `I2C_TX_BUF`. When the host performs an I2C read transaction, the bootloader supplies the response bytes via `TXIS`.

## Risks / Trade-offs

- **[Risk] External pull-ups missing on bus** → Mitigation: Enable internal pull-ups on PB3/PB4 (`PUPDR = 0x1` for both), though external 2.2k-4.7kΩ pull-ups are strongly recommended.
- **[Risk] Flash erase stalling CPU** → Mitigation: I2C hardware clock stretching is enabled by default (`NOSTRETCH = 0` in CR1), ensuring the master waits while flash write/erase completes.
