## Why

The first BareBoot milestone successfully validated a **Katapult-compatible UART bootloader on Bluepill**. The CH32V203 port now builds and flashes, and the same framed protocol can serve as a hardware-capable secondary validation target, but the USB transport itself is not yet a functional or production-ready part of BareBoot.

We need a documented decision and implementation path for USB that preserves the shared Katapult byte protocol while avoiding a transport stack that is too large for the 4 KiB/8 KiB target class.

## What Changes

- Define a portable **byte-stream USB adapter API** that maps USB packet transactions into the same protocol parser used by UART, CAN, and I2C.
- Prove a CH32 USB CDC data path, initially as an **8 KiB experimental target**.
- Establish **fail-fast host tooling**:
  - device discovery,
  - stable transaction retry boundaries,
  - error markers before flashing, and
  - enough logging to separate electrical failures from software failures.
- Reuse `usb-device`, `stm32-usbd`, and `usbd-serial` for **development/validation images** but keep production 4/8 KiB paths open for a minimal low-level `USB FS` translation.
- Introduce guided **size-bounded profiles**:
  - 4 KiB for constrained MCUs
  - 8 KiB for experiment or outer transport-heavy targets
  - 16 KiB experimental images with HAL-assisted USB

## Capabilities

### New Capabilities
- `usb-transport-abstraction`: shared CDC byte-stream bridge for BareBoot host and target runtime.
- `ch32-usbfs-bringup`: low-level CH32V203 USB FS / CDC experimental transport.
- `cross-platform-host-usb`: OS-compatible discovery, initialization, and error handling for CDC bootloaders.

### Modified Capabilities
<!-- No main-spec requirements to modify yet; this change proposes new capabilities. -->

## Impact

- **Firmware targets**:
  - `bareboot-stm32f1-usb` (validated experimental target)
  - a new `bareboot-ch32v203-usb` target if HAL-based bring-up is worth the size tradeoff
  - eventual `bareboot-ch32v203` USB transport module for production-sized images
- **Host tooling**:
  - cross-platform CDC discovery should be built around discovery plus explicit retry boundaries rather than platform-specific serial device assumptions
  - `bareboot-cli`, `bareboot-tui`, `bareboot-client` all converge on one byte-stream contract
- **Dependencies**:
  - Rust targets and `usb-device`/`usbd-serial` crates for development and HAL experimentation
  - Katapult-style low-level PMA/endpoint/FIFO code for future 8 KiB work
- **Documentation and validation**:
  - hardware matrix for the USB FS endpoint:
  - CH32V203 PA11/PA12
  - ST/BluePill PA11/PA12
  - correct VID/PID discovery table in host tooling
