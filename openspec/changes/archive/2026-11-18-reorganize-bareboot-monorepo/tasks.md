## 1. Directory Structure Setup

- [x] 1.1 Create category directories: `crates/`, `targets/`, `host/`, `configs/`, `artifacts/`, `tests/`

## 2. Core Firmware Crates Relocation

- [x] 2.1 Move `bareboot/crates/bareboot-protocol` to `crates/bareboot-protocol`
- [x] 2.2 Move `bareboot/crates/bareboot-core` to `crates/bareboot-core`
- [x] 2.3 Remove obsolete broken `bareboot/` directory

## 3. Target Bootloader Consolidation

- [x] 3.1 Move `targets/bareboot-rp2350` to `targets/rp2350-usb`
- [x] 3.2 Move root `bareboot-ch32v203-usb` to `targets/ch32v203-usb`
- [x] 3.3 Create `targets/stm32h503/` and consolidate `i3c-i2c-bootloader` C source (`i3c_dynamic_target.c`, `bootentry.c`) into `targets/stm32h503/src/`
- [x] 3.4 Consolidate STM32H503 patches (`Kconfig.patch`, `i3c_dynamic_target.patch`, `stm32h503-i2c-fixes.patch`) into `targets/stm32h503/patches/`
- [x] 3.5 Remove obsolete `i2c-bootloader`, `i3c-bootloader`, and `i3c-i2c-bootloader` directories

## 4. Host Tooling and Scripts Consolidation

- [x] 4.1 Move `bareflash/crates/*` to `host/*` (`host/bareboot-client`, `host/bareboot-cli`, `host/bareboot-tui`)
- [x] 4.2 Move root `bareflash.sh` and `wch-udev-fix.sh` into `host/scripts/`
- [x] 4.3 Move existing `scripts/*` into `host/scripts/`
- [x] 4.4 Relocate `flash_app_over_i2c.py` and `flash_app_over_i3c.py` into `host/scripts/`
- [x] 4.5 Remove obsolete `bareflash/` and root `scripts/` directories

## 5. Configs, Artifacts, Tests, and Root Cleanup

- [x] 5.1 Move `bareboot*.toml` board configs from root into `configs/`
- [x] 5.2 Move loose `.bin` and `.elf` files from root and bootloader directories into `artifacts/`
- [x] 5.3 Consolidate root test crates (`blink-test`, `flashprobe`, `standalone-pwriter`) and `test-firmware/` into `tests/`
- [x] 5.4 Purge orphaned zero-byte files (`barefoot`, `barefoot_file`, `test-firmware/ch32-uart-parent`)

## 6. Manifest and Path Configuration

- [x] 6.1 Create root `Cargo.toml` virtual workspace defining members `crates/*` and `host/*`
- [x] 6.2 Update `targets/rp2350-usb/Cargo.toml` path dependencies to `../../crates/bareboot-protocol` and `../../crates/bareboot-core`
- [x] 6.3 Update `targets/ch32v203-usb/Cargo.toml` path dependencies to `../../crates/bareboot-protocol`, `../../crates/bareboot-core`, and `../../ref/ch32-hal`

## 7. Verification and Documentation

- [x] 7.1 Execute `cargo test --workspace` on root workspace and verify all tests pass
- [x] 7.2 Run `cargo check` and host unit tests for `targets/rp2350-usb`
- [x] 7.3 Update `README.md` to document the new monorepo directory layout and build commands
