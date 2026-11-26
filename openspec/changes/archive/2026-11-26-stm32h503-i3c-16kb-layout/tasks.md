# Tasks: `stm32h503-i3c-16kb-layout`

- [x] 1. Update `targets/stm32h503-i3c/memory.x` (`LENGTH = 16K`) and `targets/stm32h503-i3c/src/main.rs` (`APP_BASE = 0x0800_4000`, WRP bits `[1:0] = 00`).
- [x] 2. Update `host/scripts/flash_app_over_i3c_usb.py` and unit test defaults if needed (`0x08004000`), build `bareboot-stm32h503-i3c`, and update `artifacts/bareboot-stm32h503-i3c-rust.bin`.
- [x] 3. Run `cargo test -p bareboot-core -p bareboot-protocol` and `python3 -m unittest host/scripts/test_flash_app_over_i3c_usb.py`.
- [x] 4. Sync specs, archive `stm32h503-i3c-16kb-layout`, and commit.
