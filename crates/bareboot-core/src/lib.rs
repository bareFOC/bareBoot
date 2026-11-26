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

    /// Persist an opaque commit-state record in target-owned flash.
    /// Defaults to unsupported for targets that do not implement commit state.
    fn write_commit_record(&mut self, _record: &[u8]) -> Result<(), CoreError> {
        Err(CoreError::Flash)
    }

    /// Hook invoked once before the first application page erase/program.
    fn on_update_start(&mut self) -> Result<(), CoreError> {
        Ok(())
    }

    /// Hook invoked on `CMD_COMPLETE` with exact programmed byte length and IEEE 802.3 CRC32.
    fn on_update_complete(&mut self, _app_len: u32, _app_crc32: u32) -> Result<(), CoreError> {
        Ok(())
    }
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

/// Inspect the SRAM mailbox at `mailbox_addr`.
/// If it matches `bareboot_protocol::REQUEST_BOOTLOADER`, clears the mailbox and returns `true`.
pub fn check_and_clear_bootloader_request(mailbox_addr: usize) -> bool {
    let ptr = mailbox_addr as *mut u64;
    unsafe {
        if core::ptr::read_volatile(ptr) == bareboot_protocol::REQUEST_BOOTLOADER {
            core::ptr::write_volatile(ptr, 0);
            true
        } else {
            false
        }
    }
}

/// Validate an ARM Cortex-M or generic application vector table:
/// - Initial Stack Pointer (SP) must be within plausible RAM boundaries and not blank (0xFFFFFFFF or 0).
/// - Reset Vector (PC) must have Thumb bit set (for Cortex-M), must not be blank,
///   and must reside within the application flash boundary `[app_base, flash_end)`.
/// - Entry opcode slice must contain at least 4 bytes and the 32-bit word at entry must not be
///   unprogrammed (`0xFFFFFFFF`) or zeroed (`0x00000000`).
pub fn validate_app_vector(
    sp: u32,
    pc: u32,
    ram_start: u32,
    ram_end: u32,
    app_base: u32,
    flash_end: u32,
    entry_opcode: &[u8],
) -> bool {
    if sp == 0 || sp == 0xFFFF_FFFF || pc == 0 || pc == 0xFFFF_FFFF {
        return false;
    }
    if sp < ram_start || sp > ram_end {
        return false;
    }
    if (pc & 1) == 0 {
        return false;
    }
    if pc < app_base || pc >= flash_end {
        return false;
    }
    if entry_opcode.len() < 4 {
        return false;
    }
    let entry = u32::from_le_bytes([
        entry_opcode[0],
        entry_opcode[1],
        entry_opcode[2],
        entry_opcode[3],
    ]);
    if entry == 0 || entry == 0xFFFF_FFFF {
        return false;
    }
    true
}

/// Failsafe decision logic: decides whether the device must remain parked in
/// bootloader mode indefinitely.
///
/// Returns `true` if:
/// - Mailbox token matches `REQUEST_BOOTLOADER` (handsfree request)
/// - Application vector table or entry opcode is blank/invalid (failsafe blank-vector parking)
pub fn should_park_in_bootloader(
    mailbox_request: bool,
    sp: u32,
    pc: u32,
    ram_start: u32,
    ram_end: u32,
    app_base: u32,
    flash_end: u32,
    entry_opcode: &[u8],
) -> bool {
    mailbox_request || !validate_app_vector(sp, pc, ram_start, ram_end, app_base, flash_end, entry_opcode)
}


pub use bareboot_protocol::{crc32, crc32_update};

pub const BOOT_LOOP_MAGIC: u32 = 0x4242_4C50; // "BBLP"
pub const BOOT_LOOP_MAX_ATTEMPTS: u32 = 3;

/// 16-byte CRC32-protected boot-loop counter in `.uninit` SRAM.
///
/// BareBoot increments `attempts` before jumping to the application. The
/// application clears `attempts = 0` once healthy execution is established.
/// Reaching `BOOT_LOOP_MAX_ATTEMPTS` (3) traps BareBoot in recovery mode.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootLoopGuard {
    pub magic: u32,
    pub attempts: u32,
    pub reserved: u32,
    pub crc32: u32,
}

