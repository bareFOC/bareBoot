# Proposal

## Why

Embedded actuator and control nodes (CAN, USB, UART, I2C, I3C) in robotics and 3D printing often run on flash-constrained microcontrollers where dual-bank A/B partitioning wastes up to 50% of available program memory. Furthermore, while upstream Katapult supports CAN, USB, and UART, it lacks native support for I2C and high-speed MIPI I3C buses. This change establishes a Katapult-compatible, single-slot in-situ bootloader architecture that incorporates Embassy-Boot's best reliability patterns (watchdog protection during long block operations, monotonic bit-flip commit records, and instruction cache invalidation) while introducing native I2C target and MIPI I3C dynamic target transports for bus-connected devices. Crucially, it guarantees non-volatile memory (NVM) retention across OTA updates and defines end-to-end hardware verification on the connected FishyServo actuator via the AgentLA test harness.

## What Changes

- **Single-Slot In-Situ Engine**: Enforce direct in-situ programming into a single application slot with no A/B partitions, maximizing usable application flash.
- **NVM & Calibration Persistence**: Strictly bound all erase and program operations to the application slot (`[APP_BASE, APP_END)`), ensuring non-volatile configuration memory (e.g. `H5FlashStorage` sectors `0x0801_C000..0x0802_0000` on STM32H503, storing motor calibrations, PID gains, and node IDs) remains preserved across all OTA updates.
- **Monotonic Bit-Flip Commit Journal**: Replace full sector erase-per-state transitions with sequential word bit-flipping (`0xFFFFFFFF` -> `UPDATE_IN_PROGRESS` -> `VALID`), preventing corruption from mid-erase brownouts and trapping incomplete transfers in Katapult recovery.
- **Watchdog Protection During Flash Writes**: Implement the `WatchdogFlash` pattern within `bareboot-core`'s `PagedWriter` to continuously service hardware watchdogs across slow bus transfers and multi-sector erase loops.
- **Clean Architecture Handoff**: Ensure instruction cache invalidation (`p.SCB.invalidate_icache()`) and peripheral clean-down on Cortex-M7/M33 and RISC-V targets before branching to application code.
- **I2C Non-Blocking Target Protocol**: Formalize the Katapult frame encapsulation over standard I2C target mode, eliminating SCL clock stretching and supporting in-band `0xF0` entry strobes.
- **MIPI I3C Dynamic Target Transport**: Provide dynamic I2C/I3C protocol detection, support for MIPI CCC `RSTACT` (Target System Reset), Dynamic Address Assignment (`ENTDAA`/`SETDASA`), and In-Band Interrupt (IBI) completion signaling.
- **Standardized HAL Adapter**: Introduce an adapter bridging `embedded-storage::nor_flash::NorFlash` traits into `bareboot-core::FlashTarget` for cross-MCU driver reuse.
- **Hardware In-Loop Verification**: Validate seamless handsfree I2C/I3C updates on connected FishyServo hardware (STM32H503) orchestrated by AgentLA (`10.10.10.10`).

## Capabilities

### New Capabilities
- `katapult-single-slot-engine`: Single-slot in-situ Katapult flashing engine featuring monotonic bit-flip commit state tracking, automated watchdog feeding during flash operations, safe CPU cache invalidation handoff, and strict NVM partition preservation across OTAs.
- `i3c-dynamic-bootloader`: MIPI I3C dynamic target bootloader transport supporting automated I2C/I3C bus switching, MIPI CCC `RSTACT` hardware reset, and non-blocking Katapult packet exchanges.

### Modified Capabilities
(none)

## Impact

- `crates/bareboot-core`: `PagedWriter` watchdog hooks, `embedded-storage` adapter, monotonic commit state tracking, and strict address bounds checking protecting NVM.
- `crates/bareboot-protocol`: Formalization of I2C/I3C framing contracts and CCC definitions.
- `targets/`: Cortex-M and RISC-V handoff routines with cache invalidation; STM32H503 and RP2350 target implementations.
- `host/bareboot-cli` and `host/bareboot-client`: First-class I2C/I3C bus backend integration alongside serial and CAN.
- Target application (`FishyServo`): Retains all calibration parameters (`H5FlashStorage`) across updates without re-calibration.
- Upstream Katapult Wire Protocol: **Fully preserved and backward compatible** with `flashtool.py` and Klipper tooling.
