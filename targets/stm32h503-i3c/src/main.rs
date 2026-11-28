#![no_std]
#![no_main]

use bareboot_arch_cortexm::CortexMArch;
use bareboot_core::{
    apply_safe_pins, check_and_clear_bootloader_request, ArchBoot, BootConfig, BootPlatform,
    BootRunner, BusTransport, CoreError, FlashRegion, PinMode, PollEvent, ResetReason, TickTimer,
    WatchdogFeed,
};
use bareboot_protocol::{ccc, SoftResetMailbox, REQUEST_BOOTLOADER};
use core::ptr::{read_volatile, write_volatile};
use cortex_m_rt::entry;

include!(concat!(env!("OUT_DIR"), "/board_config_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/safe_pins_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/status_led_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/transport_arbiter_generated.rs"));

const BOOT_LOOP_GUARD_ADDR: usize = 0x2000_7FD0;
const LEGACY_COOKIE_ADDR: usize = 0x2000_7FF8;

const BOOTLOADER_IDLE_TIMEOUT_MS: u32 = 30_000;
const I3C2_IDLE_RESET_MS: u32 = 2_000;
const SYSCLK_HZ: u32 = 32_000_000; // STM32H503 reset HSIDIV=/2 (32 MHz)

const SYST_CSR: *mut u32 = 0xE000_E010 as *mut u32;
const SYST_RVR: *mut u32 = 0xE000_E014 as *mut u32;
const SYST_CVR: *mut u32 = 0xE000_E018 as *mut u32;

const BLOCK_SIZE: usize = 64;

// RCC (Non-Secure)
const RCC_BASE: u32 = 0x4402_0C00;
const RCC_I3C_RSTR: *mut u32 = (RCC_BASE + RCC_I3C_RSTR_OFFSET) as *mut u32;
const RCC_AHB1ENR: *mut u32 = (RCC_BASE + 0x088) as *mut u32;
const RCC_AHB2ENR: *mut u32 = (RCC_BASE + 0x08C) as *mut u32;
const RCC_I3C_ENR: *mut u32 = (RCC_BASE + RCC_I3C_ENR_OFFSET) as *mut u32;
const RCC_RSR: *mut u32 = (RCC_BASE + 0x0F4) as *mut u32;
const I3C_RCC_MASK: u32 = 1 << RCC_I3C_BIT;
const RSR_RMVF: u32 = 1 << 23;
const RSR_SFTRSTF: u32 = 1 << 28;
const RSR_IWDGRSTF: u32 = 1 << 29;
const RSR_WWDGRSTF: u32 = 1 << 30;

// GPIOA..GPIOH (Non-Secure)
const GPIOA_BASE: u32 = 0x4202_0000;
const HOST_SDA_GPIO_IDR: *const u32 =
    (GPIOA_BASE + ((HOST_SDA_PORT as u32) - ('A' as u32)) * 0x0400 + 0x10) as *const u32;
const HOST_SDA_MASK: u32 = 1 << (HOST_SDA_PIN as u32);

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

// I3C peripheral registers (base from board_config_generated.rs)
const I3C2_CFGR: *mut u32 = (I3C_BASE + 0x04) as *mut u32;
const I3C2_RDR: *const u32 = (I3C_BASE + 0x10) as *const u32;
const I3C2_TDR: *mut u32 = (I3C_BASE + 0x18) as *mut u32;
const I3C2_TGTTDR: *mut u32 = (I3C_BASE + 0x24) as *mut u32;
const I3C2_SER: *const u32 = (I3C_BASE + 0x34) as *const u32;
const I3C2_RMR: *const u32 = (I3C_BASE + 0x40) as *const u32;
const I3C2_EVR: *const u32 = (I3C_BASE + 0x50) as *const u32;
const I3C2_CEVR: *mut u32 = (I3C_BASE + 0x58) as *mut u32;
const I3C2_DEVR0: *mut u32 = (I3C_BASE + 0x60) as *mut u32;
const I3C2_MAXRLR: *mut u32 = (I3C_BASE + 0x90) as *mut u32;
const I3C2_MAXWLR: *mut u32 = (I3C_BASE + 0x94) as *mut u32;
const I3C2_TIMINGR0: *mut u32 = (I3C_BASE + 0xA0) as *mut u32;
const I3C2_TIMINGR1: *mut u32 = (I3C_BASE + 0xA4) as *mut u32;
const I3C2_TIMINGR2: *mut u32 = (I3C_BASE + 0xA8) as *mut u32;
const I3C2_BCR: *mut u32 = (I3C_BASE + 0xC0) as *mut u32;
const I3C2_DCR: *mut u32 = (I3C_BASE + 0xC4) as *mut u32;
const I3C2_EPIDR: *mut u32 = (I3C_BASE + 0xD4) as *mut u32;

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
    CortexMArch::system_reset()
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

