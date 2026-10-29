#![no_std]
#![no_main]

use core::ptr;
use cortex_m_rt::entry;
use panic_halt as _;

#[entry]
fn main() -> ! {
    // Enable GPIOC clock
    unsafe {
        let rcc = 0x40021018 as *mut u32;
        core::ptr::write_volatile(rcc, core::ptr::read_volatile(rcc) | 0x10);
    }
    // PC13 output 2MHz
    unsafe {
        let crh = 0x40011004 as *mut u32;
        core::ptr::write_volatile(crh, 0x00200000);
    }
    loop {
        unsafe {
            let odr = 0x4001100C as *mut u32;
            let v = core::ptr::read_volatile(odr);
            core::ptr::write_volatile(odr, v ^ 0x2000);
        }
        for _ in 0..500000 {
            let _ = core::hint::black_box(0);
        }
    }
}
