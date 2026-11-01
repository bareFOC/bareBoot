#![no_std]
#![no_main]

use bareboot_core::{FlashTarget, PagedWriter};
use bareboot_core::dispatch::dispatch_one;
use bareboot_protocol::{self, FrameParse};
use core::arch::asm;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m as _;
use cortex_m_rt::entry;
use cortex_m_rt::ExceptionFrame;
use stm32_metapac as _;

#[path = "rtt.rs"]
mod rtt;

include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));

fn hex2(b: u8, out: &mut [u8; 2]) {
    const HEXD: &[u8; 16] = b"0123456789abcdef";
    out[0] = HEXD[(b >> 4) as usize];
    out[1] = HEXD[(b & 0xF) as usize];
}

struct BluepillFlash;

impl BluepillFlash {
    fn unlock() {
        unsafe {
            const KEYR: *mut u32 = 0x4002_2004 as *mut u32;
            core::ptr::write_volatile(KEYR, 0x4567_0123);
            core::ptr::write_volatile(KEYR, 0xCDEF_89AB);
        }
    }
}

impl FlashTarget for BluepillFlash {
    fn page_size(&self) -> usize { 1024 }
    fn app_base(&self) -> u32 { 0x0800_2000 }
    fn erase_page(&mut self , addr: u32) -> Result<(), bareboot_core::CoreError> {
        unsafe {
            const CR: *mut u32 = 0x4002_2010 as *mut u32;
            const AR: *mut u32 = 0x4002_2014 as *mut u32;
            const SR: *const u32 = 0x4002_200C as *const u32;
            core::ptr::write_volatile(CR, 1 << 1);
            core::ptr::write_volatile(AR, addr);
            core::ptr::write_volatile(CR, (1 << 1) | (1 << 6));
            while core::ptr::read_volatile(SR) & 1 != 0 {}
            core::ptr::write_volatile(CR, 0);
        }
        uart_put_byte(b'e');
        Ok(())
    }
    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), bareboot_core::CoreError> {
        unsafe {
            const CR: *mut u32 = 0x4002_2010 as *mut u32;
            const SR: *const u32 = 0x4002_200C as *const u32;
            for i in (0..data.len()).step_by(2) {
                core::ptr::write_volatile(CR, 1 << 0);
                let word = u16::from_le_bytes([data[i], data[i + 1]]);
                core::ptr::write_volatile((addr + i as u32) as *mut u16, word);
                while core::ptr::read_volatile(SR) & 1 != 0 {}
                core::ptr::write_volatile(CR, 0);
            }
        }
        Ok(())
    }
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), bareboot_core::CoreError> {
        for (i, b) in out.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile((addr + i as u32) as *const u8) };
        }
        Ok(())
    }
    fn read_uid(&mut self, out: &mut [u8; 12]) {
        for i in 0..12 {
            out[i] = unsafe { core::ptr::read_volatile((0x1FFF_F7E8 + i) as *const u8) };
        }
    }
    unsafe fn boot_jump(app_entry: u32) -> ! {
        asm!("cpsid i");
        let sp = unsafe { core::ptr::read_volatile(app_entry as *const u32) };
        let reset = unsafe { core::ptr::read_volatile((app_entry + 4) as *const u32) };
        unsafe { asm!("msr msp, {}", in(reg) sp); }
        let f: fn() -> ! = unsafe { core::mem::transmute(reset) };
        unsafe { f() }
    }
}

fn delay_ms(ms: u32) {
    for _ in 0..ms {
        for _ in 0..800 {
            let _ = core::hint::black_box(0);
        }
    }
}