struct IwdgWatchdog(bool);

impl WatchdogFeed for IwdgWatchdog {
    #[inline(always)]
    fn feed(&mut self) {
        if self.0 {
            iwdg_feed();
        }
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
        if (wrp_cur & 0x03) == 0 && (opt_cur & 0x03) == 0x03 {
            return;
        }
        while read_volatile(FLASH_NSSR) & 1 != 0 {}
        write_volatile(FLASH_NSCCR, 0x00FE_0000);
        if read_volatile(FLASH_OPTCR) & 1 != 0 {
            write_volatile(FLASH_OPTKEYR, 0x0819_2A3B);
            write_volatile(FLASH_OPTKEYR, 0x4C5D_6E7F);
            while read_volatile(FLASH_OPTCR) & 1 != 0 {}
        }
        write_volatile(FLASH_WRPSGN1R_PRG, wrp_cur & !0x03); // Bits [1:0] = 00 protects Sectors 0..1 (16 KiB)
        write_volatile(FLASH_OPTSR_PRG, (opt_cur & !0x03) | 0x03); // BOR Level 3 (~2.7 V)
        write_volatile(FLASH_OPTCR, read_volatile(FLASH_OPTCR) | (1 << 1)); // OPTSTRT
        while read_volatile(FLASH_OPTCR) & (1 << 1) != 0 {
            iwdg_feed();
        }
        write_volatile(FLASH_OPTCR, read_volatile(FLASH_OPTCR) | 1); // OPTLOCK
    }
}

struct Stm32H5FlashRegion {
    base: u32,
    end: u32,
}

impl FlashRegion for Stm32H5FlashRegion {
    fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    fn region_base(&self) -> u32 {
        self.base
    }

    fn region_end(&self) -> u32 {
        self.end
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        if addr < self.base || addr >= self.end || (addr & (PAGE_SIZE as u32 - 1)) != 0 {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_erase_sector(addr) }
    }

    fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        if addr < self.base || addr.saturating_add(data.len() as u32) > self.end {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_write_quads(addr, data) }
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        if addr < self.base || addr.saturating_add(out.len() as u32) > self.end {
            return Err(CoreError::InvalidAddress);
        }
        unsafe { flash_read_volatile(addr, out) };
        Ok(())
    }
}

struct SysTickTimer;

impl SysTickTimer {
    fn init() -> Self {
        unsafe {
            write_volatile(SYST_RVR, (SYSCLK_HZ / 1000) - 1);
            write_volatile(SYST_CVR, 0);
            write_volatile(SYST_CSR, 0x05); // Enable, processor clock
        }
        Self
    }
}

impl TickTimer for SysTickTimer {
    fn poll_1ms(&mut self) -> bool {
        unsafe { (read_volatile(SYST_CSR) & (1 << 16)) != 0 }
    }

    fn on_heartbeat(&mut self, led_on: bool) {
        status_led_set(led_on);
    }
}

struct I3c2Transport {
    node_addr: u8,
    uid_mix: u32,
    iwdg_active: bool,
    rx_buf: [u8; 256],
    rx_len: usize,
    tx_idle: bool,
    sda_low_ms: u32,
    i3c_idle_ms: u32,
}

impl I3c2Transport {
    fn new(node_addr: u8, uid_mix: u32, iwdg_active: bool) -> Self {
        init_i3c2(node_addr, uid_mix);
        Self {
            node_addr,
            uid_mix,
            iwdg_active,
            rx_buf: [0u8; 256],
            rx_len: 0,
            tx_idle: true,
            sda_low_ms: 0,
            i3c_idle_ms: 0,
        }
    }
}

