#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use bareboot_core::{CoreError, FlashTarget, PagedWriter};
use bareboot_core::dispatch::dispatch_one;
use bareboot_protocol::{self, FrameParse};
use cortex_m_rt::entry;
use cortex_m_rt::ExceptionFrame;
use stm32_metapac as _;

include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));

// ---------- FLASH TARGET ----------
struct H503Flash;

impl H503Flash {
    fn unlock() {
        if stm32_metapac::FLASH.nscr().read().lock() {
            stm32_metapac::FLASH.nskeyr().write_value(0x4567_0123);
            stm32_metapac::FLASH.nskeyr().write_value(0xCDEF_89AB);
        }
    }

    fn wait_ready() {
        while {
            let sr = stm32_metapac::FLASH.nssr().read();
            sr.bsy() || sr.wbne() || sr.dbne()
        } {}
    }
}

impl FlashTarget for H503Flash {
    fn page_size(&self) -> usize { 8192 }
    fn app_base(&self) -> u32 { APP_BASE }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        let sector = ((addr - 0x0800_0000) / 8192) as u8;
        Self::wait_ready();
        stm32_metapac::FLASH.nsccr().write(|w| {
            w.set_clr_eop(true);
            w.set_clr_wrperr(true);
            w.set_clr_pgserr(true);
            w.set_clr_strberr(true);
            w.set_clr_incerr(true);
            w.set_clr_optchangeerr(true);
        });

        stm32_metapac::FLASH.nscr().write(|w| {
            w.set_ser(true);
            w.set_snb(sector);
        });
        stm32_metapac::FLASH.nscr().modify(|w| {
            w.set_strt(true);
        });
        Self::wait_ready();
        stm32_metapac::FLASH.nscr().write(|_| {});

        let sr = stm32_metapac::FLASH.nssr().read();
        if sr.wrperr() || sr.pgserr() || sr.strberr() || sr.incerr() {
            return Err(CoreError::Flash);
        }
        Ok(())
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        Self::wait_ready();
        stm32_metapac::FLASH.nsccr().write(|w| {
            w.set_clr_eop(true);
            w.set_clr_wrperr(true);
            w.set_clr_pgserr(true);
            w.set_clr_strberr(true);
            w.set_clr_incerr(true);
            w.set_clr_optchangeerr(true);
        });
        stm32_metapac::FLASH.nscr().write(|w| {
            w.set_pg(true);
        });

        for (i, chunk) in data.chunks(16).enumerate() {
            let target = (addr as usize + i * 16) as *mut u32;
            let mut quad = [0xFFFFFFFFu32; 4];
            for (w, wchunk) in chunk.chunks(4).enumerate() {
                let mut b = [0xFFu8; 4];
                b[..wchunk.len()].copy_from_slice(wchunk);
                quad[w] = u32::from_le_bytes(b);
            }
            unsafe {
                for w in 0..4 {
                    core::ptr::write_volatile(target.add(w), quad[w]);
                }
            }
            Self::wait_ready();
            let sr = stm32_metapac::FLASH.nssr().read();
            if sr.wrperr() || sr.pgserr() || sr.strberr() || sr.incerr() {
                stm32_metapac::FLASH.nscr().write(|_| {});
                return Err(CoreError::Flash);
            }
        }
        stm32_metapac::FLASH.nscr().write(|_| {});