fn uart_init() {
    unsafe {
        let apb2enr = 0x4002_1018 as *mut u32;
        core::ptr::write_volatile(apb2enr, core::ptr::read_volatile(apb2enr) | (1 << 2) | (1 << 14));
        let crh = 0x4001_0804 as *mut u32;
        let mut v = core::ptr::read_volatile(crh);
        v &= !(0xF << 4);
        v |= 0xB << 4;
        // PA10 RX: floating input (matches the verified loopback config).
        v &= !(0xF << 8);
        v |= 0x4 << 8;
        core::ptr::write_volatile(crh, v);
        core::ptr::write_volatile(0x4001_0810 as *mut u32, core::ptr::read_volatile(0x4001_0810 as *const u32) | (1 << 10));
        let brr = 0x4001_3808 as *mut u32;
        core::ptr::write_volatile(brr, 0x0045);
        let cr1 = 0x4001_380C as *mut u32;
        core::ptr::write_volatile(cr1, (1 << 13) | (1 << 3) | (1 << 2));
    }
}

/// USART1 IRQ #37 -> NVIC ISER1 bit 5; enable RXNE interrupt in CR1.
fn uart_irq_enable() {
    unsafe {
        let cr1 = 0x4001_380C as *mut u32;
        core::ptr::write_volatile(cr1, core::ptr::read_volatile(cr1) | (1 << 5));
        let iser1 = 0xE000_E104 as *mut u32;
        core::ptr::write_volatile(iser1, core::ptr::read_volatile(iser1) | (1 << 5));
    }
}

/// I2C1 register addresses (STM32F1)
const I2C1_CR1: *mut u32 = 0x4000_5400 as *mut u32;
const I2C1_CR2: *mut u32 = 0x4000_5404 as *mut u32;
const I2C1_OAR1: *mut u32 = 0x4000_5408 as *mut u32;
const I2C1_DR: *mut u32 = 0x4000_5410 as *mut u32;
const I2C1_SR1: *mut u32 = 0x4000_5414 as *mut u32;
const I2C1_SR2: *const u32 = 0x4000_5418 as *const u32;
const I2C1_CCR: *mut u32 = 0x4000_541C as *mut u32;
const I2C1_TRISE: *mut u32 = 0x4000_5420 as *mut u32;

fn i2c_init() {
    unsafe {
        // Enable GPIOB and I2C1 clocks
        let apb2enr = 0x4002_1018 as *mut u32;
        core::ptr::write_volatile(apb2enr, core::ptr::read_volatile(apb2enr) | (1 << 3)); // IOPBEN
        let apb1enr = 0x4002_101C as *mut u32;
        core::ptr::write_volatile(apb1enr, core::ptr::read_volatile(apb1enr) | (1 << 21)); // I2C1EN

        // Configure PB6 (SCL) and PB7 (SDA) as AF open-drain with pull-up
        let crl = 0x4001_0C00 as *mut u32;
        let mut v = core::ptr::read_volatile(crl);
        // PB6: bits 24-27 = 0xB (AF open-drain, 50MHz)
        v &= !(0xF << 24);
        v |= 0xB << 24;
        // PB7: bits 28-31 = 0xB (AF open-drain, 50MHz)
        v &= !(0xF << 28);
        v |= 0xB << 28;
        core::ptr::write_volatile(crl, v);
        // Enable internal pull-ups
        let odr = 0x4001_0C0C as *mut u32;
        core::ptr::write_volatile(odr, core::ptr::read_volatile(odr) | (1 << 6) | (1 << 7));

        // Disable I2C during config
        core::ptr::write_volatile(I2C1_CR1, 0);

        // Set own address (7-bit): address << 1, OA1EN bit 15... actually bit 14 on F1
        // OAR1: ADD[7:1] in bits 1-7, ADD0 in bit 0, ADDMODE bit 15
        core::ptr::write_volatile(I2C1_OAR1, (I2C_TARGET_ADDRESS as u32) << 1 | (1 << 14)); // OA1EN

        // Configure timing for 100kHz with 8 MHz APB1
        // CCR = f_APB1 / (2 * f_SCL) = 8MHz / 200kHz = 40
        core::ptr::write_volatile(I2C1_CCR, 40);
        // TRISE = (f_APB1 / 1MHz) + 1 = 8 + 1 = 9
        core::ptr::write_volatile(I2C1_TRISE, 9);

        // CR2: FREQ = 8 MHz, enable event/buffer/error interrupts
        core::ptr::write_volatile(I2C1_CR2, (8 << 0) | (1 << 8) | (1 << 9) | (1 << 10));

        // CR1: PE=1, ACK=1, NOSTRETCH=0 (clock stretching enabled)
        core::ptr::write_volatile(I2C1_CR1, (1 << 0) | (1 << 10)); // PE | ACK
    }
}