impl BusTransport for I3c2Transport {
    fn has_activity(&mut self) -> bool {
        let evr_tick = unsafe { read_volatile(I3C2_EVR) };
        if (evr_tick & (EVR_FCF | EVR_RXFNEF | EVR_DAUPDF | EVR_RSTF)) != 0 {
            self.i3c_idle_ms = 0;
            true
        } else {
            false
        }
    }

    fn on_tick_1ms(&mut self) {
        unsafe {
            self.i3c_idle_ms = self.i3c_idle_ms.saturating_add(1);
            let sda_low = (read_volatile(HOST_SDA_GPIO_IDR) & HOST_SDA_MASK) == 0;
            let bus_progress = (read_volatile(I3C2_EVR)
                & (EVR_FCF | EVR_RXFNEF | EVR_DAUPDF | EVR_RSTF | EVR_ERRF))
                != 0;
            if !sda_low || bus_progress {
                self.sda_low_ms = 0;
            } else {
                self.sda_low_ms = self.sda_low_ms.saturating_add(1);
            }

            // Recover I3C2 either on stuck SDA (>=50 ms) or prolonged idle without DAA (>=2 s).
            let stuck_sda = self.sda_low_ms >= 50;
            let idle_unassigned = self.i3c_idle_ms >= I3C2_IDLE_RESET_MS
                && (read_volatile(I3C2_DEVR0) & 1) == 0;
            if stuck_sda || idle_unassigned {
                let prev_devr0 = read_volatile(I3C2_DEVR0);
                reset_i3c2_bus();
                init_i3c2(self.node_addr, self.uid_mix);
                if stuck_sda && (prev_devr0 & 1) != 0 {
                    write_volatile(I3C2_DEVR0, prev_devr0);
                }
                self.rx_len = 0;
                self.tx_idle = true;
                self.sda_low_ms = 0;
                self.i3c_idle_ms = 0;
            }
        }
    }

    fn poll_rx(&mut self, buf: &mut [u8]) -> PollEvent {
        unsafe {
            let evr = read_volatile(I3C2_EVR);
            let mut event = PollEvent::None;

            if evr & EVR_RSTF != 0 {
                write_volatile(I3C2_CEVR, EVR_RSTF);
                write_volatile(I3C2_DEVR0, read_volatile(I3C2_DEVR0) & !1);
                write_volatile(I3C2_TGTTDR, 0);
                write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_RXFLUSH | CFGR_TXFLUSH);
                self.rx_len = 0;
                self.tx_idle = true;
                return PollEvent::BusReset;
            }

            if evr & EVR_DAUPDF != 0 {
                write_volatile(I3C2_CEVR, EVR_DAUPDF);
                let rmr = read_volatile(I3C2_RMR);
                let rcode = ((rmr >> 8) & 0xFF) as u8;
                if rcode == ccc::DIRECT_RSTDAA {
                    write_volatile(I3C2_DEVR0, read_volatile(I3C2_DEVR0) & !1);
                } else {
                    event = PollEvent::BusActivity;
                }
            }

            if evr & EVR_ERRF != 0 {
                let _ = read_volatile(I3C2_SER);
                write_volatile(I3C2_CEVR, EVR_ERRF);
                write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_RXFLUSH);
                self.rx_len = 0;
            }

            while read_volatile(I3C2_EVR) & EVR_RXFNEF != 0 {
                let b = (read_volatile(I3C2_RDR) & 0xFF) as u8;
                if self.rx_len < self.rx_buf.len() {
                    self.rx_buf[self.rx_len] = b;
                    self.rx_len += 1;
                }
            }

