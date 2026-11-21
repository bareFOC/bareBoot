use crate::{BootConfig, CoreError, FlashTarget};

pub trait WatchdogFeed {
    fn feed(&mut self);
}

impl<F: FnMut()> WatchdogFeed for F {
    fn feed(&mut self) {
        self();
    }
}

/// Decorator that feeds a hardware or software watchdog before and after each flash operation.
pub struct WatchdogFlash<F, W> {
    flash: F,
    watchdog: W,
}

impl<F, W> WatchdogFlash<F, W> {
    pub const fn new(flash: F, watchdog: W) -> Self {
        Self { flash, watchdog }
    }

    pub fn flash(&self) -> &F {
        &self.flash
    }

    pub fn flash_mut(&mut self) -> &mut F {
        &mut self.flash
    }

    pub fn watchdog(&self) -> &W {
        &self.watchdog
    }

    pub fn watchdog_mut(&mut self) -> &mut W {
        &mut self.watchdog
    }

    pub fn into_parts(self) -> (F, W) {
        (self.flash, self.watchdog)
    }
}

impl<F: FlashTarget, W: WatchdogFeed> FlashTarget for WatchdogFlash<F, W> {
    fn page_size(&self) -> usize {
        self.flash.page_size()
    }

    fn app_base(&self) -> u32 {
        self.flash.app_base()
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        self.watchdog.feed();
        let res = self.flash.erase_page(addr);
        self.watchdog.feed();
        res
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        self.watchdog.feed();
        let res = self.flash.write_page_bytes(addr, data);
        self.watchdog.feed();
        res
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        self.flash.read_bytes(addr, out)
    }

    fn read_uid(&mut self, out: &mut [u8; 12]) {
        self.flash.read_uid(out)
    }

    unsafe fn boot_jump(app_entry: u32) -> ! {
        unsafe { F::boot_jump(app_entry) }
    }

    fn write_commit_record(&mut self, record: &[u8]) -> Result<(), CoreError> {
        self.watchdog.feed();
        let res = self.flash.write_commit_record(record);
        self.watchdog.feed();
        res
    }
}

pub struct PagedWriter<F, const PAGE: usize> {
    flash: F,
    config: BootConfig,
    cursor: u32,
    page: [u8; PAGE],
    page_len: usize,
    page_base: u32,
    pages_written: u32,
    completed: bool,
    watchdog_hook: Option<fn()>,
}

impl<F: FlashTarget, const PAGE: usize> PagedWriter<F, PAGE> {
    pub fn new(flash: F, config: BootConfig) -> Self {
        let app_base = config.app_base;
        Self {
            flash,
            config,
            cursor: app_base,
            page: [0u8; PAGE],
            page_len: 0,
            page_base: app_base,
            pages_written: 0,
            completed: false,
            watchdog_hook: None,
        }
    }

    pub fn set_watchdog_hook(&mut self, hook: fn()) {
        self.watchdog_hook = Some(hook);
    }

    fn feed_watchdog(&self) {
        if let Some(hook) = self.watchdog_hook {
            hook();
        }
    }

    pub fn config(&self) -> &BootConfig {
        &self.config
    }

