## 1. Target Configuration & Peripheral Setup

- [x] 1.1 Enable `transport-i2c` in `bareboot/targets/bareboot-stm32h503/Cargo.toml` and update `bareboot-stm32h503.toml`
- [x] 1.2 Implement `i2c_init()` for `I2C2` on PB3 (SCL, AF8) and PB4 (SDA, AF8) in open-drain mode with pull-ups and 100 kHz timing

## 2. I2C Event & Error Interrupt Handlers

- [x] 2.1 Implement `I2C2_EV` interrupt service routine for ADDR match, RXNE reception, TXIS transmission, and STOP condition detection
- [x] 2.2 Implement `I2C2_ER` interrupt service routine to clear bus errors
- [x] 2.3 Implement main loop integration: dispatch Katapult frames upon `I2C_STOP_RECEIVED` and stage response in `I2C_TX_BUF`

## 3. Verification & Flashing

- [x] 3.1 Build and flash `bareboot-stm32h503` with I2C transport to the target MCU via SWD
- [x] 3.2 Verify I2C slave address detection (0x40) via CH341 adapter
- [x] 3.3 Test Katapult CONNECT handshake and firmware flashing over I2C