            let evr_end = read_volatile(I3C2_EVR);
            if evr_end & (EVR_FCF | EVR_RXTGTENDF) != 0 {
                while read_volatile(I3C2_EVR) & EVR_RXFNEF != 0 {
                    let b = (read_volatile(I3C2_RDR) & 0xFF) as u8;
                    if self.rx_len < self.rx_buf.len() {
                        self.rx_buf[self.rx_len] = b;
                        self.rx_len += 1;
                    }
                }
                write_volatile(I3C2_CEVR, EVR_FCF | EVR_RXTGTENDF);

                let rcode = ((read_volatile(I3C2_RMR) >> 8) & 0xFF) as u8;
                if rcode == ccc::DIRECT_SETDASA && self.rx_len >= 1 {
                    let da = (self.rx_buf[0] >> 1) & 0x7F;
                    if (0x08..=0x77).contains(&da) {
                        write_volatile(I3C2_DEVR0, ((da as u32) << 1) | 1);
                        event = PollEvent::BusActivity;
                    }
                    self.rx_len = 0;
                } else if self.rx_len > 0 {
                    let copy_len = self.rx_len.min(buf.len());
                    buf[..copy_len].copy_from_slice(&self.rx_buf[..copy_len]);
                    self.rx_len = 0;
                    return PollEvent::Frame(copy_len);
                }
            }

            event
        }
    }

    fn send_tx(&mut self, frame: &[u8]) -> Result<(), CoreError> {
        unsafe {
            self.tx_idle = false;
            write_volatile(I3C2_TGTTDR, 0);
            write_volatile(I3C2_CFGR, read_volatile(I3C2_CFGR) | CFGR_TXFLUSH);
            for _ in 0..64 {
                if read_volatile(I3C2_EVR) & EVR_TXFEF != 0 {
                    break;
                }
            }
            write_volatile(I3C2_TGTTDR, (1 << 16) | (frame.len() as u32));
            let mut tx_pos = 0usize;
            let mut wait_spins: u32 = 0;
            while tx_pos < frame.len() && wait_spins < 2_000_000 {
                if self.iwdg_active {
                    iwdg_feed();
                }
                let ev = read_volatile(I3C2_EVR);
                if ev & EVR_TXFNFF != 0 {
                    write_volatile(I3C2_TDR, frame[tx_pos] as u32);
                    tx_pos += 1;
                    wait_spins = 0;
                    continue;
                }
                if ev & (EVR_RXFNEF | EVR_ERRF | EVR_RSTF | EVR_DAUPDF) != 0 {
                    break;
                }
                wait_spins += 1;
            }
            self.tx_idle = tx_pos >= frame.len();
            Ok(())
        }
    }

    fn tx_idle(&self) -> bool {
        self.tx_idle
    }

    fn wait_tx_drain(&mut self) {
        // Brief delay to allow final private_read to finish clocking out
        for _ in 0..200_000 {
            cortex_m::asm::nop();
        }
    }

    fn prepare_for_jump(&mut self) {
        status_led_set(false);
        unsafe {
            reset_i3c2_bus();
            set_pin_analog(HOST_SCL_PORT, HOST_SCL_PIN);
            set_pin_analog(HOST_SDA_PORT, HOST_SDA_PIN);
        }
    }
}

struct Stm32H5Platform {
    uid: [u8; 12],
    reset_reason: ResetReason,
    mailbox_req: bool,
    transport: I3c2Transport,
    iwdg_active: bool,
    timer: SysTickTimer,
}

impl BootPlatform for Stm32H5Platform {
    type Flash = Stm32H5FlashRegion;
    type CommitFlash = Stm32H5FlashRegion;
    type Transport = I3c2Transport;
    type Watchdog = IwdgWatchdog;
    type Timer = SysTickTimer;
    type Arch = CortexMArch;

    fn boot_config(&self) -> BootConfig {
        BootConfig {
            protocol_version: 1,
            app_base: APP_BASE,
            app_end: APP_END,
            block_size: BLOCK_SIZE,
            mcu_type: b"stm32h503kb",
            software_version: b"bareboot-i3c-0.2",
            uid: self.uid,
        }
    }

    fn ram_bounds(&self) -> (u32, u32) {
        (RAM_START, RAM_END)
    }

    fn boot_window_ms(&self) -> u32 {
        BOOT_WINDOW_MS
    }

    fn idle_timeout_ms(&self) -> Option<u32> {
        Some(BOOTLOADER_IDLE_TIMEOUT_MS)
    }

