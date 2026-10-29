## Context

BareBoot already validates its Katapult-compatible protocol and flashing logic on the **BluePill/STM32F1 UART path**. The shared protocol stack is deliberately byte-oriented, and the host-side tooling treats each bus as a plain byte stream under `FlashSession`.

USB is a natural future transport, but it fundamentally differs from UART:
- USB arrives in fixed-ish **packets**, not a continuous byte stream
- endpoints and descriptors require activation and enumeration
- some HAL stacks impose async runtimes (Embassy) that are heavyweight for flash-constrained boot images
- host operating systems expose USB CDC through different device models despite a ChuNKY serialization problem

## Goals / Non-Goals

**Goals**
- Support USB CDC through a shared byte-transport adapter into `FlashSession`.
- Keep protocol handling identical between UART/CAN/I2B///I2C and USB.
- Provide a clear DMA/IO path for short, recovery-friendly USB images.
- Make 4 KiB production and 16 KiB experimental images supportable at the same time.

**Non-Goals**
- Reimplement a full Katapult USB stack from scratch in phase 1.
- Assume a single size target for all MCU families.
- Remove current UART on successful device USB results.

## Decisions
- Prefer **Linux-first CDC discovery**, then cross-platform abstraction once the core semantics freeze.
- Use **`UsbBus::read`/`write` packet shaping** with explicit framing and response-queue isolation.
- Implement at least two **size profiles** while leaving the USB transport itself as a reusable, low-overhead HAL.
- Validate all firmware-side and host-side behaviors as separate tests, not one binary end-to-end experiment.

## Risks / Trade-offs
- [RUST async host discovery is tricky] -> [Start with direct serial adjacency and mark enumbered modes clearly in the docs]
- [Firmware will be heavier than UART-only] -> [Use 8 KiB+ for USB images while retaining 4 KiB for UART/CAN]
- [Platform differences in device nodes] -> [Use a common discovery routine with VID/PID rules plus per-OS fallbacks]

## Migration plan
1. Add `usb-cdc-transport` profile flags and metadata.
2. Implement the experimental USB test image (`stm32f1-usb`, `ch32v203-usb`) with clear scope.
3. Add host-facing `Transport` and discovery rules for CDC.
4. Validate one 64-byte transfer plus EOF after a reset on the board.
5. Transfer logs/show the verified response frames.
6. Only then add a production path candidate.

## Open questions
- Is a pure Katapult `usbfs.c` port worth the flash budget for the first production USB target?
- What minimum firmware image can reliably support USB CDC + 4-word frame flow?
- What is the smallest safe host-side latency for `CONNECT` and for packet-width reduced ACKs?