    pub fn pages_written(&self) -> u32 {
        self.pages_written
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn set_completed(&mut self) {
        self.completed = true;
    }

    pub fn write_block(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        self.feed_watchdog();

        // Strict partition bounds check: [APP_BASE, APP_END)
        if addr < self.config.app_base {
            return Err(CoreError::InvalidAddress);
        }
        if self.config.app_end != 0 {
            if addr >= self.config.app_end {
                return Err(CoreError::InvalidAddress);
            }
            if addr.saturating_add(data.len() as u32) > self.config.app_end {
                return Err(CoreError::InvalidAddress);
            }
        }

        if addr != self.cursor {
            return Err(CoreError::InvalidAddress);
        }
        if PAGE == 0 || self.flash.page_size() != PAGE {
            return Err(CoreError::Flash);
        }

        let mut offset = 0;
        while offset < data.len() {
            if self.page_len == 0 {
                self.feed_watchdog();
                self.flash.erase_page(self.page_base)?;
            }
            let remaining = PAGE - self.page_len;
            let chunk = (data.len() - offset).min(remaining);
            self.page[self.page_len..self.page_len + chunk]
                .copy_from_slice(&data[offset..offset + chunk]);
            self.page_len += chunk;
            offset += chunk;

            if self.page_len == PAGE {
                self.flush_page()?;
            }
        }

        self.cursor += data.len() as u32;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<u32, CoreError> {
        self.feed_watchdog();
        if self.page_len > 0 {
            // Pad the partial page with erased flash state.
            for byte in &mut self.page[self.page_len..PAGE] {
                *byte = 0xff;
            }
            self.flush_page()?;
        }
        self.feed_watchdog();
        Ok(self.pages_written)
    }

    pub fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        self.flash.read_bytes(addr, out)
    }

    /// Persist an opaque commit-state record via the underlying flash target.
    pub fn write_commit_record(&mut self, record: &[u8]) -> Result<(), CoreError> {
        self.feed_watchdog();
        self.flash.write_commit_record(record)
    }

    fn flush_page(&mut self) -> Result<(), CoreError> {
        self.feed_watchdog();
        self.flash.write_page_bytes(self.page_base, &self.page)?;
        let mut check = [0u8; 4];
        self.flash.read_bytes(self.page_base, &mut check)?;
        if check != self.page[..4] {
            return Err(CoreError::Flash);
        }
        self.pages_written += 1;
        self.page_base += PAGE as u32;
        self.page_len = 0;
        self.feed_watchdog();
        Ok(())
    }

    pub fn into_flash(self) -> F {
        self.flash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFlash {
        page_size: usize,
        app_base: u32,
        mem: [u8; 256],
    }

    impl MockFlash {
        fn new() -> Self {
            Self {
                page_size: 64,
                app_base: 0x1000,
                mem: [0xFF; 256],
            }
        }
    }

    impl FlashTarget for MockFlash {
        fn page_size(&self) -> usize {
            self.page_size
        }
        fn app_base(&self) -> u32 {
            self.app_base
        }
        fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
            let offset = (addr - self.app_base) as usize;
            self.mem[offset..offset + self.page_size].fill(0xFF);
            Ok(())
        }
        fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
            let offset = (addr - self.app_base) as usize;
            self.mem[offset..offset + data.len()].copy_from_slice(data);
            Ok(())
        }
        fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
            let offset = (addr - self.app_base) as usize;
            out.copy_from_slice(&self.mem[offset..offset + out.len()]);
            Ok(())
        }
        fn read_uid(&mut self, out: &mut [u8; 12]) {
            out.fill(0);
        }
        unsafe fn boot_jump(_app_entry: u32) -> ! {
            unreachable!()
        }
    }

    static mut WATCHDOG_CALL_COUNT: usize = 0;
    fn test_feed_hook() {
        unsafe {
            WATCHDOG_CALL_COUNT += 1;
        }
    }

    #[test]
    fn test_watchdog_flash_wrapper() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        let flash = MockFlash::new();
        static FEEDS: AtomicUsize = AtomicUsize::new(0);
        FEEDS.store(0, Ordering::SeqCst);
        let feed_fn = || {
            FEEDS.fetch_add(1, Ordering::SeqCst);
        };

        let mut wd_flash = WatchdogFlash::new(flash, feed_fn);
        wd_flash.erase_page(0x1000).unwrap();
        assert_eq!(FEEDS.load(Ordering::SeqCst), 2);

        wd_flash.write_page_bytes(0x1000, &[1, 2, 3, 4]).unwrap();
        assert_eq!(FEEDS.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn test_paged_writer_watchdog_hook() {
        unsafe {
            WATCHDOG_CALL_COUNT = 0;
        }

        let flash = MockFlash::new();
        let cfg = BootConfig {
            protocol_version: 0x100,
            app_base: 0x1000,
            app_end: 0x1100,
            block_size: 64,
            mcu_type: b"test",
            software_version: b"1.0",
            uid: [0; 12],
        };

        let mut writer = PagedWriter::<_, 64>::new(flash, cfg);
        writer.set_watchdog_hook(test_feed_hook);

        let block = [0x55; 64];
        writer.write_block(0x1000, &block).unwrap();
        writer.finish().unwrap();

        unsafe {
            assert!(WATCHDOG_CALL_COUNT > 0);
        }
    }

    #[test]
    fn test_strict_partition_bounds_checking() {
        let flash = MockFlash::new();
        let cfg = BootConfig {
            protocol_version: 0x100,
            app_base: 0x1000,
            app_end: 0x1080, // Allow 128 bytes (0x1000..0x1080)
            block_size: 64,
            mcu_type: b"test",
            software_version: b"1.0",
            uid: [0; 12],
        };

        let mut writer = PagedWriter::<_, 64>::new(flash, cfg);

        // Address below app_base rejected
        let block = [0xAA; 64];
        assert_eq!(
            writer.write_block(0x0FFF, &block),
            Err(CoreError::InvalidAddress)
        );

        // Valid block 1
        writer.write_block(0x1000, &block).unwrap();

        // Valid block 2 (cursor now at 0x1040, ends at 0x1080)
        writer.write_block(0x1040, &block).unwrap();

        // Block at or above app_end rejected (protects NVM at >= 0x1080)
        assert_eq!(
            writer.write_block(0x1080, &block),
            Err(CoreError::InvalidAddress)
        );
    }
}
