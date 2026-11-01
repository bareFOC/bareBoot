# CH32 Low-Level USB Research Findings

## 1.1 Register assumptions (identical to existing low-level path)
- Base USB controller is accessed through `ch32-metapac::peripherals::Usb` (`USB` / `USB_FS` peripheral).
- Endpoint 0-7 register layout (`epr`, `cntr`, `istr`) mirrors the STM32/CW layout family.
- PAC is the only HAL-free source of truth; `ch32-hal` is async-only and not used for production.

## 1.2 Minimum CDC descriptor and endpoint requirements
- EP0: control (64-byte)
- EP1: bulk IN (64-byte) for ACK frames
- EP2: bulk OUT (64-byte) for incoming Katapult frames
- Descriptor needs: Device, Configuration, Interface (CDC Control + CDC Data), endpoint descriptors.

## 1.3 Transaction playbook
1. `CONNECT` response -> ACK
2. One `RX_BLOCK` -> ACK
3. `EOF` (no commit) -> ACK
4. `EOF` (commit) -> verify flash @ 0x08001000
5. `COMPLETE` -> jump to app
6. App blink check

## Hardware blockers
- WCH-Link currently reports protocol fault `0x81 0x55 0x01 0x01`; no stable serial bridge.
- No `/dev/ttyUSB*` external adapter attached in this session.

## 4.1 Timing review
- Respect max USB turnaround (single `RX_BLOCK` ≤ 64 bytes).
- ACK must be queued within the frame interval (1 ms FS) for `CONNECT`, `RX_BLOCK`, `COMPLETE`.
- Do not call `bxCAN` in tight interrupt context.
- 8 KiB profile is appropriate for production; 16 KiB only for debug/SNC.
