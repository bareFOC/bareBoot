#![no_std]
#![no_main]

use bareboot_core::dispatch::dispatch_one;
use bareboot_core::writer::{PagedWriter, WatchdogFlash};
use bareboot_core::{
    apply_safe_pins, check_and_clear_bootloader_request, crc32_update, validate_app_vector,
    BootConfig, BootLoopGuard, CommitState, CoreError, FlashTarget, MonotonicCommitJournal,
    PinMode,
};
use bareboot_protocol::{ccc, responses, SoftResetMailbox, REQUEST_BOOTLOADER};
use core::ptr::{read_volatile, write_volatile};
use cortex_m_rt::entry;

include!(concat!(env!("OUT_DIR"), "/safe_pins_generated.rs"));

const APP_BASE: u32 = 0x0800_2000;
const APP_END: u32 = 0x0801_A000; // Sectors 1..12 (96 KiB)
const COMMIT_BASE: u32 = 0x0801_A000; // Sector 13 (8 KiB)
const COMMIT_SIZE: usize = 8192;
const PAGE_SIZE: usize = 8192;
const BLOCK_SIZE: usize = 64;

const RAM_START: u32 = 0x2000_0000;
const RAM_END: u32 = 0x2000_8000;
const BOOT_LOOP_GUARD_ADDR: usize = 0x2000_7FD0;
const MAILBOX_ADDR: u32 = 0x2000_7FE0;
const LEGACY_COOKIE_ADDR: usize = 0x2000_7FF8;

const BOOT_WINDOW_MS: u32 = 1500;
const BOOTLOADER_IDLE_TIMEOUT_MS: u32 = 30_000;
const I3C2_IDLE_RESET_MS: u32 = 2_000;
const SYSCLK_HZ: u32 = 32_000_000; // STM32H503 reset HSIDIV=/2 (32 MHz)

const SYST_CSR: *mut u32 = 0xE000_E010 as *mut u32;
const SYST_RVR: *mut u32 = 0xE000_E014 as *mut u32;
const SYST_CVR: *mut u32 = 0xE000_E018 as *mut u32;
const SCB_AIRCR: *mut u32 = 0xE000_ED0C as *mut u32;

// RCC (Non-Secure)
const RCC_BASE: u32 = 0x4402_0C00;
const RCC_APB3RSTR: *mut u32 = (RCC_BASE + 0x080) as *mut u32;
const RCC_AHB1ENR: *mut u32 = (RCC_BASE + 0x088) as *mut u32;
const RCC_AHB2ENR: *mut u32 = (RCC_BASE + 0x08C) as *mut u32;
const RCC_APB3ENR: *mut u32 = (RCC_BASE + 0x0A8) as *mut u32;
const RCC_RSR: *mut u32 = (RCC_BASE + 0x0F4) as *mut u32;
const APB3RSTR_I3C2RST: u32 = 1 << 9;
const RSR_RMVF: u32 = 1 << 23;
const RSR_SFTRSTF: u32 = 1 << 28;
const RSR_IWDGRSTF: u32 = 1 << 29;
const RSR_WWDGRSTF: u32 = 1 << 30;

// GPIOA / GPIOB (Non-Secure)
const GPIOA_BASE: u32 = 0x4202_0000;
const GPIOA_MODER: *mut u32 = GPIOA_BASE as *mut u32;
const GPIOA_BSRR: *mut u32 = (GPIOA_BASE + 0x18) as *mut u32;

const GPIOB_BASE: u32 = 0x4202_0400;
const GPIOB_MODER: *mut u32 = GPIOB_BASE as *mut u32;
const GPIOB_OTYPER: *mut u32 = (GPIOB_BASE + 0x04) as *mut u32;
const GPIOB_OSPEEDR: *mut u32 = (GPIOB_BASE + 0x08) as *mut u32;
const GPIOB_PUPDR: *mut u32 = (GPIOB_BASE + 0x0C) as *mut u32;
const GPIOB_IDR: *const u32 = (GPIOB_BASE + 0x10) as *const u32;
const GPIOB_AFRL: *mut u32 = (GPIOB_BASE + 0x20) as *mut u32;
const PB4_SDA_MASK: u32 = 1 << 4;

