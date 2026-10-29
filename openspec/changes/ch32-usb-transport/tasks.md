## 1. Discovery and scope capture
- [x] 1.1 Confirm which host OS environments are in scope for USB CDC discovery first.
- [x] 1.2 Confirm acceptable CH32 image size vs. required protocol behavior.
- [x] 1.3 Confirm the class contract is limited to CDC ACM before evaluating alternative endpoints.

## 2. Shared USB transport
- [x] 2.1 Design a host `UsbTransport` behind the common `FlashSession`.
- [x] 2.2 Map CDC ACM bytes to `Scan`, `Dispatch`, `Ack`, and initial EOF telemetry.
- [ ] 2.3 Add a Linux-first discovery adapter that enumerates `/dev/ttyACM*` and VID/PID metadata.

## 3. CH32 hardware bring-up
- [x] 3.1 Build the HAL-based CH32 USB experiment against a protected size profile.
- [x] 3.2 Validate CH32 USB CDC connection with a fixed `CONNECT` response.
- [ ] 3.3 Implement a single-stage timeout/retry sequence for USB ACK inference.

## 4. Firmware and host integration
- [x] 4.1 Integrate the shared interface into `bareboot-client`, `bareboot-cli`, and `bareboot-tui` only after the wire model is frozen.
- [ ] 4.2 Validate host-size images in 4 KiB, 8 KiB, and 16 KiB buckets.
- [ ] 4.3 Publish the hardware validation matrix after the transaction path is reliable.

## 5. Validation
- [ ] 5.1 Run one-transaction upload through the host proxy helper.
- [ ] 5.2 Run a 64-byte end-to-end upload, EOF, and jump sequence.
- [ ] 5.3 Confirm the CH32 app vector at `0x08001000` and any observable post-jump behavior.
