# Tasks: `bareboot-hal-platform-traits`

- [x] 1. Add `FlashRegion`, `ArchBoot`, `TickTimer`, `BusTransport`, `BootPlatform`, `NoopWatchdog`, and generic `verify_app_crc32` + `BootRunner` in `crates/bareboot-core`.
- [x] 2. Generalize `validate_app_vector` to accept architecture-specific `is_valid_pc` (fixing ARM Thumb assumption for RISC-V) and parameterize `SoftResetMailbox` address via `BootPlatform`.
- [x] 3. Add `crates/bareboot-arch-cortexm` (`CortexMArch` implementing `ArchBoot` with `SysTick` disable, `VTOR`, `dsb`/`isb`, `MSP`+entry jump, and `SCB_AIRCR` reset).
- [x] 4. Migrate `targets/stm32h503-i3c` to `BootPlatform` + `BootRunner` + `CortexMArch` and verify `.bin <= 16,384` bytes.
- [x] 5. Add host unit tests for `BootRunner` (mock `BootPlatform`), run `cargo test -p bareboot-core -p bareboot-protocol`, sync specs, and archive.