// FLASH (Non-Secure)
const FLASH_BASE: u32 = 0x4002_2000;
const FLASH_NSKEYR: *mut u32 = (FLASH_BASE + 0x04) as *mut u32;
const FLASH_OPTKEYR: *mut u32 = (FLASH_BASE + 0x0C) as *mut u32;
const FLASH_OPTCR: *mut u32 = (FLASH_BASE + 0x1C) as *mut u32;
const FLASH_NSSR: *mut u32 = (FLASH_BASE + 0x20) as *mut u32;
const FLASH_NSCR: *mut u32 = (FLASH_BASE + 0x28) as *mut u32;
const FLASH_NSCCR: *mut u32 = (FLASH_BASE + 0x30) as *mut u32;
const FLASH_OPTSR_CUR: *const u32 = (FLASH_BASE + 0x50) as *const u32;
const FLASH_OPTSR_PRG: *mut u32 = (FLASH_BASE + 0x54) as *mut u32;
const FLASH_WRPSGN1R_CUR: *const u32 = (FLASH_BASE + 0xE8) as *const u32;
const FLASH_WRPSGN1R_PRG: *mut u32 = (FLASH_BASE + 0xEC) as *mut u32;

// IWDG (Independent Watchdog)
const IWDG_BASE: u32 = 0x4000_3000;
const IWDG_KR: *mut u32 = IWDG_BASE as *mut u32;
const IWDG_PR: *mut u32 = (IWDG_BASE + 0x04) as *mut u32;
const IWDG_RLR: *mut u32 = (IWDG_BASE + 0x08) as *mut u32;
const IWDG_SR: *const u32 = (IWDG_BASE + 0x0C) as *const u32;

// I3C2 (APB3 Non-Secure, PB3=SCL AF3, PB4=SDA AF3)
const I3C2_BASE: u32 = 0x4400_3000;
const I3C2_CFGR: *mut u32 = (I3C2_BASE + 0x04) as *mut u32;
const I3C2_RDR: *const u32 = (I3C2_BASE + 0x10) as *const u32;
const I3C2_TDR: *mut u32 = (I3C2_BASE + 0x18) as *mut u32;
const I3C2_TGTTDR: *mut u32 = (I3C2_BASE + 0x24) as *mut u32;
const I3C2_SER: *const u32 = (I3C2_BASE + 0x34) as *const u32;
const I3C2_RMR: *const u32 = (I3C2_BASE + 0x40) as *const u32;
const I3C2_EVR: *const u32 = (I3C2_BASE + 0x50) as *const u32;
const I3C2_CEVR: *mut u32 = (I3C2_BASE + 0x58) as *mut u32;
const I3C2_DEVR0: *mut u32 = (I3C2_BASE + 0x60) as *mut u32;
const I3C2_MAXRLR: *mut u32 = (I3C2_BASE + 0x90) as *mut u32;
const I3C2_MAXWLR: *mut u32 = (I3C2_BASE + 0x94) as *mut u32;
const I3C2_TIMINGR0: *mut u32 = (I3C2_BASE + 0xA0) as *mut u32;
const I3C2_TIMINGR1: *mut u32 = (I3C2_BASE + 0xA4) as *mut u32;
const I3C2_TIMINGR2: *mut u32 = (I3C2_BASE + 0xA8) as *mut u32;
const I3C2_BCR: *mut u32 = (I3C2_BASE + 0xC0) as *mut u32;
const I3C2_DCR: *mut u32 = (I3C2_BASE + 0xC4) as *mut u32;
const I3C2_EPIDR: *mut u32 = (I3C2_BASE + 0xD4) as *mut u32;

const CFGR_RXFLUSH: u32 = 1 << 9;
const CFGR_TXFLUSH: u32 = 1 << 13;

// I3C_EVR bits (verified against STM32H5 i3c_v1.rs)
const EVR_TXFEF: u32 = 1 << 1;
const EVR_TXFNFF: u32 = 1 << 4;
const EVR_RXFNEF: u32 = 1 << 5;
const EVR_FCF: u32 = 1 << 9;
const EVR_RXTGTENDF: u32 = 1 << 10;
const EVR_ERRF: u32 = 1 << 11;
const EVR_DAUPDF: u32 = 1 << 24;
const EVR_RSTF: u32 = 1 << 27;

const UID_BASE: u32 = 0x08FF_F800;

#[unsafe(link_section = ".vector_table.interrupts")]
#[unsafe(no_mangle)]
pub static __INTERRUPTS: [u32; 135] = [0; 135];

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    unsafe {
        write_volatile(0xE000_ED0C as *mut u32, (0x05FA << 16) | (1 << 2));
    }
    loop {
        cortex_m::asm::nop();
    }
}