fn i2c_irq_enable() {
    unsafe {
        // I2C1_EV is IRQ 31 -> ISER0 bit 31
        let iser0 = 0xE000_E100 as *mut u32;
        core::ptr::write_volatile(iser0, core::ptr::read_volatile(iser0) | (1 << 31));
        // I2C1_ER is IRQ 32 -> ISER1 bit 0
        let iser1 = 0xE000_E104 as *mut u32;
        core::ptr::write_volatile(iser1, core::ptr::read_volatile(iser1) | (1 << 0));
    }
}

fn uart_put_byte(byte: u8) {
    unsafe {
        TX_BUSY.store(true, Ordering::Relaxed);
        let sr = 0x4001_3800 as *const u32;
        let dr = 0x4001_3804 as *mut u32;
        while core::ptr::read_volatile(sr) & (1 << 7) == 0 {}
        core::ptr::write_volatile(dr, byte as u32);
        // Wait for transmission to complete so echo doesn't leak back
        while core::ptr::read_volatile(sr) & (1 << 6) == 0 {}
        TX_BUSY.store(false, Ordering::Relaxed);
    }
}

fn uart_get_byte() -> Option<u8> {
    // Pop from the ISR-filled ring buffer.
    unsafe {
        let rd = core::ptr::read_volatile(core::ptr::addr_of!(RX_RING_RD));
        let wr = core::ptr::read_volatile(core::ptr::addr_of!(RX_RING_WR));
        if rd != wr {
            let b = core::ptr::read_volatile(core::ptr::addr_of!(RX_RING[rd]));
            core::ptr::write_volatile(core::ptr::addr_of_mut!(RX_RING_RD), (rd + 1) % RX_RING_SIZE);
            Some(b)
        } else {
            None
        }
    }
}

const RX_RING_SIZE: usize = 256;
static mut RX_RING: [u8; RX_RING_SIZE] = [0; RX_RING_SIZE];
static mut RX_RING_WR: usize = 0;
static mut RX_RING_RD: usize = 0;

// I2C buffers (Katapult-compatible sizes)
const I2C_RX_BUF_SIZE: usize = 192;
const I2C_TX_BUF_SIZE: usize = 128;
static mut I2C_RX_BUF: [u8; I2C_RX_BUF_SIZE] = [0; I2C_RX_BUF_SIZE];
static mut I2C_RX_POS: usize = 0;
static mut I2C_TX_BUF: [u8; I2C_TX_BUF_SIZE] = [0; I2C_TX_BUF_SIZE];
static mut I2C_TX_POS: usize = 0;
static mut I2C_TX_MAX: usize = 0;
static mut I2C_STOP_RECEIVED: bool = false;

/// RX ring filled by the USART1 interrupt so no byte is ever lost while the
/// main thread blocks (page erase/program can hold the CPU for tens of ms).

