## Context

BareBoot has already validated the core protocol and `PagedWriter` on Bluepill through UART. The unresolved USB work is not a protocol gap; it is a **transport and runtime mismatch**:
- `ch32-hal` + Embassy is usable for bringing up USB, but it is async and too heavy for the 4 KiB/8 KiB boot targets.
- `usb-device`, `usbd-serial`, `stm32f1xx-hal`, and `ch32-hal` are excellent references but do not satisfy the clean trait-oriented size target on their own.
- `ch32fun` supplies a much smaller, low-level USBFS/CDC stack built around explicit interrupt handlers, endpoints, and 64-byte data buffers.
- The Katapult wire protocol itself is transport-neutral and all host/tooling can use one byte-stream adapter once descriptors and frame boundaries are normalized.

## Goals / Non-Goals

**Goals**
- Define a portable byte-transport abstraction across UART, USB, CAN, and I2C.
- Implement the first specialized USB transport based on the `ch32fun` fsusb reference patterns.
- Keep CPU/main-loop logic minimal and synchronized around one `SCAN/DISPATCH/ACK` pass.
- Provide truthful size measurements by target family, not assume 4 KiB for all chips.

**Non-Goals**
- Reimplementing USB as a generic crate for all major MCU families at once.
- Forcing `usb-device` or full Embassy into the production 4 KiB target.
- Discarding the existing HAL-based experimental firmware before it has isolated CDC and endpoint quirks.

## Decisions

### Use a shared bytes boundary
- `FlashSession` stays bytes-only, while the transport fills its send queue and drains receive frames.
- Host side code should stay one `SerialBusBackend` binding for CDC ACM and switch between OS-specific discovery table entries.

### Use family-specific transport drivers
- A CH32 low-level driver cannot share PIC/FS layout details across vendors.
- Shared items are trait, buffer shape, endpoint constants, and profile selection.
- Register sequences are per-chip.

### Use a response staging buffer
- All ACK responses are assembled fully before transmission.
- `CONNECT`, `RX_BLOCK`, `RX_EOF`, and `COMPLETE` each stage one whole Katapult frame into the active packet queue.
- The goal is to avoid USB-class partial reads or dispatching a second command while an IN transfer is still pending.

### Use explicit profile selection
- 4 KiB: UART/CAN/I2C only
- 8 KiB: one lean USB CDC path
- 16 KiB: HAL-assisted bring-up and validation

## Risks / Trade-offs
- [Interrupt ordering errors] -> Mitigation: use one IRQ handler model and a simple state bitmask for each endpoint's busy/formal boundary.
- [HLE race conditions] -> Mitigation: freeze the response queue boundary on a packet level.
- [Host port instability] -> Mitigation: retain `probe-rs`/manual frame scripts for direct ACK inspection.
- [Size drift] -> Mitigation: gate build profiles and fail if the final image exceeds the selected target bucket.
- [Timing flukes] -> Mitigation: early work uses no verification marker discipline that compromises the parser.

## Migration plan
1. Capture the byte-oriented host adapter and transport profile as an OpenSpec change.  (done here)
2. Add one per-family low-level transport module with CDC descriptors.
3. Reuse the BluePill UART/protocol coverage as the transaction contract.
4. Validate flash readback and boot jumps only after the ACK path is explicit.
5. Benchmark USB size against the 8 KiB default.

## Open questions
- Can USB on CH32V203 fit within 8 KiB after the full CDC descriptor tables, or is 16 KiB required?
- Should the host-side discovery table be shared or generated per vendor/hardware model?
- Is `usb-device` worth carrying, or should the low-level endpoint logic be expanded to full interrupt-backed handshakes?'

---

## Detailed implementation roadmap

### Phase 1 — Capture the concrete size / clock profiles
- Add a `bareboot-ch32v203` profile with `flash_size = 8192`, `app_base = 0x08002000`, default transport `uart` only.
- Keep all debug symbols and one full-size HAL experiment in a separate target (not in the production 8 KiB path).
- Emit `put_bold` markers only in a debug profile, never in the production image.
- Resolve the page program sequence before adding USB.

### Phase 2 — low-level USB minimum: control flow
- Port a minimal, interrupt-safe `USBFS` body into `bareboot/targets/bareboot-ch32v203/src/usbfs.rs`.
- Scope the first pass strictly to:
  - `usbd` endpoint 0 handshake
  - device/endpoint descriptors
  - bulk IN/OUT queue
  - frame receive buffer isolation
- Do **not** attempt CDC ACM composite/class tables beyond the known minimum descriptor set required by Katapult transaction start.
- Retain the expected mechanical coupling to the PC (`USBFS` register blocking within core only), and never block the serial protocol while waiting for HEN bit settling.

### Phase 3 — byte contract and staging
1. Clear the RX current buffer on every new transaction.
2. Receive bounded frames into `packet[64]`.
3. Run one `FrameParse` pass per dispatch pass.
4. Only parse a `FrameParse::Complete` when `frame_len <= 128` and at least `rx_len` is available.
5. Queue every response into a `heapless::Vec<u8, 256>` and output **only** whole `ACK/NACK/ERROR` frames.
6. Never return a partial ACK or expose the internal RX buffer to the protocol boundary.

### Phase 4 — CH32 timing discipline
- Keep `rx_len` zero during the 250–500 ms delay window.
- Do not call `bxCAN` on targets that are not configured for it.
- Maintain a single trigger after `COMPLETE` and do not re-arm it.
- Use pull-up lines on PA10 to reduce spurious frame residue.
- Reuse the `write_page_bytes` sequence with `CR_PAGE_PG` / `CR_PG_STRT`.

### Phase 5 — hardware transaction ladder
- `CONNECT` only
- `CONNECT` + one `RX_BLOCK` + response readback
- `CONNECT` + `RX_BLOCK` + `EOF` with no flash commit
- `CONNECT` + `RX_BLOCK` + `EOF` + flash commit (verify from raw dump)
- `COMPLETE`
- application blink/jump test
- finalize by real hardware timing and condition A bit checks from `PENDING.md`

### Status expectations
- 4 KiB allowed only for strict, single-message paths.
- 8 KiB is the practical default for the CH32 USB transition.
- 16 KiB remains the fallback for experiments.
- Pure register observation is preferred over the HAL/Embassy dependency.
