# Design: Configurable Single-LED Status Indicator

## State Priority & Pattern Calculation

In `bareboot-core`:
1. **`BootStatus` priority order** (evaluated on each 1 ms tick):
   - `Flashing`: `flash_activity_ms > 0` (reset to `150` on `CMD_WRITE_BLOCK` / `CMD_WRITE_PAGE` / `CMD_COMPLETE`)
   - `BootLoopError`: `boot_loop_trapped || flash_error`
   - `BlankApp`: `!app_integrity_ok`
   - `BootWindow`: `boot_window_active`
   - `Parked`: otherwise (valid app, parked by `REQUEST_CANBOOT`)
2. **`compute_led_on(status: BootStatus, ms: u32) -> bool`**:
   - Uses `let phase = ms % 1000;` (integer modulo only, no floats, < 60 bytes Thumb-2):
     - `BootWindow` -> `true`
     - `Parked` -> `phase < 500`
     - `Flashing` -> `(ms % 100) < 50`
     - `BlankApp` -> `phase < 100 || (phase >= 200 && phase < 300)`
     - `BootLoopError` -> `phase < 80 || (phase >= 160 && phase < 240) || (phase >= 320 && phase < 400)`
3. **HAL Simplicity**:
   - `TickTimer::on_heartbeat(&mut self, led_on: bool)` only receives `led_on` when the computed boolean changes (avoiding unnecessary register writes on every 1 ms tick).
4. **Target `build.rs` Codegen (`targets/stm32h503-i3c`)**:
   - Reads `[bootloader].status_led` from TOML (if `BAREBOOT_CONFIG` or `BAREFOC_BOARD` is provided; default `"!PA0"`), parses port (`A..H`), pin (`0..15`), and active-low (`!`), and emits `STATUS_LED_PORT`, `STATUS_LED_PIN`, `STATUS_LED_ACTIVE_LOW` (or `None` when omitted).