static TX_BUSY: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn USART1() {
    let sr = 0x4001_3800 as *const u32;
    let dr = 0x4001_3804 as *const u32;
    let status = core::ptr::read_volatile(sr);
    if status & (1 << 5) != 0 {
        let b = core::ptr::read_volatile(dr) as u8;
        if TX_BUSY.load(Ordering::Relaxed) {
            return;
        }
        let wr = core::ptr::read_volatile(core::ptr::addr_of!(RX_RING_WR));
        let rd = core::ptr::read_volatile(core::ptr::addr_of!(RX_RING_RD));
        let next = (wr + 1) % RX_RING_SIZE;
        if next != rd {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(RX_RING[wr]), b);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(RX_RING_WR), next);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn I2C1_EV() {
    let sr1 = core::ptr::read_volatile(I2C1_SR1);
    let sr2 = core::ptr::read_volatile(I2C1_SR2);

    // Address matched
    if sr1 & (1 << 1) != 0 {
        // Clear ADDR flag by reading SR1 then SR2
        let _ = core::ptr::read_volatile(I2C1_SR1);
        let _ = core::ptr::read_volatile(I2C1_SR2);

        if sr2 & (1 << 2) != 0 {
            // Master is reading (TRA=1): slave transmit mode
            let tpos = core::ptr::read_volatile(core::ptr::addr_of!(I2C_TX_POS));
            let tmax = core::ptr::read_volatile(core::ptr::addr_of!(I2C_TX_MAX));
            if tpos < tmax {
                // Enable TXIE to stream response
                let cr1 = core::ptr::read_volatile(I2C1_CR1);
                core::ptr::write_volatile(I2C1_CR1, cr1 | (1 << 7)); // TXIE... wait, on F1 TXIE is bit 7 in CR2? No.
                // Actually on F1, TxE interrupt is controlled by ITBUFEN in CR2
                // ITBUFEN is already enabled. TXE flag in SR1 triggers interrupt.
            }
        } else {
            // Master is writing: disable TX (not needed, ITBUFEN handles RXNE)
        }
    }

    // Byte received (RXNE)
    if sr1 & (1 << 6) != 0 {
        let data = core::ptr::read_volatile(I2C1_DR) as u8;
        let rpos = core::ptr::read_volatile(core::ptr::addr_of!(I2C_RX_POS));
        if rpos < I2C_RX_BUF_SIZE {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_RX_BUF[rpos]), data);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_RX_POS), rpos + 1);
        }
    }

    // Data register empty (TxE) - master is reading
    if sr1 & (1 << 7) != 0 {
        let tpos = core::ptr::read_volatile(core::ptr::addr_of!(I2C_TX_POS));
        let tmax = core::ptr::read_volatile(core::ptr::addr_of!(I2C_TX_MAX));
        if tpos < tmax {
            let byte = core::ptr::read_volatile(core::ptr::addr_of!(I2C_TX_BUF[tpos]));
            core::ptr::write_volatile(I2C1_DR, byte as u32);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_TX_POS), tpos + 1);
        } else {
            // Buffer empty: send 0xFF dummy
            core::ptr::write_volatile(I2C1_DR, 0xFF);
        }
    }

    // NACK received (AF)
    if sr1 & (1 << 10) != 0 {
        // Clear AF flag
        let sr1_val = core::ptr::read_volatile(I2C1_SR1);
        core::ptr::write_volatile(I2C1_SR1, sr1_val & !(1 << 10));
    }

    // STOP condition detected
    if sr1 & (1 << 4) != 0 {
        // Clear STOPF by reading SR1 then writing CR1
        let _ = core::ptr::read_volatile(I2C1_SR1);
        let cr1 = core::ptr::read_volatile(I2C1_CR1);
        core::ptr::write_volatile(I2C1_CR1, cr1);
        // Set stop received flag
        core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_STOP_RECEIVED), true);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn I2C1_ER() {
    let sr1 = core::ptr::read_volatile(I2C1_SR1);
    // Clear error flags
    if sr1 & (1 << 8) != 0 {  // BERR
        core::ptr::write_volatile(I2C1_SR1, sr1 & !(1 << 8));
    }
    if sr1 & (1 << 9) != 0 {  // ARLO
        core::ptr::write_volatile(I2C1_SR1, sr1 & !(1 << 9));
    }
    if sr1 & (1 << 11) != 0 {  // OVR
        core::ptr::write_volatile(I2C1_SR1, sr1 & !(1 << 11));
    }
}