fn iwdg_start() {
    unsafe {
        write_volatile(IWDG_KR, 0xCCCC);
        write_volatile(IWDG_KR, 0x5555);
        write_volatile(IWDG_PR, 3); // /32 -> ~1 kHz
        write_volatile(IWDG_RLR, 2000); // ~2.0 s timeout
        while read_volatile(IWDG_SR) & 0x03 != 0 {}
        write_volatile(IWDG_KR, 0xAAAA);
    }
}

#[inline(always)]
fn iwdg_feed() {
    unsafe {
        write_volatile(IWDG_KR, 0xAAAA);
    }
}

unsafe fn flash_unlock() {
    unsafe {
        if read_volatile(FLASH_NSCR) & 1 != 0 {
            write_volatile(FLASH_NSKEYR, 0x4567_0123);
            write_volatile(FLASH_NSKEYR, 0xCDEF_89AB);
        }
        write_volatile(FLASH_NSCCR, 0x00FE_0000);
    }
}

unsafe fn flash_lock() {
    unsafe {
        write_volatile(FLASH_NSCR, read_volatile(FLASH_NSCR) | 1);
    }
}

unsafe fn flash_wait_busy() -> Result<(), CoreError> {
    unsafe {
        while read_volatile(FLASH_NSSR) & 1 != 0 {
            iwdg_feed();
        }
        let sr = read_volatile(FLASH_NSSR);
        if sr & 0x00EE_0000 != 0 {
            write_volatile(FLASH_NSCCR, 0x00FE_0000);
            return Err(CoreError::Flash);
        }
        Ok(())
    }
}

unsafe fn flash_erase_sector(addr: u32) -> Result<(), CoreError> {
    let rel = addr - 0x0800_0000;
    let bank_bit: u32 = if rel >= 0x1_0000 { 1 << 31 } else { 0 };
    let sector = ((rel & 0xFFFF) / PAGE_SIZE as u32) & 0x7F;
    unsafe {
        flash_unlock();
        flash_wait_busy()?;
        write_volatile(FLASH_NSCR, bank_bit | (1 << 2) | (1 << 5) | (sector << 6));
        let res = flash_wait_busy();
        write_volatile(FLASH_NSCR, 0);
        flash_lock();
        res
    }
}

unsafe fn flash_write_quads(addr: u32, data: &[u8]) -> Result<(), CoreError> {
    if data.len() % 16 != 0 {
        return Err(CoreError::InvalidAddress);
    }
    unsafe {
        flash_unlock();
        flash_wait_busy()?;
        write_volatile(FLASH_NSCR, 1 << 1); // PG
        let mut dst = addr as *mut u32;
        for chunk in data.chunks_exact(16) {
            let src = chunk.as_ptr() as *const u32;
            write_volatile(dst, src.read_unaligned());
            write_volatile(dst.add(1), src.add(1).read_unaligned());
            write_volatile(dst.add(2), src.add(2).read_unaligned());
            write_volatile(dst.add(3), src.add(3).read_unaligned());
            if let Err(e) = flash_wait_busy() {
                write_volatile(FLASH_NSCR, 0);
                flash_lock();
                return Err(e);
            }
            dst = dst.add(4);
        }
        write_volatile(FLASH_NSCR, 0);
        flash_lock();
    }
    Ok(())
}

unsafe fn flash_read_volatile(addr: u32, out: &mut [u8]) {
    unsafe {
        let src = addr as *const u8;
        for (i, b) in out.iter_mut().enumerate() {
            *b = read_volatile(src.add(i));
        }
    }
}

fn ensure_sector0_wrp_and_bor() {
    unsafe {
        let wrp_cur = read_volatile(FLASH_WRPSGN1R_CUR);
        let opt_cur = read_volatile(FLASH_OPTSR_CUR);
        if (wrp_cur & 0x01) == 0 && (opt_cur & 0x03) == 0x03 {
            return;
        }
        while read_volatile(FLASH_NSSR) & 1 != 0 {}
        write_volatile(FLASH_NSCCR, 0x00FE_0000);
        if read_volatile(FLASH_OPTCR) & 1 != 0 {
            write_volatile(FLASH_OPTKEYR, 0x0819_2A3B);
            write_volatile(FLASH_OPTKEYR, 0x4C5D_6E7F);
            while read_volatile(FLASH_OPTCR) & 1 != 0 {}
        }
        write_volatile(FLASH_WRPSGN1R_PRG, wrp_cur & !0x01); // Bit 0 = 0 protects Sector 0
        write_volatile(FLASH_OPTSR_PRG, (opt_cur & !0x03) | 0x03); // BOR Level 3 (~2.7 V)
        write_volatile(FLASH_OPTCR, read_volatile(FLASH_OPTCR) | (1 << 1)); // OPTSTRT
        while read_volatile(FLASH_OPTCR) & (1 << 1) != 0 {
            iwdg_feed();
        }
        write_volatile(FLASH_OPTCR, read_volatile(FLASH_OPTCR) | 1); // OPTLOCK
    }
}

