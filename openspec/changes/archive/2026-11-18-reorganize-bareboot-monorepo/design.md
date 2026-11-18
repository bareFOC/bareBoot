## Context

The `bareBoot` repository hosts both firmware-level Katapult bootloaders (targeting diverse MCU architectures like ARM Cortex-M0+/M33 and RISC-V QingKe) and host-level flashing utilities (CLI, TUI, Client library, Python automation scripts).

Currently, the directory layout is severely disjointed:
- Core crates (`bareboot-protocol`, `bareboot-core`) are nested under `bareboot/crates/`.
- `bareboot/Cargo.toml` has dangling references to deleted targets in `bareboot/targets/`.
- Target bootloaders are scattered across `targets/bareboot-rp2350`, `bareboot-ch32v203-usb`, and root Katapult C directories `i2c-bootloader`, `i3c-bootloader`, `i3c-i2c-bootloader`.
- Host tools live under `bareflash/crates/` while flashing scripts reside in `scripts/`, `i2c-bootloader/`, and `i3c-bootloader/`.
- Stale zero-byte files and precompiled binaries pollute the repository root.

## Goals / Non-Goals

**Goals:**
- Unify repository layout into clear functional layers: `crates/`, `targets/`, `host/`, `configs/`, `artifacts/`, `tests/`, `docs/`, and `ref/`.
- Create a working root virtual `Cargo.toml` workspace encompassing host tools and core shared firmware crates so `cargo test` runs cleanly on the host.
- Standardize target directory structure under `targets/` with clear distinction between Rust targets (`targets/rp2350-usb`, `targets/ch32v203-usb`) and C/Katapult targets (`targets/stm32h503`).
- Consolidate all flashing scripts (Python and Bash) under `host/scripts/`.
- Centralize target definition configs under `configs/`.
- Purge empty junk files and relocate precompiled binary blobs to `artifacts/`.

**Non-Goals:**
- Modifying wire protocol logic or Katapult message contracts.
- Porting STM32H503 C/Katapult dynamic target driver into Rust in this change.
- Altering the RP2350 flash layout (recovery slot at 0x10000000..0x10008000, app slot at 0x10008000..0x10140000).

## Decisions

### Decision 1: Root Virtual Workspace for Core Crates and Host Tools
- **Rationale**: Host crates (`bareboot-client`, `bareboot-cli`, `bareboot-tui`) and core firmware libraries (`bareboot-protocol`, `bareboot-core`) can all build and run tests under the host target architecture without embedded cross-compilers.
- **Alternatives Considered**:
  - Keep separate `bareboot/Cargo.toml` and `bareflash/Cargo.toml`: Leaves workspace duplication and requires entering subdirectories to run cargo commands.
  - Include embedded targets in root workspace: Causes `cargo check` on host to fail because embedded targets require specific bare-metal target triples (`thumbv8m.main-none-eabihf`, `riscv32imac-unknown-none-elf`).

### Decision 2: Isolated Target Crates in `targets/`
- **Rationale**: Embedded targets have distinct cross-compilation target triples, memory layouts (`memory.x`), and panic handlers. Keeping them as standalone crates under `targets/<name>` with path dependencies `../../crates/bareboot-core` and `../../crates/bareboot-protocol` provides clean build isolation.
- **Alternatives Considered**:
  - Monolithic multi-architecture workspace with cargo feature gating: Very fragile with conflicting platform runtime crates (`cortex-m-rt` vs `qingke-rt`).

### Decision 3: Consolidated Katapult C Target Structure
- **Rationale**: `i3c-i2c-bootloader`, `i2c-bootloader`, and `i3c-bootloader` represent the STM32H503 Katapult target implementation and its associated patches and scripts. Organizing them under `targets/stm32h503/` with `src/`, `patches/`, and `configs/` provides coherent target ownership.
- **Alternatives Considered**:
  - Leaving C code in root: Causes confusion with top-level repository components.

### Decision 4: Consolidated Scripts under `host/scripts/`
- **Rationale**: Scripts such as `flash_app_over_i2c.py`, `flash_app_over_i3c.py`, `ch32v203_flash.sh`, and `wch-udev-fix.sh` are host-side flashing and device management helpers. Placing them under `host/scripts/` clusters all host tools together.

## Risks / Trade-offs

- **[Broken path references in target Cargo.toml files]** → Update `path = "../../crates/bareboot-core"` and verify each target with `cargo check`.
- **[Broken script paths in documentation]** → Update all documentation links and README references to point to the new paths.
- **[Git history tracking]** → Use `git mv` for all file relocations to preserve full git history.

## Migration Plan

1. Create target directories (`crates/`, `targets/`, `host/`, `configs/`, `artifacts/`, `tests/`).
2. Move core crates from `bareboot/crates/*` to `crates/*`.
3. Move host crates from `bareflash/crates/*` to `host/*` and move `scripts/*` into `host/scripts/`.
4. Relocate `targets/bareboot-rp2350` to `targets/rp2350-usb/`, and `bareboot-ch32v203-usb` to `targets/ch32v203-usb/`.
5. Relocate `i3c-i2c-bootloader` and `i2c-bootloader` contents into `targets/stm32h503/`.
6. Consolidate Python flashing scripts (`flash_app_over_i2c.py`, `flash_app_over_i3c.py`) into `host/scripts/`.
7. Move `bareboot*.toml` configs into `configs/`.
8. Move test firmware into `tests/`.
9. Move binary blobs (`.bin`, `.elf`) into `artifacts/`.
10. Remove empty files (`barefoot`, `barefoot_file`, `ch32-uart-parent`) and delete obsolete empty directories.
11. Create root `Cargo.toml` virtual workspace and update all internal dependency paths.
12. Verify host workspace with `cargo test` and verify embedded targets with target-specific `cargo check`.
