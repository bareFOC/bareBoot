# Design: `bareboot-hal-platform-traits`

## Trait Hierarchy (Static Dispatch, `#![no_std]`)

```rust
pub trait FlashRegion {
    fn page_size(&self) -> usize;
    fn region_base(&self) -> u32;
    fn region_end(&self) -> u32;
    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError>;
    fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError>;
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError>;
}

pub trait ArchBoot {
    fn is_valid_pc(pc: u32, app_base: u32, app_end: u32) -> bool;
    unsafe fn jump_to_app(app_base: u32) -> !;
    fn system_reset() -> !;
}

pub trait TickTimer {
    fn poll_1ms(&mut self) -> bool;
}

pub trait BusTransport {
    fn poll_rx(&mut self, buf: &mut [u8]) -> PollEvent;
    fn send_tx(&mut self, frame: &[u8]) -> Result<(), CoreError>;
    fn tx_idle(&self) -> bool;
}

pub trait BootPlatform {
    type Flash: FlashRegion;
    type CommitFlash: FlashRegion;
    type Transport: BusTransport;
    type Watchdog: WatchdogFeed;
    type Timer: TickTimer;
    type Arch: ArchBoot;
    ...
}
```

## Compatibility & Migration
- Keep `FlashTarget` / `NorFlashAdapter` temporarily as thin compatibility bridges (or migrate `PagedWriter` + `MonotonicCommitJournal` onto `FlashRegion` with a blanket/adapter impl for `embedded_storage::nor_flash::NorFlash`) so existing targets (`rp2350-usb`, `ch32v203-usb`) continue building without breakage while `stm32h503-i3c` pilots `BootRunner`.
- `BootRunner` runs synchronously (polled `1 ms` `TickTimer`) so `<= 16 KiB` targets do not pull in an async executor.
- `SoftResetMailbox::ADDRESS` remains an optional default constant while `BootPlatform::mailbox_addr(&self) -> Option<u32>` allows each MCU to specify its own top-of-SRAM address.