/// Flash target strictly scoped to Sector 13 CommitStore (`0x0801_A000..0x0801_C000`).
struct CommitSectorFlash;

impl FlashTarget for CommitSectorFlash {
    fn page_size(&self) -> usize {
        COMMIT_SIZE
    }

    fn app_base(&self) -> u32 {
        COMMIT_BASE
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        if addr != COMMIT_BASE {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_erase_sector(addr) }
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        if addr < COMMIT_BASE || addr.saturating_add(data.len() as u32) > COMMIT_BASE + COMMIT_SIZE as u32 {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_write_quads(addr, data) }
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        if addr < COMMIT_BASE || addr.saturating_add(out.len() as u32) > COMMIT_BASE + COMMIT_SIZE as u32 {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_read_volatile(addr, out) };
        Ok(())
    }

    fn read_uid(&mut self, _out: &mut [u8; 12]) {}

    unsafe fn boot_jump(_app_entry: u32) -> ! {
        loop {}
    }
}

/// Application flash target strictly scoped to Sectors 1..12 (`0x0800_2000..0x0801_A000`).
struct Stm32H5Flash {
    journal: MonotonicCommitJournal<COMMIT_SIZE, 16>,
}

impl Stm32H5Flash {
    fn new(journal: MonotonicCommitJournal<COMMIT_SIZE, 16>) -> Self {
        Self { journal }
    }
}

impl FlashTarget for Stm32H5Flash {
    fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    fn app_base(&self) -> u32 {
        APP_BASE
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        if addr < APP_BASE || addr >= APP_END || (addr & (PAGE_SIZE as u32 - 1)) != 0 {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_erase_sector(addr) }
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        if addr < APP_BASE || addr.saturating_add(data.len() as u32) > APP_END {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_write_quads(addr, data) }
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        if addr < APP_BASE || addr.saturating_add(out.len() as u32) > APP_END {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_read_volatile(addr, out) };
        Ok(())
    }

    fn read_uid(&mut self, out: &mut [u8; 12]) {
        unsafe {
            let src = UID_BASE as *const u32;
            for i in 0..3 {
                let w = read_volatile(src.add(i)).to_le_bytes();
                out[i * 4..i * 4 + 4].copy_from_slice(&w);
            }
        }
    }

    unsafe fn boot_jump(app_entry: u32) -> ! {
        if !app_vector_is_valid() {
            unsafe {
                write_volatile(SCB_AIRCR, 0x05FA_0004);
            }
            loop {
                core::hint::spin_loop();
            }
        }
        unsafe {
            // Disable SysTick
            write_volatile(SYST_CSR, 0);
            write_volatile(SYST_CVR, 0);
            // Reset I3C2 via RCC_APB3RSTR and return PB3/PB4 to analog/input
            reset_i3c2_bus();
            write_volatile(GPIOB_MODER, read_volatile(GPIOB_MODER) | 0x03C0);
            // Feed IWDG if active before handing off
            iwdg_feed();
            let sp = read_volatile(app_entry as *const u32);
            let reset_handler = read_volatile((app_entry + 4) as *const u32);
            let vtor = 0xE000_ED08 as *mut u32;
            write_volatile(vtor, app_entry);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();
            cortex_m::asm::bootstrap(sp as *const u32, reset_handler as *const u32);
        }
    }

    fn on_update_start(&mut self) -> Result<(), CoreError> {
        self.journal.mark_in_progress(&mut CommitSectorFlash)
    }

    fn on_update_complete(&mut self, app_len: u32, app_crc32: u32) -> Result<(), CoreError> {
        self.journal
            .mark_valid(&mut CommitSectorFlash, app_len, app_crc32)
    }
}

