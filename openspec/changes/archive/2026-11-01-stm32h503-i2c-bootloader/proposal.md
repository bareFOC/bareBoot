## Why

The STM32H503KBU6 target on the Miniata board is wired to an external host or adapter via I2C (PB3 as SCL, PB4 as SDA). While UART was initially considered, hardware constraints and the physical setup with a CH341 USB-to-I2C adapter make I2C the required transport for bootloading and flashing firmware.

## What Changes

- Enable `transport-i2c` on `bareboot-stm32h503`.
- Configure `I2C2` peripheral on STM32H503 with PB3 (`I2C2_SCL`, AF8) and PB4 (`I2C2_SDA`, AF8) in open-drain mode with pull-ups.
- Implement I2C slave event (`I2C2_EV`) and error (`I2C2_ER`) interrupt handlers supporting Katapult framing:
  - Address match with DIR recognition (write/read).
  - Packet reception into `I2C_RX_BUF` on master write, triggered on STOP condition (`STOPF`).
  - Response transmission from `I2C_TX_BUF` on master read (`TXIS`).
- Configure target I2C address (default `0x40`).
- Update `bareboot-stm32h503.toml` board configuration to specify `transports = ["i2c"]` and target address `0x40`.
- Verify communication and flashing via CH341 adapter.

## Capabilities

### New Capabilities
- `stm32h503-i2c-bootloader`: Katapult-compatible I2C slave bootloader for STM32H503 (Miniata board), operating on I2C2 (PB3=SCL, PB4=SDA) at address 0x40, supporting firmware flash programming and application handoff.

### Modified Capabilities

(none)

## Impact

- Affected code: `bareboot/targets/bareboot-stm32h503/src/main.rs`, `bareboot/targets/bareboot-stm32h503/Cargo.toml`, `bareboot/targets/bareboot-stm32h503/build.rs`, `bareboot-stm32h503.toml`.
- Dependencies: `stm32-metapac` (feature `stm32h503kb`), `bareboot-core`, `bareboot-protocol`.
- Hardware: STM32H503KBU6 connected to CH341 USB-to-I2C adapter (PB3=SCL, PB4=SDA, GND).