#[entry]
fn main() -> ! {
    uart_init();
    uart_irq_enable();
    i2c_init();
    i2c_irq_enable();
    dwt_cycle_counter_init();
    rtt::rtt_init();
    rtt::rtt_line!("[bb] boot");
    let mut flash = BluepillFlash;
    BluepillFlash::unlock();
    let cfg = bareboot_core::BootConfig {
        protocol_version: 0x00010100,
        app_base: 0x0800_2000,
        block_size: 64,
        mcu_type: b"stm32f103c8",
        software_version: b"dev",
        uid: [0; 12],
    };
    let mut writer = PagedWriter::<BluepillFlash, 1024>::new(flash, cfg);

    let mut rx = [0u8; 256];
    let mut rx_len = 0usize;
    let mut elapsed_ms = 0u32;
    let mut last_ms_at = cycle_count();
    let mut last_byte_ms = 0u32;
    let boot_window = 3000u32;

    loop {
        // Millisecond tick from the free-running cycle counter.
        let now = cycle_count();
        if (now.wrapping_sub(last_ms_at)) >= dwt_cycles_per_ms() {
            last_ms_at = last_ms_at.wrapping_add(dwt_cycles_per_ms());
            elapsed_ms = elapsed_ms.saturating_add(1);
        }

        // Drain all available bytes first.
        let mut got_bytes = false;
        while let Some(byte) = uart_get_byte() {
            rx_len = rx_len.min(rx.len() - 1);
            rx[rx_len] = byte;
            rx_len += 1;
            got_bytes = true;
            last_byte_ms = elapsed_ms;
            let mut hx = [0u8; 2];
            hex2(byte, &mut hx);
            rtt::rtt_write(&hx);
        }

        // Idle resync: discard a partial frame if no bytes arrive for a
        // while (e.g. host stopped sending). Use a longer timeout to allow
        // for paced sends.
        if rx_len > 0 && elapsed_ms.saturating_sub(last_byte_ms) > 100 {
            rx_len = 0;
            rtt::rtt_line!(" resync");
        }

        // I2C: check if a STOP condition was received
        let i2c_stop = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(I2C_STOP_RECEIVED)) };
        if i2c_stop {
            // Process I2C received data
            let rx_pos = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(I2C_RX_POS)) };
            if rx_pos > 0 {
                // Copy RX buffer to local rx for processing
                for i in 0..rx_pos.min(256) {
                    rx[i] = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(I2C_RX_BUF[i])) };
                }
                rx_len = rx_pos.min(256);
            }
            // Clear I2C stop flag
            unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_STOP_RECEIVED), false); }
        }

        // If we have at least a header, try to collect the rest of the frame.
        if rx_len >= 4 {
            let wc = rx[3] as usize;
            let frame_len = 4 + wc * 4 + 4;
            if frame_len <= rx.len() {
                while rx_len < frame_len {
                    if let Some(byte) = uart_get_byte() {
                        if rx_len < rx.len() {
                            rx[rx_len] = byte;
                            rx_len += 1;
                            rtt::rtt_write(b"\n");
                            let mut hx = [0u8; 2];
                            hex2(byte, &mut hx);
                            rtt::rtt_write(&hx);
                        }
                    } else {
                        break;
                    }
                }
            } else {
                rtt::rtt_line!(" overflow guard wc={}", wc);
                rx_len = 0;
            }
        }

        let mut consumed = 0;
        while consumed < rx_len {
            match bareboot_protocol::scan(&rx[consumed..rx_len]) {
                FrameParse::Complete { frame_len, command, .. } => {
                    let mut out = [0u8; 256];
                    let res = if command == bareboot_protocol::commands::CONNECT {
                        dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                    } else if command == bareboot_protocol::commands::RX_BLOCK {
                        dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                    } else if command == bareboot_protocol::commands::RX_EOF {
                        dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                    } else if command == bareboot_protocol::commands::COMPLETE {
                        dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                    } else {
                        Ok(0)
                    };
                    if let Ok(n) = res {
                        if i2c_stop {
                            // Copy response to I2C TX buffer
                            unsafe {
                                core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_TX_POS), 0);
                                core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_TX_MAX), n);
                                for i in 0..n {
                                    core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_TX_BUF[i]), out[i]);
                                }
                            }
                        } else {
                            for b in &out[..n] { uart_put_byte(*b); }
                        }
                        rtt::rtt_line!(" rep n={}", n);
                    } else {
                        rtt::rtt_line!(" dispatch err");
                    }
                    consumed += frame_len;
                }
                FrameParse::Invalid { drop } => {
                    rtt::rtt_line!("dump: len={} c={}", rx_len, consumed);
                    let mut k = 0;
                    while k < rx_len && k < 160 {
                        let mut b2 = [0u8; 2];
                        hex2(rx[k], &mut b2);
                        rtt::rtt_write(&b2);
                        k += 1;
                    }
                    rtt::rtt_write(b"\n");
                    rtt::rtt_line!(" invalid drop={}", drop);
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
            if consumed <= available {
                rx_len = available - consumed;
            } else {
                rx_len = 0;
            }
        }
        // If I2C was used, clear the I2C RX buffer
        if i2c_stop {
            unsafe {
                core::ptr::write_volatile(core::ptr::addr_of_mut!(I2C_RX_POS), 0);
            }
        }

        if writer.is_completed() {
            delay_ms(100);
            let app_sp = unsafe { core::ptr::read_volatile(0x0800_2000 as *const u32) };
            if app_sp >= 0x2000_0000 && app_sp <= 0x2000_5000 {
                unsafe { BluepillFlash::boot_jump(0x08002000) }
            }
        }

        if rx_len == 0 && elapsed_ms >= boot_window {
            let app_sp = unsafe { core::ptr::read_volatile(0x0800_2000 as *const u32) };
            // Only boot a plausible vector table: initial SP must land inside SRAM.
            if app_sp >= 0x2000_0000 && app_sp <= 0x2000_5000 {
                unsafe { BluepillFlash::boot_jump(0x08002000) }
            }
        }
    }
}