fn read_mailbox_request(fallback_addr: u8) -> (bool, u8) {
    let stack_cookie = check_and_clear_bootloader_request(BOOT_LOOP_GUARD_ADDR);
    let mb_ptr = MAILBOX_ADDR as *const SoftResetMailbox;
    let mb = unsafe { read_volatile(mb_ptr) };
    if mb.is_valid() {
        let req = mb.magic == REQUEST_BOOTLOADER;
        let addr = if (0x08..=0x77).contains(&mb.dynamic_node_id) {
            mb.dynamic_node_id
        } else {
            fallback_addr
        };
        unsafe {
            write_volatile(MAILBOX_ADDR as *mut u64, 0);
        }
        return (req || stack_cookie, addr);
    }
    let legacy = check_and_clear_bootloader_request(LEGACY_COOKIE_ADDR);
    (legacy || stack_cookie, fallback_addr)
}

fn app_vector_is_valid() -> bool {
    unsafe {
        let sp = read_volatile(APP_BASE as *const u32);
        let pc = read_volatile((APP_BASE + 4) as *const u32);
        let entry_addr = pc & !1;
        if entry_addr < APP_BASE || entry_addr.saturating_add(4) > APP_END {
            return false;
        }
        let op = [
            read_volatile(entry_addr as *const u8),
            read_volatile((entry_addr + 1) as *const u8),
            read_volatile((entry_addr + 2) as *const u8),
            read_volatile((entry_addr + 3) as *const u8),
        ];
        validate_app_vector(sp, pc, RAM_START, RAM_END, APP_BASE, APP_END, &op)
    }
}

fn verify_app_crc32(app_len: u32, expected_crc: u32) -> bool {
    if app_len == 0 || app_len > (APP_END - APP_BASE) {
        return false;
    }
    let mut crc = 0xFFFF_FFFFu32;
    let mut offset = 0u32;
    while offset < app_len {
        iwdg_feed();
        let chunk = (app_len - offset).min(PAGE_SIZE as u32) as usize;
        let slice = unsafe { core::slice::from_raw_parts((APP_BASE + offset) as *const u8, chunk) };
        crc = crc32_update(crc, slice);
        offset += chunk as u32;
    }
    (!crc) == expected_crc
}

unsafe fn reset_i3c2_bus() {
    unsafe {
        write_volatile(I3C2_CFGR, 0);
        let rstr = read_volatile(RCC_APB3RSTR);
        write_volatile(RCC_APB3RSTR, rstr | APB3RSTR_I3C2RST);
        for _ in 0..8 {
            cortex_m::asm::nop();
        }
        write_volatile(RCC_APB3RSTR, rstr & !APB3RSTR_I3C2RST);
    }
}

fn init_clocks_and_pins() {
    unsafe {
        // Enable FLASH, GPIO clocks (A/B + any port in INITIAL_SAFE_PINS), I3C2 clock
        let mut gpio_clocks: u32 = 0x03; // GPIOA | GPIOB
        for sp in INITIAL_SAFE_PINS {
            if ('A'..='H').contains(&sp.port) {
                gpio_clocks |= 1u32 << ((sp.port as u32) - ('A' as u32));
            }
        }
        write_volatile(RCC_AHB1ENR, read_volatile(RCC_AHB1ENR) | (1 << 8) | (1 << 31));
        write_volatile(RCC_AHB2ENR, read_volatile(RCC_AHB2ENR) | gpio_clocks);
        write_volatile(RCC_APB3ENR, read_volatile(RCC_APB3ENR) | (1 << 9));
        let _ = read_volatile(RCC_APB3ENR);

        // Apply configured safe pin states immediately on reset before peripheral init
        apply_safe_pins(INITIAL_SAFE_PINS, &mut |port, pin, mode| {
            if !('A'..='H').contains(&port) || pin > 15 {
                return;
            }
            let gpio_base = GPIOA_BASE + ((port as u32) - ('A' as u32)) * 0x0400;
            let moder = gpio_base as *mut u32;
            let pupdr = (gpio_base + 0x0C) as *mut u32;
            let bsrr = (gpio_base + 0x18) as *mut u32;
            let shift = (pin as u32) * 2;
            let mask = 0x03u32 << shift;
            match mode {
                PinMode::OutputLow => {
                    write_volatile(bsrr, 1u32 << (pin as u32 + 16));
                    write_volatile(pupdr, read_volatile(pupdr) & !mask);
                    write_volatile(moder, (read_volatile(moder) & !mask) | (0x01 << shift));
                }
                PinMode::OutputHigh => {
                    write_volatile(bsrr, 1u32 << (pin as u32));
                    write_volatile(pupdr, read_volatile(pupdr) & !mask);
                    write_volatile(moder, (read_volatile(moder) & !mask) | (0x01 << shift));
                }
                PinMode::InputPullUp => {
                    write_volatile(moder, read_volatile(moder) & !mask);
                    write_volatile(pupdr, (read_volatile(pupdr) & !mask) | (0x01 << shift));
                }
                PinMode::InputPullDown => {
                    write_volatile(moder, read_volatile(moder) & !mask);
                    write_volatile(pupdr, (read_volatile(pupdr) & !mask) | (0x02 << shift));
                }
            }
        });

        // PA0 = LED_B (active LOW output)
        let moder_a = read_volatile(GPIOA_MODER);
        write_volatile(GPIOA_MODER, (moder_a & !0x03) | 0x01);
        write_volatile(GPIOA_BSRR, 1 << 16); // LED ON
    }
}

