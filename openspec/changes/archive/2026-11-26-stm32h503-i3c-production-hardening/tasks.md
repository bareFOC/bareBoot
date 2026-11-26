# Tasks: `stm32h503-i3c-production-hardening`

- [x] 1. Update `targets/stm32h503-i3c/src/main.rs` (`IWDG` gating + final feed, `SoftResetMailbox` ordering, `boot_jump()` cleanup + `app_vector_is_valid`, `EPIDR`/UID addressing, `NACK` `0xF1` on `dispatch_one()` `Err`, `read_bytes` upper bound `APP_END`, and code-size consolidation to stay `<= 8,192` bytes).
- [x] 2. Update `host/scripts/flash_app_over_i3c_usb.py` (`RESP_NACK = 0xF1`, `RESP_CMD_ERR = 0xF2`, I3C SDR `private_write([0xF0])` trigger).
- [x] 3. Build `bareboot-stm32h503-i3c` (`--release`), verify `.text + .rodata <= 8192` bytes, and run `cargo test` in `bareBoot`.
- [x] 4. Archive `stm32h503-i3c-production-hardening` and commit.
