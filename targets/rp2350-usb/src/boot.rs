//! Pure, host-testable boot-decision helpers.
//!
//! Flash layout contract (XIP addresses):
//!   bootloader: 0x10000000 .. 0x10008000 (32 KB)
//!   app slot:   0x10008000 .. 0x10140000
//!   SYSTEM_FS:  0x10140000 .. 0x10240000 (never touched by this bootloader)

/// XIP base of the bootloader itself. Its vector table supplies the initial SP
/// used to derive the cookie mailbox address.
pub const BOOTLOADER_XIP: u32 = 0x1000_0000;

/// XIP base of the single application slot.
pub const APP_BASE: u32 = 0x1000_8000;

/// End (exclusive) of the application slot. No write at or above this address.
pub const APP_END: u32 = 0x1014_0000;

/// Size of the application slot.
pub const APP_SIZE: u32 = APP_END - APP_BASE;

/// Lowest valid initial stack pointer of an application image.
pub const RAM_BASE: u32 = 0x2000_0000;

/// Highest valid initial stack pointer of an application image (520 KB SRAM).
pub const RAM_END: u32 = 0x2008_2000;

/// Highest valid reset PC (app base + 2 MB, per the layout contract).
pub const PC_END: u32 = APP_BASE + 2 * 1024 * 1024;

/// Cookie value that requests indefinite recovery mode.
pub const COOKIE: u64 = bareboot_protocol::REQUEST_BOOTLOADER;

/// Return true when `value` is the bareboot "stay in bootloader" cookie.
pub fn cookie_present(value: u64) -> bool {
    value == COOKIE
}

/// Validate an application vector table: SP in SRAM, PC in the app slot.
pub fn valid_app_vector(sp: u32, pc: u32) -> bool {
    (sp >= RAM_BASE && sp <= RAM_END) && (pc >= APP_BASE && pc < PC_END)
}

/// Decide whether recovery mode must be entered.
///
/// Recovery is required when the host placed the cookie, or when the app
/// vector table is missing/corrupt.
pub fn should_enter_recovery(cookie: u64, sp: u32, pc: u32) -> bool {
    cookie_present(cookie) || !valid_app_vector(sp, pc)
}

/// Derive the SRAM cookie mailbox address from the bootloader's initial SP.
///
/// Returns `None` when the value is not a plausible 8-byte-aligned SRAM
/// address, so callers never dereference a garbage pointer.
pub fn mailbox_from_initial_sp(initial_sp: u32) -> Option<usize> {
    if initial_sp >= RAM_BASE && initial_sp < RAM_END && (initial_sp & 7) == 0 {
        Some(initial_sp as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_recognised() {
        assert!(cookie_present(COOKIE));
        assert!(cookie_present(0x5984_E3FA_6CA1_589B));
        assert!(!cookie_present(0));
        assert!(!cookie_present(0x5984_E3FA_6CA1_589A));
    }

    #[test]
    fn app_vector_valid_bounds() {
        assert!(valid_app_vector(0x2000_0000, APP_BASE));
        assert!(valid_app_vector(0x2000_4000, 0x1001_0000));
        assert!(valid_app_vector(RAM_END, PC_END - 1));
    }

    #[test]
    fn app_vector_rejects_bad_sp() {
        assert!(!valid_app_vector(RAM_BASE - 1, APP_BASE));
        assert!(!valid_app_vector(RAM_END + 1, APP_BASE));
        assert!(!valid_app_vector(0x1000_8000, APP_BASE));
    }

    #[test]
    fn app_vector_rejects_bad_pc() {
        assert!(!valid_app_vector(0x2000_4000, APP_BASE - 1));
        assert!(!valid_app_vector(0x2000_4000, PC_END));
        assert!(!valid_app_vector(0x2000_4000, 0xFFFF_FFFF));
        assert!(!valid_app_vector(0x2000_4000, 0));
    }

    #[test]
    fn recovery_decision_matrix() {
        let sp = 0x2000_4000;
        let pc = APP_BASE;
        assert!(!should_enter_recovery(0, sp, pc));
        assert!(should_enter_recovery(COOKIE, sp, pc));
        assert!(should_enter_recovery(0, 0, 0));
        assert!(should_enter_recovery(0, RAM_BASE - 1, pc));
        assert!(should_enter_recovery(0, sp, PC_END));
    }

    #[test]
    fn mailbox_derivation() {
        assert_eq!(mailbox_from_initial_sp(0x2000_4000), Some(0x2000_4000));
        assert_eq!(mailbox_from_initial_sp(0x2000_0008), Some(0x2000_0008));
        assert_eq!(mailbox_from_initial_sp(0x2000_0004), None);
        assert_eq!(mailbox_from_initial_sp(0x1FFF_FFFF), None);
        assert_eq!(mailbox_from_initial_sp(RAM_END), None);
        assert_eq!(mailbox_from_initial_sp(0x1000_0000), None);
    }

    #[test]
    fn layout_constants() {
        assert_eq!(BOOTLOADER_XIP, 0x1000_0000);
        assert_eq!(APP_BASE, 0x1000_8000);
        assert_eq!(APP_END, 0x1014_0000);
        assert_eq!(APP_SIZE, 0x13_8000);
        assert_eq!(PC_END, 0x1020_8000);
        assert_eq!(RAM_END, 0x2008_2000);
    }
}
