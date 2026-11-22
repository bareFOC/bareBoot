# Tasks

## 1. Protocol Core & Identity Primitives

- [x] 1.1 Implement `SoftResetMailbox` struct and `fasthash64` in `crates/bareboot-protocol` and verify with unit tests (`cargo test -p bareboot-protocol`)
- [x] 1.2 Export constants `REQUEST_CANBOOT` (`0x5984_E3FA_6CA1_589B`) and `REQUEST_APP` (`0x4649_5348_4150_5031`) in `bareboot-protocol`

## 2. Target Implementation (`targets/stm32h503`)

- [x] 2.1 Update `i3c_dynamic_target.c` to parse `SoftResetMailbox` at `0x2000_7FE0` with CRC32 verification prior to reading Flash Sector 14/15
- [x] 2.2 Configure `I3C2->EPIDR` and `I3C2->DEVR0` / `I2C1->OAR1` using the unified address and silicon UID
- [x] 2.3 Implement deterministic fallback address calculation (`0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F)`) when flash and mailbox are blank
- [x] 2.4 Verify firmware builds cleanly and generate binary artifact (`artifacts/bareboot-stm32h503-i3c-i2c.bin`)

## 3. Host Verification & Test Suite

- [x] 3.1 Update host flashtool scripts in `host/scripts/` to support reading and sending Katapult UUID and dynamic address handoff
- [x] 3.2 Add automated unit test in `tests/` verifying mailbox CRC32 validation, address binding, and fallback calculation

## Workflow follow-up

- Sync delta specs to main specs (`openspec spec sync unified-uid-bus-address`).
- Archive the change after verification (`openspec archive unified-uid-bus-address`).
