
## CH32V203 status update
- CONNECT test was successful once with the PB6/PB7 firmware and WCH-Link UART bridge.
- The same firmware now returns no CONNECT reply in some resets.
- Likely cause: WCH-Link reset timing and serial-bridge routing on CH32 dev kits are inconsistent.
- Host CLI still needs a clean transaction run after a successful adapter response.
- Recommended next 2 attempts:
  1.  Use `PB6/PB7` firmware and send the CONNECT frame with:
     `python per-byte time.sleep(0.005)` then `time.sleep(1.0)`.
  2.  Run immediately:
     `bareboot-cli --serial /dev/ttyACM0 --baud 115200 --firmware test.bin --tries 3 --timeout 5 --no-verify`

## Key action items completed
- CH32V203 target leaf page size corrected to 256 bytes, matching ch32fun's CH20X/30X flash characteristics.
- The build uses the existing `PagedWriter`/trait path, so no protocol or host changes are required.

## USB HAL analysis findings
- `ref/ch32-hal` supports USB OTG FS on CH32 V2/V3.
- Its USB implementation is Embassy/async-based and relies on:
  - `hal::usbd::Driver`
  - `embassy-usb` device stack
  - interrupt binding and time-driver support
- It is suitable for CDC bring-up and experimental targets, but not for the BareBoot 4 KiB/8 KiB production plan.
- A practical experiment should build the HAL-based USB target separately and measure it, but the production path should remain the low-level Katapult-style USB FS/CDC driver.

## CH32V203 USB bootloader testing
- Experimental CH32 USB using `ch32-hal` + Embassy is documented but not integrated into BareBoot.
- `ref/ch32-hal/examples/ch32v203/src/bin/usbd.rs` requires an Embassy executor and interrupt-driven USB stack; the build is currently nontrivial against the locked stack versions available locally.
- A BareBoot-proper USB target would need:
  - 8 KiB or 16 KiB app offset
  - `ch32v203c8t6`
  - USBD on PA11/PA12
  - a USB class that starts serial or provides the Katapult byte stream
- Before testing:
  - Use a separate HAL-based USB test firmware to confirm enumeration on `ttyACM*`.
  - Then adapt only the transport/router logic into BareBoot.

## Very lightweight USB stack option (ch32fun)
- `ref/ch32fun/extralibs/fsusb.c` is only ~1.1kloc and contains enough USBFS descriptor handling for CDC transports.
- It may be cheaper to reuse the **transformation patterns** from `ch32fun` than to adopt `stm32-usbd + ch32-hal`, mainly for a 16–24 KiB experimental image.
- Production 4/8 KiB BareBoot images still need a hand-ported C-to-Rust translation because the stack is WCH-specific.

### USB transaction (latest)
A HAL-based CH32 USB target now runs through CONNECT + RX_BLOCK and reports:
```text
connected: mcu=ch32v203c8 block_size=64 app_base=0x8001000
progress: 100% (60/60)
eof: 1 pages written
done
```
Readback of `0x08001000` returned `0x03 25 c1 15 …` instead of the uploaded `00 50 00 20 …` pattern. The USB framing layer is close, but the flash programming result is not yet honoring the host payload.

### Next root-cause work
1. Inspect `erase_page`/`write_page_bytes` with a forced readback marker.
2. Ensure `PagedWriter` does not confuse USB burst boundaries or use an incorrect word formatting path.
3. Compare with `ch32fun/extralibs/ch20x_30x_flash.h` before ever assuming a 256-byte page operation succeeded.
4. Do not use `--no-verify` until readback succeeds.

## USB low-level hardware note
- Latest `wlink status` still returns the 0x55 protocol error and no WCH transaction is available in `--watch-serial`.
- The WCH-Link should stay on **RV mode** for all CH32V203 work.
- If the link drops into probe mode, rerun:
  ```bash
  wlink mode-switch --rv
  ```

## CH32 UART deferred — USB is now the primary CH32 path
- CH32 UART CONNECT handshake and frame readback are **deferred**; focus has shifted to the USBFS CDC transport.
- Do not block CH32 USB work on UART validation.
- CH32 UART verification should only resume after low-level USB byte contract is proven.
