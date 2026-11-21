# Tasks

## 1. Core Reliability & Commit Journaling

- [x] 1.1 Implement monotonic bit-flip commit state tracker in `crates/bareboot-core` and verify with host unit tests in `cargo test -p bareboot-core`.
- [x] 1.2 Implement `WatchdogFlash` wrapper and watchdog servicing hook inside `PagedWriter` in `crates/bareboot-core/src/writer.rs` and verify unit tests.
- [x] 1.3 Implement strict application partition bounds checking in `PagedWriter` (`[APP_BASE, APP_END)`) to prevent writes into NVM sectors, and verify tests.
- [x] 1.4 Create an adapter trait bridging `embedded-storage::nor_flash::NorFlash` into `bareboot-core::FlashTarget` and verify compilation across target crates.

## 2. Processor Handoff & Architecture Safety

- [x] 2.1 Add instruction cache invalidation (`SCB.invalidate_icache()`) and VTOR update to Cortex-M boot jump in `targets/rp2350-usb/src/main.rs` and verify execution.
- [x] 2.2 Implement peripheral cleandown routine to disable active communication interrupts and pull-ups prior to application handoff and verify on target.

## 3. I2C and MIPI I3C Target Transport

- [x] 3.1 Formalize I2C and I3C CCC definitions (`RSTACT`, `ENTDAA`, `SETDASA`) and entry byte contracts in `crates/bareboot-protocol` and verify tests.
- [x] 3.2 Implement non-blocking I2C target command reception and response staging buffer (eliminating SCL clock stretching) and verify packet round-trip.
- [x] 3.3 Integrate dynamic I2C/I3C bus switching on shared pins with `RSTACT` target reset handling in target firmware and verify compilation.

## 4. Hardware Verification on FishyServo via AgentLA

- [x] 4.1 Flash `bareboot-stm32h503-i3c-i2c.bin` onto FishyServo (`0x0800_0000`) and verify I2C ACK on address `0x40` during the boot window.
- [x] 4.2 Perform handsfree OTA flash of `n20-servo.bin` over I2C using `flash_app_over_i2c.py` triggered by `0xF0` strobe, and verify successful boot.
- [x] 4.3 Verify NVM persistence: write calibration values into `H5FlashStorage` (`0x0801_C000`), perform an OTA update, and assert calibration values remain unchanged.
- [x] 4.4 Verify dynamic MIPI I3C switching and `RSTACT` target reset using `flash_app_over_i3c.py` and verify successful transfer.
- [x] 4.5 Perform negative failure test: abort update mid-stream, power cycle, and verify target remains trapped in BareBoot recovery mode.