/// Enable the Cortex-M3 DWT cycle counter so timing can be derived from a
/// free-running counter instead of spin loops inside the receive loop.
fn dwt_cycle_counter_init() {
    unsafe {
        const DEMCR: *mut u32 = 0xE000_EDFC as *mut u32;
        const DWT_CTRL: *mut u32 = 0xE000_1000 as *mut u32;
        const DWT_CYCCNT: *mut u32 = 0xE000_1004 as *mut u32;
        core::ptr::write_volatile(DEMCR, core::ptr::read_volatile(DEMCR) | (1 << 24));
        core::ptr::write_volatile(DWT_CYCCNT, 0);
        core::ptr::write_volatile(DWT_CTRL, core::ptr::read_volatile(DWT_CTRL) | 1);
    }
}

#[inline]
fn cycle_count() -> u32 {
    unsafe { core::ptr::read_volatile(0xE000_1004 as *const u32) }
}

/// SYSCLK runs at the default HSI 8 MHz on this target (BRR 0x45 is tuned
/// for it), so one millisecond is 8000 core cycles.
#[inline]
fn dwt_cycles_per_ms() -> u32 { 8000 }

#[cortex_m_rt::exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    let pc = ef.pc();
    unsafe {
        uart_put_byte(b'H');
        uart_put_byte((pc >> 24) as u8);
        uart_put_byte((pc >> 16) as u8);
        uart_put_byte((pc >> 8) as u8);
        uart_put_byte(pc as u8)
    };
    loop { let _ = core::hint::black_box(0); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