fn init_i3c2(static_addr: u8, uid_mix: u32) {
    unsafe {
        // PB3 (SCL) and PB4 (SDA) -> AF3 (I3C2), Very High Speed, Pull-Up, Push-Pull
        let moder_b = read_volatile(GPIOB_MODER);
        write_volatile(GPIOB_MODER, (moder_b & !0x03C0) | 0x0280);
        write_volatile(GPIOB_OTYPER, read_volatile(GPIOB_OTYPER) & !0x18);
        write_volatile(GPIOB_OSPEEDR, read_volatile(GPIOB_OSPEEDR) | 0x03C0);
        write_volatile(GPIOB_PUPDR, (read_volatile(GPIOB_PUPDR) & !0x03C0) | 0x0140);
        write_volatile(
            GPIOB_AFRL,
            (read_volatile(GPIOB_AFRL) & !0x000F_F000) | 0x0003_3000,
        );

        write_volatile(I3C2_CFGR, 0);
        write_volatile(I3C2_CFGR, (1 << 3) | (1 << 4)); // RSTPTRN | EXITPTRN
        write_volatile(I3C2_BCR, 0x06);
        write_volatile(I3C2_DCR, 0x63);
        write_volatile(I3C2_EPIDR, (0x0114 << 17) | (uid_mix & 0xFFFF));
        // Leave DAVAL=0 so ENTDAA assigns dynamic address cleanly
        write_volatile(I3C2_DEVR0, (static_addr as u32 & 0x7F) << 1);
        write_volatile(I3C2_MAXRLR, 128);
        write_volatile(I3C2_MAXWLR, 128);
        write_volatile(I3C2_TIMINGR0, 0x1818_0303);
        write_volatile(I3C2_TIMINGR1, (64 << 16) | 64);
        write_volatile(I3C2_TIMINGR2, 0);
        write_volatile(I3C2_CEVR, 0xFFFF_FFFF);
        write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | 1); // EN
    }
}

