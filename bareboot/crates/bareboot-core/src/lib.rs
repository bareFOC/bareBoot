#![no_std]

use bareboot_protocol::commands;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    InvalidAddress,
    Flash,
    Transport,
    Timeout,
    BufferTooSmall,
}

pub trait FlashTarget {
    fn page_size(&self) -> usize;
    fn app_base(&self) -> u32;
    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError>;
    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError>;
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError>;
    fn read_uid(&mut self, out: &mut [u8; 12]);
    unsafe fn boot_jump(app_entry: u32) -> !;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Can,
    I2c,
    Uart,
    Usb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveInterface {
    Undecided,
    Selected(TransportKind),
    Locked,
}

pub struct Arbiter {
    pub boot_window_ms: u32,
    pub elapsed_ms: u32,
    pub active: ActiveInterface,
}

impl Arbiter {
    pub fn new(boot_window_ms: u32) -> Self {
        Self {
            boot_window_ms,
            elapsed_ms: 0,
            active: ActiveInterface::Undecided,
        }
    }

    pub fn select_if_undecided(&mut self, kind: TransportKind) -> bool {
        if self.active == ActiveInterface::Undecided && self.elapsed_ms < self.boot_window_ms {
            self.active = ActiveInterface::Selected(kind);
            true
        } else {
            false
        }
    }

    pub fn expire(&mut self) {
        if self.active == ActiveInterface::Undecided {
            self.active = ActiveInterface::Locked;
        }
    }

    pub fn is_selected(&self, kind: TransportKind) -> bool {
        self.active == ActiveInterface::Selected(kind)
    }
}

pub fn connect_command() -> u8 {
    commands::CONNECT
}


pub mod writer;
pub use writer::PagedWriter;
pub mod dispatch;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbiter_selects_first_transport() {
        let mut a = Arbiter::new(300);
        assert!(a.select_if_undecided(TransportKind::Can));
        assert!(!a.select_if_undecided(TransportKind::I2c));
        assert!(a.is_selected(TransportKind::Can));
    }

    #[test]
    fn arbiter_expires_undecided() {
        let mut a = Arbiter::new(300);
        a.expire();
        assert_eq!(a.active, ActiveInterface::Locked);
    }
}

#[derive(Clone, Copy)]
pub struct BootConfig {
    pub protocol_version: u32,
    pub app_base: u32,
    pub block_size: usize,
    pub mcu_type: &'static [u8],
    pub software_version: &'static [u8],
    pub uid: [u8; 12],
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    use super::writer::PagedWriter;
    use super::dispatch::dispatch_one;

    struct FakeFlash {
        page_size: usize,
        app_base: u32,
        uid: [u8; 12],
        memory: [u8; 512],
    }

    impl FakeFlash {
        fn new() -> Self {
            Self {
                page_size: 64,
                app_base: 0x1000,
                uid: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
                memory: [0xff; 512],
            }
        }
    }

    impl FlashTarget for FakeFlash {
        fn page_size(&self) -> usize {
            self.page_size
        }

        fn app_base(&self) -> u32 {
            self.app_base
        }

        fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
            let start = (addr - self.app_base) as usize;
            for b in &mut self.memory[start..start + self.page_size] {
                *b = 0xff;
            }
            Ok(())
        }

        fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
            let start = (addr - self.app_base) as usize;
            self.memory[start..start + data.len()].copy_from_slice(data);
            Ok(())
        }

        fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
            let start = (addr - self.app_base) as usize;
            out.copy_from_slice(&self.memory[start..start + out.len()]);
            Ok(())
        }

        fn read_uid(&mut self, out: &mut [u8; 12]) {
            *out = self.uid;
        }

        unsafe fn boot_jump(_app_entry: u32) -> ! {
            loop {}
        }
    }

    #[test]
    fn paged_writer_flushes_pages() {
        let flash = FakeFlash::new();
        let cfg = BootConfig {
            protocol_version: 0x00010100,
            app_base: 0x1000,
            block_size: 64,
            mcu_type: b"ch32v203",
            software_version: b"dev",
            uid: [0; 12],
        };
        let mut writer = PagedWriter::<_, 64>::new(flash, cfg);
        let first = [0x55u8; 64];
        let second = [0xaa; 64];
        writer.write_block(0x1000, &first).unwrap();
        writer.write_block(0x1040, &second).unwrap();
        let pages = writer.finish().unwrap();
        assert_eq!(pages, 2);
    }

    #[test]
    fn dispatch_connect_and_write() {
        let flash = FakeFlash::new();
        let cfg = BootConfig {
            protocol_version: 0x00010100,
            app_base: 0x1000,
            block_size: 64,
            mcu_type: b"ch32",
            software_version: b"dev",
            uid: [9; 12],
        };
        let mut writer = PagedWriter::<_, 64>::new(flash, cfg);

        // Write block
        let block = [0x77u8; 64];
        let mut payload = [0u8; 68];
        payload[0..4].copy_from_slice(&0x1000u32.to_le_bytes());
        payload[4..].copy_from_slice(&block);
        let mut frame = [0u8; 192];
        let flen = bareboot_protocol::encode_frame(bareboot_protocol::commands::RX_BLOCK, &payload, &mut frame).unwrap();
        let mut out = [0u8; 192];
        let rlen = dispatch_one(&mut writer, &frame[..flen], &mut out).unwrap();
        assert!(rlen > 0);
        let stored = writer.read_bytes(0x1000, &mut [0u8;64]).is_ok();
        assert!(stored);
    }
}