    fn mailbox_addr(&self) -> Option<u32> {
        Some(MAILBOX_ADDR)
    }

    fn legacy_cookie_addr(&self) -> Option<u32> {
        Some(LEGACY_COOKIE_ADDR as u32)
    }

    fn boot_loop_guard_addr(&self) -> Option<usize> {
        Some(BOOT_LOOP_GUARD_ADDR)
    }

    fn reset_reason(&self) -> ResetReason {
        self.reset_reason
    }

    fn read_mailbox_request(&mut self) -> bool {
        self.mailbox_req
    }

    fn into_parts(
        self,
    ) -> (
        Self::Flash,
        Self::CommitFlash,
        Self::Transport,
        Self::Watchdog,
        Self::Timer,
    ) {
        (
            Stm32H5FlashRegion {
                base: APP_BASE,
                end: APP_END,
            },
            Stm32H5FlashRegion {
                base: COMMIT_BASE,
                end: COMMIT_BASE + COMMIT_SIZE as u32,
            },
            self.transport,
            IwdgWatchdog(self.iwdg_active),
            self.timer,
        )
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

unsafe fn reset_i3c2_bus() {
    unsafe {
        write_volatile(I3C2_CFGR, 0);
        let rstr = read_volatile(RCC_I3C_RSTR);
        write_volatile(RCC_I3C_RSTR, rstr | I3C_RCC_MASK);
        for _ in 0..8 {
            cortex_m::asm::nop();
        }
        write_volatile(RCC_I3C_RSTR, rstr & !I3C_RCC_MASK);
    }
}

#[inline(always)]
fn status_led_set(led_on: bool) {
    if let Some((port, pin, active_low)) = STATUS_LED_CONFIG {
        let gpio_base = GPIOA_BASE + ((port as u32) - ('A' as u32)) * 0x0400;
        let bsrr = (gpio_base + 0x18) as *mut u32;
        let drive_high = led_on ^ active_low;
        let val = if drive_high {
            1u32 << (pin as u32)
        } else {
            1u32 << ((pin as u32) + 16)
        };
        unsafe {
            write_volatile(bsrr, val);
        }
    }
}

fn init_clocks_and_pins() {
    unsafe {
        // Enable FLASH, GPIO clocks (A/B + any port in INITIAL_SAFE_PINS + STATUS_LED_CONFIG), I3C2 clock
        let mut gpio_clocks: u32 = 0x03; // GPIOA | GPIOB
        for sp in INITIAL_SAFE_PINS {
            if ('A'..='H').contains(&sp.port) {
                gpio_clocks |= 1u32 << ((sp.port as u32) - ('A' as u32));
            }
        }
        if let Some((port, _, _)) = STATUS_LED_CONFIG {
            if ('A'..='H').contains(&port) {
                gpio_clocks |= 1u32 << ((port as u32) - ('A' as u32));
            }
        }
        if ('A'..='H').contains(&HOST_SCL_PORT) {
            gpio_clocks |= 1u32 << ((HOST_SCL_PORT as u32) - ('A' as u32));
        }
        if ('A'..='H').contains(&HOST_SDA_PORT) {
            gpio_clocks |= 1u32 << ((HOST_SDA_PORT as u32) - ('A' as u32));
        }
        write_volatile(RCC_AHB1ENR, read_volatile(RCC_AHB1ENR) | (1 << 8) | (1 << 31));
        write_volatile(RCC_AHB2ENR, read_volatile(RCC_AHB2ENR) | gpio_clocks);
        write_volatile(RCC_I3C_ENR, read_volatile(RCC_I3C_ENR) | I3C_RCC_MASK);
        let _ = read_volatile(RCC_I3C_ENR);

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

        if let Some((port, pin, active_low)) = STATUS_LED_CONFIG {
            if ('A'..='H').contains(&port) && pin <= 15 {
                let gpio_base = GPIOA_BASE + ((port as u32) - ('A' as u32)) * 0x0400;
                let moder = gpio_base as *mut u32;
                let pupdr = (gpio_base + 0x0C) as *mut u32;
                let bsrr = (gpio_base + 0x18) as *mut u32;
                let shift = (pin as u32) * 2;
                let mask = 0x03u32 << shift;
                // Drive inactive first, then configure as push-pull output
                let inactive_val = if active_low {
                    1u32 << (pin as u32)
                } else {
                    1u32 << ((pin as u32) + 16)
                };
                write_volatile(bsrr, inactive_val);
                write_volatile(pupdr, read_volatile(pupdr) & !mask);
                write_volatile(moder, (read_volatile(moder) & !mask) | (0x01 << shift));
            }
        }
    }
}

fn set_pin_analog(port: char, pin: u8) {
    if !('A'..='H').contains(&port) || pin > 15 {
        return;
    }
    let gpio_base = GPIOA_BASE + ((port as u32) - ('A' as u32)) * 0x0400;
    let moder = gpio_base as *mut u32;
    let shift2 = (pin as u32) * 2;
    unsafe {
        write_volatile(moder, read_volatile(moder) | (0x03 << shift2));
    }
}

fn configure_af_pin(port: char, pin: u8, af: u8) {
    if !('A'..='H').contains(&port) || pin > 15 {
        return;
    }
    let gpio_base = GPIOA_BASE + ((port as u32) - ('A' as u32)) * 0x0400;
    let moder = gpio_base as *mut u32;
    let otyper = (gpio_base + 0x04) as *mut u32;
    let ospeedr = (gpio_base + 0x08) as *mut u32;
    let pupdr = (gpio_base + 0x0C) as *mut u32;
    let afr = (gpio_base + if pin < 8 { 0x20 } else { 0x24 }) as *mut u32;
    let shift2 = (pin as u32) * 2;
    let shift4 = ((pin % 8) as u32) * 4;
    unsafe {
        write_volatile(moder, (read_volatile(moder) & !(0x03 << shift2)) | (0x02 << shift2));
        write_volatile(otyper, read_volatile(otyper) & !(1u32 << pin));
        write_volatile(ospeedr, read_volatile(ospeedr) | (0x03 << shift2));
        write_volatile(pupdr, (read_volatile(pupdr) & !(0x03 << shift2)) | (0x01 << shift2));
        write_volatile(afr, (read_volatile(afr) & !(0x0F << shift4)) | ((af as u32) << shift4));
    }
}

fn init_i3c2(static_addr: u8, uid_mix: u32) {
    unsafe {
        configure_af_pin(HOST_SCL_PORT, HOST_SCL_PIN, HOST_AF);
        configure_af_pin(HOST_SDA_PORT, HOST_SDA_PIN, HOST_AF);

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
    let reset_reason = if (rsr & (RSR_IWDGRSTF | RSR_WWDGRSTF)) != 0 {
        ResetReason::Watchdog
    } else if (rsr & RSR_SFTRSTF) != 0 {
        ResetReason::Software
    } else {
        ResetReason::PowerOn
    };

    let uid_words = unsafe {
        [
            read_volatile(UID_BASE as *const u32),
            read_volatile((UID_BASE + 4) as *const u32),
            read_volatile((UID_BASE + 8) as *const u32),
        ]
    };
    let mut uid = [0u8; 12];
    for i in 0..3 {
        uid[i * 4..i * 4 + 4].copy_from_slice(&uid_words[i].to_le_bytes());
    }
    let uid_mix = uid_words[0] ^ uid_words[1] ^ uid_words[2];
    let fallback_addr = if HOST_STATIC_ADDR != 0 {
        HOST_STATIC_ADDR
    } else {
        bareboot_protocol::fallback_address(&uid_words)
    };

    let (mailbox_req, node_addr) = read_mailbox_request(fallback_addr);

    init_clocks_and_pins();
    if iwdg_rst {
        iwdg_start();
    }
    ensure_sector0_wrp_and_bor();

    let platform = Stm32H5Platform {
        uid,
        reset_reason,
        mailbox_req,
        transport: I3c2Transport::new(node_addr, uid_mix, iwdg_rst),
        iwdg_active: iwdg_rst,
        timer: SysTickTimer::init(),
    };

    let mut runner = BootRunner::<Stm32H5Platform, PAGE_SIZE, COMMIT_SIZE, 16>::new(platform);
    runner.run()
}