impl BootLoopGuard {
    pub const SIZE: usize = 16;

    pub fn new(attempts: u32) -> Self {
        let mut guard = Self {
            magic: BOOT_LOOP_MAGIC,
            attempts,
            reserved: 0,
            crc32: 0,
        };
        guard.crc32 = guard.compute_crc32();
        guard
    }

    pub fn compute_crc32(&self) -> u32 {
        let m = self.magic.to_le_bytes();
        let a = self.attempts.to_le_bytes();
        let r = self.reserved.to_le_bytes();
        let buf = [
            m[0], m[1], m[2], m[3],
            a[0], a[1], a[2], a[3],
            r[0], r[1], r[2], r[3],
        ];
        crc32(&buf)
    }

    pub fn is_valid(&self) -> bool {
        self.magic == BOOT_LOOP_MAGIC && self.crc32 == self.compute_crc32()
    }

    pub fn read(addr: usize) -> Option<Self> {
        // SAFETY: Caller provides a valid 4-byte-aligned SRAM address.
        let guard = unsafe { core::ptr::read_volatile(addr as *const Self) };
        if guard.is_valid() {
            Some(guard)
        } else {
            None
        }
    }

    pub fn write(&self, addr: usize) {
        // SAFETY: Caller provides a valid 4-byte-aligned SRAM address.
        unsafe {
            core::ptr::write_volatile(addr as *mut Self, *self);
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn is_trapped(addr: usize) -> bool {
        Self::read(addr).map_or(false, |g| g.attempts >= BOOT_LOOP_MAX_ATTEMPTS)
    }

    pub fn record_boot_attempt(addr: usize) -> u32 {
        let next = Self::read(addr)
            .map_or(0, |g| g.attempts)
            .saturating_add(1);
        Self::new(next).write(addr);
        next
    }

    pub fn clear(addr: usize) {
        Self::new(0).write(addr);
    }
}

pub mod writer;
pub use writer::{PagedWriter, WatchdogFeed, WatchdogFlash};
pub mod safe_pins;
pub use safe_pins::{apply_safe_pins, PinDriver, PinMode, SafePin};
pub mod dispatch;
pub use dispatch::{MultiTransportArbiter, ArbitrationState, ArbitrationAction};
pub mod commit;
pub use commit::{
    CommitRecord, CommitState, MonotonicCommitJournal, COMMIT_MAGIC, STATE_UPDATE_IN_PROGRESS,
    STATE_VALID,
};
pub mod storage_adapter;
pub use storage_adapter::NorFlashAdapter;

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

    #[test]
    fn test_mailbox_request_check() {
        let mut mailbox = 0u64;
        let addr = (&raw mut mailbox) as usize;

        assert!(!check_and_clear_bootloader_request(addr));
        assert_eq!(mailbox, 0);

        mailbox = bareboot_protocol::REQUEST_BOOTLOADER;
        assert!(check_and_clear_bootloader_request(addr));
        assert_eq!(mailbox, 0);

        assert!(!check_and_clear_bootloader_request(addr));
    }

    #[test]
    fn test_blank_vector_and_failsafe_parking() {
        const RAM_START: u32 = 0x2000_0000;
        const RAM_END: u32 = 0x2000_8000;
        const APP_BASE: u32 = 0x0800_2000;
        const FLASH_END: u32 = 0x0802_0000;

        let valid_opcode = [0x80, 0xB5, 0x00, 0xAF]; // Valid push {r7, lr}, sub sp, #0
        let blank_opcode = [0xFF, 0xFF, 0xFF, 0xFF]; // Blank/erased flash
        let zero_opcode = [0x00, 0x00, 0x00, 0x00];  // Zeroed flash

        // Blank flash (unprogrammed sector 0xFFFFFFFF)
        assert!(!validate_app_vector(0xFFFF_FFFF, 0xFFFF_FFFF, RAM_START, RAM_END, APP_BASE, FLASH_END, &blank_opcode));
        assert!(should_park_in_bootloader(false, 0xFFFF_FFFF, 0xFFFF_FFFF, RAM_START, RAM_END, APP_BASE, FLASH_END, &blank_opcode));

        // Zeroed flash
        assert!(!validate_app_vector(0, 0, RAM_START, RAM_END, APP_BASE, FLASH_END, &zero_opcode));
        assert!(should_park_in_bootloader(false, 0, 0, RAM_START, RAM_END, APP_BASE, FLASH_END, &zero_opcode));

        // Valid vector: SP in RAM, PC in app flash with Thumb bit set, valid entry opcode
        let valid_sp = 0x2000_4000;
        let valid_pc = 0x0800_2101; // Thumb bit set (odd)
        assert!(validate_app_vector(valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));
        assert!(!should_park_in_bootloader(false, valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));

        // Torn flash condition: Valid SP and PC, but entry code is blank (0xFFFFFFFF)
        assert!(!validate_app_vector(valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &blank_opcode));
        assert!(should_park_in_bootloader(false, valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &blank_opcode));

        // Torn flash condition: Valid SP and PC, but entry code is zeroed (0x00000000)
        assert!(!validate_app_vector(valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &zero_opcode));
        assert!(should_park_in_bootloader(false, valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &zero_opcode));

        // Truncated opcode slice (< 4 bytes)
        assert!(!validate_app_vector(valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &[0x80, 0xB5]));

        // Valid vector with mailbox request: must park indefinitely
        assert!(should_park_in_bootloader(true, valid_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));

        // BootLoopGuard 3-strike trap and clear
        let mut guard_slot = [0u32; 4];
        let gaddr = guard_slot.as_mut_ptr() as usize;
        assert!(!BootLoopGuard::is_trapped(gaddr));
        assert_eq!(BootLoopGuard::record_boot_attempt(gaddr), 1);
        assert!(!BootLoopGuard::is_trapped(gaddr));
        assert_eq!(BootLoopGuard::record_boot_attempt(gaddr), 2);
        assert!(!BootLoopGuard::is_trapped(gaddr));
        assert_eq!(BootLoopGuard::record_boot_attempt(gaddr), 3);
        assert!(BootLoopGuard::is_trapped(gaddr));
        BootLoopGuard::clear(gaddr);
        assert!(!BootLoopGuard::is_trapped(gaddr));
        assert_eq!(BootLoopGuard::read(gaddr).unwrap().attempts, 0);

        // Corrupted BootLoopGuard CRC resets attempts safely
        unsafe {
            let crc_ptr = (gaddr + 12) as *mut u32;
            core::ptr::write_volatile(crc_ptr, core::ptr::read_volatile(crc_ptr) ^ 0xFF);
        }
        assert!(BootLoopGuard::read(gaddr).is_none());
        assert!(!BootLoopGuard::is_trapped(gaddr));
        assert_eq!(BootLoopGuard::record_boot_attempt(gaddr), 1);

        // Incremental CRC32 parity with bareboot_protocol::crc32
        let sample = b"123456789";
        let state = crc32_update(0xFFFF_FFFF, &sample[..4]);
        let state = crc32_update(state, &sample[4..]);
        assert_eq!(!state, crc32(sample));
        assert_eq!(!state, 0xCBF4_3926);

        // Invalid Thumb bit (even PC)
        let even_pc = 0x0800_2100;
        assert!(!validate_app_vector(valid_sp, even_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));
        assert!(should_park_in_bootloader(false, valid_sp, even_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));

        // Out of bounds PC (pointing to bootloader region < APP_BASE)
        let bl_pc = 0x0800_0101;
        assert!(!validate_app_vector(valid_sp, bl_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));
        assert!(should_park_in_bootloader(false, valid_sp, bl_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));

        // Out of bounds SP (pointing to flash)
        let bad_sp = 0x0800_3000;
        assert!(!validate_app_vector(bad_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));
        assert!(should_park_in_bootloader(false, bad_sp, valid_pc, RAM_START, RAM_END, APP_BASE, FLASH_END, &valid_opcode));
    }
}

#[derive(Clone, Copy)]
pub struct BootConfig {
    pub protocol_version: u32,
    pub app_base: u32,
    pub app_end: u32,
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
            app_end: 0x2000,
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
            app_end: 0x2000,
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
