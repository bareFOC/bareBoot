## 1. Commit-state storage and linker layout

- [x] 1.1 Add a `commit_state` module defining magic, `VALID`/`UPDATE_IN_PROGRESS` states, complement checking, and flash read/write. Implemented in both `targets/rp2350-usb/src/commit_state.rs` (Rust) and `ref/katapult/src/commit_state.c/.h` (production transport).
- [x] 1.2 Reserve the 4 KB state sector at flash offset 0x7000 (last sector of the 32 KB bootloader slot): `memory.x` (FLASH 28K + STATE 4K) in the Rust target; `COMMIT_OFFSET 0x7000` in the Katapult C target.
- [x] 1.3 Add host-testable pure helpers for state encoding/decoding; Rust unit tests pass (11/11).

## 2. Boot decision hardening

- [x] 2.1 Thumb-bit check (`pc & 1 == 1`) in `valid_app_vector` / `application_check_valid`.
- [x] 2.2 Boot decision requires `VALID` commit state (`should_enter_recovery` / `application_check_valid`).
- [x] 2.3 Unit tests for layout constants and decision matrix updated and passing.

## 3. Protocol loop integration

- [x] 3.1 First `SEND_BLOCK` marks `UPDATE_IN_PROGRESS` before any erase.
- [x] 3.2 `COMPLETE` (host-verified) marks `VALID`.
- [x] 3.3 BOOTSEL fallback removed; flash errors NACK and stay in recovery / self-reset.
- [x] 3.4 Cookie cleared only when jumping to a valid committed app.

## 4. Build and packaging correctness

- [x] 4.1 `cargo test` 11/11 in `targets/rp2350-usb`; full workspace suite passes.
- [x] 4.2 `cargo build --release --target thumbv8m.main-none-eabihf` passes (19,828 B text <= 28 KB code region).
- [x] 4.3 Stale `targets/bareboot-rp2350` path fixed in AgentLA `.github/workflows/release.yml` and `tools/package_uf2.py`.
- [x] 4.4 RP2350 bootrom PICOBIN image definition added to the Rust target (verified at flash offset 0x40).
- [x] 4.5 `--commit-valid` added to `package_uf2.py` for factory UF2s (verified byte-for-byte against Rust `encode_record(STATE_VALID)`).
- [x] 4.6 Production transport built from Katapult: `scripts/build_rp2350_usb.sh` builds `ref/katapult/out/katapult.bin` (4,764 B) with config `bareboot-rp2350-usb.config` (app base 0x10008000, USB CDC 1d50:6177).

## 5. Hardware verification (Pico 2 / RP2350 A4, debugprobe)

- [x] 5.1 Flashed the bootloader at 0x10000000 via SWD (probe-rs) and BOOTSEL UF2 (picotool).
- [x] 5.2 USB CDC enumerates as `1d50:6177`; stock `flashtool.py -d /dev/ttyACM1 -s` reports "Katapult Connected", protocol 1.1.0, Application Start 0x10008000, MCU rp2350. (Embassy Rust target failed this; Katapult-based target passes.)
- [x] 5.3 Full stock flashtool cycle: `flashtool.py -f /tmp/app.bin` wrote 3039 pages, verified 12153 blocks with SHA1 match, sent COMPLETE. Commit record read back as `42425354 00000001 fffffffe bdbdacab` (VALID). After reset the app booted (AgentLA USB `1209:414d` enumerated).
- [x] 5.4 Torn-image protection: interrupted a transfer after all blocks written but before COMPLETE; commit record read back as `42425354 00000002 fffffffd bdbdacab` (UPDATE_IN_PROGRESS). After reset the device stayed in BareBoot Katapult recovery (`1d50:6177`) and did NOT boot the fully-written app.
- [x] 5.5 No BOOTSEL fallback: `grep -rn reset_to_usb_boot targets/rp2350-usb/src/` returns nothing; Katapult transport contains no `reset_to_usb_boot`. Factory flashing (BOOTSEL/SWD) is one-time only.
