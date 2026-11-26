use crate::{BootConfig, CoreError, WatchdogFeed};
use embedded_storage::nor_flash::NorFlash;

/// Pure flash storage region decoupled from CPU jump, UID, or commit-journal concerns.
pub trait FlashRegion {
    fn page_size(&self) -> usize;
    fn region_base(&self) -> u32;
    fn region_end(&self) -> u32;
    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError>;
    fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError>;
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError>;
}

/// Adapter bridging `embedded_storage::nor_flash::NorFlash` into `FlashRegion`.
pub struct NorFlashRegion<F> {
    flash: F,
    flash_base: u32,
    region_base: u32,
    region_end: u32,
}

impl<F: NorFlash> NorFlashRegion<F> {
    pub const fn new(flash: F, flash_base: u32, region_base: u32, region_end: u32) -> Self {
        Self {
            flash,
            flash_base,
            region_base,
            region_end,
        }
    }

    pub fn inner(&self) -> &F {
        &self.flash
    }

    pub fn inner_mut(&mut self) -> &mut F {
        &mut self.flash
    }

    pub fn into_inner(self) -> F {
        self.flash
    }

    fn to_offset(&self, addr: u32, len: usize) -> Result<u32, CoreError> {
        if addr < self.region_base || addr.saturating_add(len as u32) > self.region_end {
            return Err(CoreError::InvalidAddress);
        }
        addr.checked_sub(self.flash_base)
            .ok_or(CoreError::InvalidAddress)
    }
}

impl<F: NorFlash> FlashRegion for NorFlashRegion<F> {
    fn page_size(&self) -> usize {
        F::ERASE_SIZE
    }

    fn region_base(&self) -> u32 {
        self.region_base
    }

    fn region_end(&self) -> u32 {
        self.region_end
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        let from = self.to_offset(addr, F::ERASE_SIZE)?;
        let to = from
            .checked_add(F::ERASE_SIZE as u32)
            .ok_or(CoreError::InvalidAddress)?;
        self.flash.erase(from, to).map_err(|_| CoreError::Flash)
    }

    fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        let offset = self.to_offset(addr, data.len())?;
        self.flash.write(offset, data).map_err(|_| CoreError::Flash)
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        let offset = self.to_offset(addr, out.len())?;
        self.flash.read(offset, out).map_err(|_| CoreError::Flash)
    }
}

/// Architecture-specific boot, reset, and vector validation primitives.
pub trait ArchBoot {
    fn is_valid_pc(pc: u32, app_base: u32, app_end: u32) -> bool;

    /// Jump to the application at `app_base`.
    ///
    /// # Safety
    /// Caller must ensure `app_base` holds a valid application image and vector table.
    unsafe fn jump_to_app(app_base: u32) -> !;

    fn system_reset() -> !;
}

/// 1 ms polled tick timer for synchronous boot-window and idle-timeout tracking.
pub trait TickTimer {
    fn poll_1ms(&mut self) -> bool;

    /// Optional 250 ms heartbeat callback (e.g. status LED toggle).
    fn on_heartbeat(&mut self, _led_on: bool) {}
}

/// Event returned by `BusTransport::poll_rx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollEvent {
    None,
    Frame(usize),
    BusActivity,
    BusReset,
}

/// Synchronous bus transport abstraction (I3C, I2C, CAN, UART, USB).
pub trait BusTransport {
    fn poll_rx(&mut self, buf: &mut [u8]) -> PollEvent;
    fn send_tx(&mut self, frame: &[u8]) -> Result<(), CoreError>;
    fn tx_idle(&self) -> bool;

    /// Optional 1 ms periodic bus maintenance hook (e.g. stuck-SDA or idle-bus recovery).
    fn on_tick_1ms(&mut self) {}

    /// Optional raw bus activity check to reset idle timeout even before a full frame arrives.
    fn has_activity(&mut self) -> bool {
        false
    }

    /// Optional delay/drain hook called after `CMD_COMPLETE` ACK before jumping to app.
    fn wait_tx_drain(&mut self) {}

    /// Optional teardown hook called right before `ArchBoot::jump_to_app()`.
    fn prepare_for_jump(&mut self) {}
}

/// Hardware reset cause classification used by `BootLoopGuard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetReason {
    PowerOn,
    Software,
    Watchdog,
    Other,
}

/// Composite Board Support Package (BSP) trait driving `BootRunner`.
pub trait BootPlatform: Sized {
    type Flash: FlashRegion;
    type CommitFlash: FlashRegion;
    type Transport: BusTransport;
    type Watchdog: WatchdogFeed;
    type Timer: TickTimer;
    type Arch: ArchBoot;

    fn boot_config(&self) -> BootConfig;
    fn ram_bounds(&self) -> (u32, u32);
    fn boot_window_ms(&self) -> u32;

    fn idle_timeout_ms(&self) -> Option<u32> {
        None
    }

    fn mailbox_addr(&self) -> Option<u32> {
        None
    }

    fn legacy_cookie_addr(&self) -> Option<u32> {
        None
    }

    fn boot_loop_guard_addr(&self) -> Option<usize> {
        None
    }

    fn reset_reason(&self) -> ResetReason;

    fn read_mailbox_request(&mut self) -> bool {
        let guard_req = self
            .boot_loop_guard_addr()
            .map_or(false, crate::check_and_clear_bootloader_request);
        let mb_req = self.mailbox_addr().map_or(false, |addr| {
            let ptr = addr as *mut bareboot_protocol::SoftResetMailbox;
            // SAFETY: `mailbox_addr` is a valid, aligned SRAM address provided by the BSP.
            let mb = unsafe { core::ptr::read_volatile(ptr) };
            if mb.is_valid() {
                let req = mb.magic == bareboot_protocol::REQUEST_BOOTLOADER;
                // SAFETY: Clearing the 8-byte magic header at `addr`.
                unsafe { core::ptr::write_volatile(addr as *mut u64, 0) };
                req
            } else {
                false
            }
        });
        let legacy_req = self
            .legacy_cookie_addr()
            .map_or(false, |addr| crate::check_and_clear_bootloader_request(addr as usize));
        guard_req || mb_req || legacy_req
    }

    fn into_parts(
        self,
    ) -> (
        Self::Flash,
        Self::CommitFlash,
        Self::Transport,
        Self::Watchdog,
        Self::Timer,
    );
}