#[entry]
fn main() -> ! {
    let rsr = unsafe { read_volatile(RCC_RSR) };
    unsafe { write_volatile(RCC_RSR, rsr | RSR_RMVF) };
    let iwdg_rst = (rsr & RSR_IWDGRSTF) != 0;
    let fault_rst = (rsr & (RSR_IWDGRSTF | RSR_WWDGRSTF | RSR_SFTRSTF)) != 0;

    let uid_words = unsafe {
        [
            read_volatile(UID_BASE as *const u32),
            read_volatile((UID_BASE + 4) as *const u32),
            read_volatile((UID_BASE + 8) as *const u32),
        ]
    };
    let uid_mix = uid_words[0] ^ uid_words[1] ^ uid_words[2];
    let fallback_addr = bareboot_protocol::fallback_address(&uid_words);

    let (mailbox_req, node_addr) = read_mailbox_request(fallback_addr);
    let boot_loop_trapped = BootLoopGuard::is_trapped(BOOT_LOOP_GUARD_ADDR);
    if mailbox_req || !fault_rst {
        BootLoopGuard::clear(BOOT_LOOP_GUARD_ADDR);
    }

    init_clocks_and_pins();
    init_i3c2(node_addr, uid_mix);
    let mut iwdg_active = iwdg_rst;
    if iwdg_active {
        iwdg_start();
    }
    ensure_sector0_wrp_and_bor();

    let mut journal = MonotonicCommitJournal::<COMMIT_SIZE, 16>::new(COMMIT_BASE);
    let commit_valid = matches!(journal.init(&mut CommitSectorFlash), Ok(CommitState::Valid));
    let app_integrity_ok = commit_valid
        && app_vector_is_valid()
        && verify_app_crc32(journal.app_len(), journal.app_crc32());

    let mut flash = Stm32H5Flash::new(journal);
    let mut uid = [0u8; 12];
    flash.read_uid(&mut uid);

    let config = BootConfig {
        protocol_version: 1,
        app_base: APP_BASE,
        app_end: APP_END,
        block_size: BLOCK_SIZE,
        mcu_type: b"stm32h503kb",
        software_version: b"bareboot-i3c-0.2",
        uid,
    };

    let wd_flash = WatchdogFlash::new(flash, iwdg_feed as fn());
    let mut writer: PagedWriter<WatchdogFlash<Stm32H5Flash, fn()>, PAGE_SIZE> =
        PagedWriter::new(wd_flash, config);

    let mut rx_buf = [0u8; 256];
    let mut rx_len: usize = 0;
    let mut tx_buf = [0u8; 256];
    let mut tx_len: usize = 0;
    let mut tx_pos: usize = 0;

    let mut boot_window_active = !mailbox_req && !boot_loop_trapped && app_integrity_ok;
    // SysTick 1ms tick at 32 MHz
    unsafe {
        write_volatile(SYST_RVR, (SYSCLK_HZ / 1000) - 1);
        write_volatile(SYST_CVR, 0);
        write_volatile(SYST_CSR, 0x05); // Enable, processor clock
    }
    let mut ms_elapsed: u32 = 0;
    let mut sda_low_ms: u32 = 0;
    let mut idle_ms: u32 = 0;
    let mut i3c_idle_ms: u32 = 0;
    let mut led_on = true;

    loop {
        if !boot_window_active && !iwdg_active {
            iwdg_start();
            iwdg_active = true;
        }
        if iwdg_active {
            iwdg_feed();
        }

        unsafe {
            let evr_tick = read_volatile(I3C2_EVR);
            if (evr_tick & (EVR_FCF | EVR_RXFNEF | EVR_DAUPDF | EVR_RSTF)) != 0 {
                idle_ms = 0;
                i3c_idle_ms = 0;
            }
            if read_volatile(SYST_CSR) & (1 << 16) != 0 {
                ms_elapsed = ms_elapsed.wrapping_add(1);
                idle_ms = idle_ms.saturating_add(1);
                i3c_idle_ms = i3c_idle_ms.saturating_add(1);
                if ms_elapsed % 250 == 0 {
                    led_on = !led_on;
                    write_volatile(GPIOA_BSRR, if led_on { 1 << 16 } else { 1 });
                }
                let sda_low = (read_volatile(GPIOB_IDR) & PB4_SDA_MASK) == 0;
                let bus_progress = (read_volatile(I3C2_EVR)
                    & (EVR_FCF | EVR_RXFNEF | EVR_DAUPDF | EVR_RSTF | EVR_ERRF))
                    != 0;
                if !sda_low || bus_progress {
                    sda_low_ms = 0;
                } else {
                    sda_low_ms = sda_low_ms.saturating_add(1);
                }

                // Recover I3C2 either on stuck SDA (>=50 ms) or prolonged idle without DAA (>=2 s).
                let stuck_sda = sda_low_ms >= 50;
                let idle_unassigned = i3c_idle_ms >= I3C2_IDLE_RESET_MS
                    && (read_volatile(I3C2_DEVR0) & 1) == 0;
                if stuck_sda || idle_unassigned {
                    let prev_devr0 = read_volatile(I3C2_DEVR0);
                    reset_i3c2_bus();
                    init_i3c2(node_addr, uid_mix);
                    if stuck_sda && (prev_devr0 & 1) != 0 {
                        write_volatile(I3C2_DEVR0, prev_devr0);
                    }
                    rx_len = 0;
                    tx_len = 0;
                    tx_pos = 0;
                    sda_low_ms = 0;
                    i3c_idle_ms = 0;
                }

                // If parked in bootloader with zero valid I3C activity for 30s and a valid
                // non-looping app exists, reset cleanly to boot into the application.
                if app_integrity_ok && !boot_loop_trapped && idle_ms >= BOOTLOADER_IDLE_TIMEOUT_MS
                {
                    write_volatile(SCB_AIRCR, 0x05FA_0004);
                    loop {
                        core::hint::spin_loop();
                    }
                }

                if boot_window_active && ms_elapsed >= BOOT_WINDOW_MS {
                    if fault_rst {
                        BootLoopGuard::record_boot_attempt(BOOT_LOOP_GUARD_ADDR);
                    }
                    Stm32H5Flash::boot_jump(APP_BASE);
                }
            }

            let evr = read_volatile(I3C2_EVR);

            if evr & EVR_RSTF != 0 {
                write_volatile(I3C2_CEVR, EVR_RSTF);
                write_volatile(I3C2_DEVR0, read_volatile(I3C2_DEVR0) & !1);
                write_volatile(I3C2_TGTTDR, 0);
                write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_RXFLUSH | CFGR_TXFLUSH);
                rx_len = 0;
                tx_len = 0;
                tx_pos = 0;
                boot_window_active = false;
                BootLoopGuard::clear(BOOT_LOOP_GUARD_ADDR);
            }

            if evr & EVR_DAUPDF != 0 {
                write_volatile(I3C2_CEVR, EVR_DAUPDF);
                let rmr = read_volatile(I3C2_RMR);
                let rcode = ((rmr >> 8) & 0xFF) as u8;
                if rcode == ccc::DIRECT_RSTDAA {
                    write_volatile(I3C2_DEVR0, read_volatile(I3C2_DEVR0) & !1);
                } else {
                    boot_window_active = false;
                }
            }

            if evr & EVR_ERRF != 0 {
                let _ = read_volatile(I3C2_SER);
                write_volatile(I3C2_CEVR, EVR_ERRF);
                write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_RXFLUSH);
                rx_len = 0;
            }

            while read_volatile(I3C2_EVR) & EVR_RXFNEF != 0 {
                let b = (read_volatile(I3C2_RDR) & 0xFF) as u8;
                if rx_len < rx_buf.len() {
                    rx_buf[rx_len] = b;
                    rx_len += 1;
                }
            }

            let evr_end = read_volatile(I3C2_EVR);
            if evr_end & (EVR_FCF | EVR_RXTGTENDF) != 0 {
                while read_volatile(I3C2_EVR) & EVR_RXFNEF != 0 {
                    let b = (read_volatile(I3C2_RDR) & 0xFF) as u8;
                    if rx_len < rx_buf.len() {
                        rx_buf[rx_len] = b;
                        rx_len += 1;
                    }
                }
                write_volatile(I3C2_CEVR, EVR_FCF | EVR_RXTGTENDF);

                let rcode = ((read_volatile(I3C2_RMR) >> 8) & 0xFF) as u8;
                if rcode == ccc::DIRECT_SETDASA && rx_len >= 1 {
                    let da = (rx_buf[0] >> 1) & 0x7F;
                    if (0x08..=0x77).contains(&da) {
                        write_volatile(I3C2_DEVR0, ((da as u32) << 1) | 1);
                        boot_window_active = false;
                    }
                    rx_len = 0;
                } else if rx_len > 0 {
                    let resp_len = match dispatch_one(&mut writer, &rx_buf[..rx_len], &mut tx_buf) {
                        Ok(len) => len,
                        Err(_) => {
                            bareboot_protocol::encode_frame(responses::NACK, &[], &mut tx_buf)
                                .unwrap_or(0)
                        }
                    };
                    if resp_len > 0 {
                        boot_window_active = false;
                        tx_len = resp_len;
                        tx_pos = 0;
                        write_volatile(I3C2_TGTTDR, 0);
                        write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_TXFLUSH);
                        for _ in 0..64 {
                            if read_volatile(I3C2_EVR) & EVR_TXFEF != 0 {
                                break;
                            }
                        }
                        write_volatile(I3C2_TGTTDR, (1 << 16) | (resp_len as u32));
                        // Tightly drain the entire TX response into the 8-byte FIFO
                        // without main-loop SysTick/SDA-check jitter so 1+ MHz SDR reads
                        // never underrun after the first 8 bytes.
                        let mut wait_spins: u32 = 0;
                        while tx_pos < tx_len && wait_spins < 500_000 {
                            iwdg_feed();
                            let ev = read_volatile(I3C2_EVR);
                            if ev & EVR_TXFNFF != 0 {
                                write_volatile(I3C2_TDR, tx_buf[tx_pos] as u32);
                                tx_pos += 1;
                                wait_spins = 0;
                                continue;
                            }
                            if ev & (EVR_RXFNEF | EVR_ERRF | EVR_RSTF | EVR_DAUPDF) != 0 {
                                break;
                            }
                            wait_spins += 1;
                        }
                    }
                    rx_len = 0;
                }

                if writer.is_completed() && tx_pos >= tx_len {
                    // Brief delay to allow final private_read to finish clocking out
                    for _ in 0..200_000 {
                        cortex_m::asm::nop();
                    }
                    BootLoopGuard::clear(BOOT_LOOP_GUARD_ADDR);
                    Stm32H5Flash::boot_jump(APP_BASE);
                }
            }
        }
    }
}