        // Readback verify
        let flash_slice = unsafe { core::slice::from_raw_parts(addr as *const u8, data.len()) };
        if flash_slice == data {
            Ok(())
        } else {
            Err(CoreError::Flash)
        }
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        for (i, b) in out.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile((addr + i as u32) as *const u8) };
        }
        Ok(())
    }

    fn read_uid(&mut self, out: &mut [u8; 12]) {
        for w in 0..3 {
            let word = unsafe { core::ptr::read_volatile((0x08FF_F800 + w * 4) as *const u32) };
            out[w * 4..w * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }

    unsafe fn boot_jump(app_entry: u32) -> ! {
        unsafe {
            asm!("cpsid i");
            // Disable I2C peripheral and its NVIC interrupts
            core::ptr::write_volatile(I2C2_CR1, 0);
            let icer1 = 0xE000_E184 as *mut u32;
            core::ptr::write_volatile(icer1, (1 << 21) | (1 << 22));

            const VTOR: *mut u32 = 0xE000_ED08 as *mut u32;
            core::ptr::write_volatile(VTOR, app_entry);
            let sp = core::ptr::read_volatile(app_entry as *const u32);
            let reset = core::ptr::read_volatile((app_entry + 4) as *const u32);
            asm!(
                "msr msp, {0}",
                "cpsie i",
                "bx {1}",
                in(reg) sp,
                in(reg) reset,
                options(noreturn)
            );
        }
    }
}

// ---------- I2C REGISTERS & BUFFERS ----------
const I2C2_BASE: u32 = 0x4000_5800;
const I2C2_CR1: *mut u32 = (I2C2_BASE + 0x00) as *mut u32;
const I2C2_OAR1: *mut u32 = (I2C2_BASE + 0x08) as *mut u32;
const I2C2_TIMINGR: *mut u32 = (I2C2_BASE + 0x10) as *mut u32;
const I2C2_ISR: *mut u32 = (I2C2_BASE + 0x18) as *mut u32;
const I2C2_ICR: *mut u32 = (I2C2_BASE + 0x1C) as *mut u32;
const I2C2_RXDR: *const u32 = (I2C2_BASE + 0x24) as *const u32;
const I2C2_TXDR: *mut u32 = (I2C2_BASE + 0x28) as *mut u32;

const I2C_RX_BUF_SIZE: usize = 192;
const I2C_TX_BUF_SIZE: usize = 128;
static mut I2C_RX_BUF: [u8; I2C_RX_BUF_SIZE] = [0; I2C_RX_BUF_SIZE];
static mut I2C_RX_POS: usize = 0;
static mut I2C_TX_BUF: [u8; I2C_TX_BUF_SIZE] = [0; I2C_TX_BUF_SIZE];
static mut I2C_TX_POS: usize = 0;
static mut I2C_TX_MAX: usize = 0;
static mut I2C_STOP_RECEIVED: bool = false;

fn i2c_init() {
    unsafe {
        // Set HSI to 64 MHz (DIV1): in RCC_CR, clear HSIDIV (bits 3..4)
        let rcc_cr = 0x4402_0c00 as *mut u32;
        core::ptr::write_volatile(rcc_cr, core::ptr::read_volatile(rcc_cr) & !(0x3 << 3));
        while core::ptr::read_volatile(rcc_cr) & (1 << 5) == 0 {} // wait for HSIDIVF

        // Enable GPIOB clock (AHB2ENR bit1)
        let ahb2enr = 0x4402_0c8c as *mut u32;
        core::ptr::write_volatile(ahb2enr, core::ptr::read_volatile(ahb2enr) | (1 << 1));

        // Enable I2C2 clock (APB1LENR bit 22)
        let apb1lenr = 0x4402_0c9c as *mut u32;
        core::ptr::write_volatile(apb1lenr, core::ptr::read_volatile(apb1lenr) | (1 << 22));

        // Configure PB3 (I2C2_SCL, AF8) and PB4 (I2C2_SDA, AF8)
        let moder = 0x4202_0400 as *mut u32;
        let mut v = core::ptr::read_volatile(moder);
        v &= !((0x3 << 6) | (0x3 << 8) | (0x3 << 16));
        v |= (0x2 << 6) | (0x2 << 8); // PB3 AF, PB4 AF
        v |= 0x1 << 16;               // PB8 output (LED_B)
        core::ptr::write_volatile(moder, v);

        let afrl = 0x4202_0420 as *mut u32;
        let mut a = core::ptr::read_volatile(afrl);
        a &= !((0xF << 12) | (0xF << 16));
        a |= (0x8 << 12) | (0x8 << 16); // PB3 AF8, PB4 AF8
        core::ptr::write_volatile(afrl, a);

        let otyper = 0x4202_0404 as *mut u32;
        let mut ot = core::ptr::read_volatile(otyper);
        ot |= (1 << 3) | (1 << 4); // Open-Drain for PB3 and PB4
        core::ptr::write_volatile(otyper, ot);

        let ospeedr = 0x4202_0408 as *mut u32;
        let mut s = core::ptr::read_volatile(ospeedr);
        s |= (0x3 << 6) | (0x3 << 8); // High speed
        core::ptr::write_volatile(ospeedr, s);

        let pupdr = 0x4202_040c as *mut u32;
        let mut p = core::ptr::read_volatile(pupdr);
        p &= !((0x3 << 6) | (0x3 << 8));
        p |= (0x1 << 6) | (0x1 << 8); // Pull-up for PB3 and PB4
        core::ptr::write_volatile(pupdr, p);

        // Configure I2C2 peripheral
        core::ptr::write_volatile(I2C2_CR1, 0); // Disable during configuration

        // TIMINGR for 100 kHz at 64 MHz clock: PRESC=3, SCLDEL=4, SDADEL=2, SCLH=0x4F, SCLL=0xC7
        core::ptr::write_volatile(I2C2_TIMINGR, 0x3042_4FC7);

        // Own address 1: 7-bit, I2C_TARGET_ADDRESS, OA1EN (bit 15)
        core::ptr::write_volatile(I2C2_OAR1, ((I2C_TARGET_ADDRESS as u32) << 1) | (1 << 15));

        // CR1: PE(0), TXIE(1), RXIE(2), ADDRIE(3), NACKIE(4), STOPIE(5), ERRIE(7)
        core::ptr::write_volatile(I2C2_CR1, (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 7));
    }
}

fn i2c_irq_enable() {
    unsafe {
        // I2C2_EV: IRQ 53 -> ISER1 bit (53-32)=21
        // I2C2_ER: IRQ 54 -> ISER1 bit (54-32)=22
        let iser1 = 0xE000_E104 as *mut u32;
        core::ptr::write_volatile(iser1, (1 << 21) | (1 << 22));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn I2C2_EV() {
    unsafe {
        let isr = core::ptr::read_volatile(I2C2_ISR);

        // Address matched (ADDR, bit 3)
        if isr & (1 << 3) != 0 {
            let dir = (isr >> 16) & 1; // 0 = write (slave rx), 1 = read (slave tx)
            if dir == 0 {
                I2C_RX_POS = 0;
                I2C_TX_POS = 0;
                I2C_TX_MAX = 0;
            } else {
                // Master read: flush TXDR and preload next byte
                core::ptr::write_volatile(I2C2_ISR, core::ptr::read_volatile(I2C2_ISR) | (1 << 0));
                if I2C_TX_POS < I2C_TX_MAX {
                    let b = I2C_TX_BUF[I2C_TX_POS];
                    core::ptr::write_volatile(I2C2_TXDR, b as u32);
                    I2C_TX_POS += 1;
                } else {
                    core::ptr::write_volatile(I2C2_TXDR, 0xFF);
                }
            }
            // Clear ADDR flag
            core::ptr::write_volatile(I2C2_ICR, 1 << 3);
        }

        // RXNE (bit 2) - Byte received
        if isr & (1 << 2) != 0 {
            let byte = core::ptr::read_volatile(I2C2_RXDR) as u8;
            if I2C_RX_POS < I2C_RX_BUF_SIZE {
                I2C_RX_BUF[I2C_RX_POS] = byte;
                I2C_RX_POS += 1;
            }
        }

        // TXIS (bit 1) - Transmit buffer empty (master reading subsequent bytes)
        if isr & (1 << 1) != 0 {
            if I2C_TX_POS < I2C_TX_MAX {
                let b = I2C_TX_BUF[I2C_TX_POS];
                core::ptr::write_volatile(I2C2_TXDR, b as u32);
                I2C_TX_POS += 1;
            } else {
                core::ptr::write_volatile(I2C2_TXDR, 0xFF);
            }
        }

        // NACKF (bit 4)
        if isr & (1 << 4) != 0 {
            core::ptr::write_volatile(I2C2_ICR, 1 << 4);
            core::ptr::write_volatile(I2C2_ISR, core::ptr::read_volatile(I2C2_ISR) | (1 << 0));
        }

        // STOPF (bit 5)
        if isr & (1 << 5) != 0 {
            core::ptr::write_volatile(I2C2_ICR, 1 << 5);
            core::ptr::write_volatile(I2C2_ISR, core::ptr::read_volatile(I2C2_ISR) | (1 << 0));
            if I2C_RX_POS > 0 {
                I2C_STOP_RECEIVED = true;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn I2C2_ER() {
    unsafe {
        // Clear BERR (bit 8), ARLO (bit 9), OVR (bit 10)
        core::ptr::write_volatile(I2C2_ICR, (1 << 8) | (1 << 9) | (1 << 10));
    }
}

#[inline(never)]
fn delay_ms(ms: u32) {
    cortex_m::asm::delay(ms.saturating_mul(16_000));
}

fn dwt_cycle_counter_init() {
    unsafe {
        const DEMCR: *mut u32 = 0xE000_EDFC as *mut u32;
        const DWT_LAR: *mut u32 = 0xE000_1FB0 as *mut u32;
        const DWT_CTRL: *mut u32 = 0xE000_1000 as *mut u32;
        const DWT_CYCCNT: *mut u32 = 0xE000_1004 as *mut u32;
        core::ptr::write_volatile(DEMCR, core::ptr::read_volatile(DEMCR) | (1 << 24));
        core::ptr::write_volatile(DWT_LAR, 0xC5AC_CE55);
        core::ptr::write_volatile(DWT_CYCCNT, 0);
        core::ptr::write_volatile(DWT_CTRL, core::ptr::read_volatile(DWT_CTRL) | 1);
    }
}
#[inline]
fn cycle_count() -> u32 { unsafe { core::ptr::read_volatile(0xE000_1004 as *const u32) } }
#[inline]
fn dwt_cycles_per_ms() -> u32 { 64000 }

#[entry]
fn main() -> ! {
    i2c_init();
    i2c_irq_enable();
    dwt_cycle_counter_init();
    H503Flash::unlock();

    let mut uid = [0u8; 12];
    H503Flash.read_uid(&mut uid);

    let flash = H503Flash;
    let mut writer = PagedWriter::<H503Flash, 8192>::new(flash, bareboot_core::BootConfig {
        protocol_version: 0x00010100,
        app_base: APP_BASE,
        block_size: 64,
        mcu_type: b"stm32h503kb",
        software_version: b"dev",
        uid,
    });

    let mut rx = [0u8; 256];
    let mut rx_len = 0usize;
    let mut elapsed_ms = 0u32;
    let mut last_ms_at = cycle_count();
    let boot_window = BOOT_WINDOW_MS;

    loop {
        let now = cycle_count();
        if (now.wrapping_sub(last_ms_at)) >= dwt_cycles_per_ms() {
            last_ms_at = last_ms_at.wrapping_add(dwt_cycles_per_ms());
            elapsed_ms = elapsed_ms.saturating_add(1);
        }

        let i2c_stop = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(I2C_STOP_RECEIVED)) };
        if i2c_stop {
            unsafe {
                core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_STOP_RECEIVED), false);
                let rpos = core::ptr::read_volatile(core::ptr::addr_of!(I2C_RX_POS));
                if rpos > 0 {
                    for i in 0..rpos.min(rx.len()) {
                        rx[i] = I2C_RX_BUF[i];
                    }
                    rx_len = rpos.min(rx.len());
                    I2C_RX_POS = 0;
                }
                // Toggle PB8 LED on packet received
                let odr = 0x4202_0414 as *mut u32;
                core::ptr::write_volatile(odr, core::ptr::read_volatile(odr) ^ (1 << 8));
            }
        }

        let mut consumed = 0;
        while consumed < rx_len {
            match bareboot_protocol::scan(&rx[consumed..rx_len]) {
                FrameParse::Complete { frame_len, command, .. } => {
                    let mut out = [0u8; 256];
                    let res = dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out);
                    match res {
                        Ok(n) => {
                            unsafe {
                                I2C_TX_POS = 0;
                                I2C_TX_MAX = n;
                                for i in 0..n {
                                    I2C_TX_BUF[i] = out[i];
                                }
                            }
                        }
                        Err(_) => {
                            let mut err_out = [0u8; 16];
                            if let Ok(n) = bareboot_protocol::encode_frame(bareboot_protocol::responses::COMMAND_ERROR, &[], &mut err_out) {
                                unsafe {
                                    I2C_TX_POS = 0;
                                    I2C_TX_MAX = n;
                                    for i in 0..n {
                                        I2C_TX_BUF[i] = err_out[i];
                                    }
                                }
                            }
                        }
                    }
                    consumed += frame_len;
                    let _ = command;
                }
                FrameParse::Invalid { drop } => {
                    consumed += drop.max(1);
                }
                FrameParse::NeedMore => break,
            }
        }
        if consumed > 0 {
            let available = rx_len;
            let shift = consumed.min(available);
            for i in 0..available.saturating_sub(shift) {
                rx[i] = rx[shift + i];
            }
            if consumed <= available { rx_len = available - consumed; } else { rx_len = 0; }
        }

        if writer.is_completed() {
            delay_ms(100);
            let app_sp = unsafe { core::ptr::read_volatile(APP_BASE as *const u32) };
            if app_sp >= 0x2000_0000 && app_sp <= 0x2000_8000 {
                unsafe { H503Flash::boot_jump(APP_BASE) }
            }
        }

        if rx_len == 0 && elapsed_ms >= boot_window {
            let app_sp = unsafe { core::ptr::read_volatile(APP_BASE as *const u32) };
            if app_sp >= 0x2000_0000 && app_sp <= 0x2000_8000 {
                unsafe { H503Flash::boot_jump(APP_BASE) }
            }
        }
    }
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_ef: &ExceptionFrame) -> ! {
    loop { let _ = core::hint::black_box(0); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
