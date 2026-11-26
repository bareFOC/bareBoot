#![no_std]

use bareboot_core::ArchBoot;
use core::ptr::{read_volatile, write_volatile};

const SYST_CSR: *mut u32 = 0xE000_E010 as *mut u32;
const SYST_CVR: *mut u32 = 0xE000_E018 as *mut u32;
const NVIC_ICER_BASE: *mut u32 = 0xE000_E180 as *mut u32;
const NVIC_ICPR_BASE: *mut u32 = 0xE000_E280 as *mut u32;
const SCB_VTOR: *mut u32 = 0xE000_ED08 as *mut u32;
const SCB_AIRCR: *mut u32 = 0xE000_ED0C as *mut u32;
const AIRCR_VECTKEY_SYSRESETREQ: u32 = 0x05FA_0004;

/// ARM Cortex-M `ArchBoot` implementation.
pub struct CortexMArch;

impl ArchBoot for CortexMArch {
    #[inline]
    fn is_valid_pc(pc: u32, app_base: u32, app_end: u32) -> bool {
        pc != 0 && pc != 0xFFFF_FFFF && (pc & 1) == 1 && pc >= app_base && pc < app_end
    }

    unsafe fn jump_to_app(app_base: u32) -> ! {
        cortex_m::interrupt::disable();
        unsafe {
            // Disable SysTick
            write_volatile(SYST_CSR, 0);
            write_volatile(SYST_CVR, 0);

            // Clear all NVIC interrupt enables & pending flags
            for i in 0..8 {
                write_volatile(NVIC_ICER_BASE.add(i), 0xFFFF_FFFF);
                write_volatile(NVIC_ICPR_BASE.add(i), 0xFFFF_FFFF);
            }

            let sp = read_volatile(app_base as *const u32);
            let reset_handler = read_volatile((app_base + 4) as *const u32);

            write_volatile(SCB_VTOR, app_base);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();

            // Restore reset-default PRIMASK=0 before entering the application;
            // all NVIC and SysTick sources are already disabled above.
            cortex_m::interrupt::enable();

            #[cfg(target_arch = "arm")]
            cortex_m::asm::bootstrap(sp as *const u32, reset_handler as *const u32);

            #[cfg(not(target_arch = "arm"))]
            {
                let _ = (sp, reset_handler);
                loop {
                    core::hint::spin_loop();
                }
            }
        }
    }

    fn system_reset() -> ! {
        cortex_m::interrupt::disable();
        unsafe {
            write_volatile(SCB_AIRCR, AIRCR_VECTKEY_SYSRESETREQ);
        }
        loop {
            core::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_bit_and_bounds_check() {
        assert!(CortexMArch::is_valid_pc(0x0800_4101, 0x0800_4000, 0x0801_A000));
        assert!(!CortexMArch::is_valid_pc(0x0800_4100, 0x0800_4000, 0x0801_A000));
        assert!(!CortexMArch::is_valid_pc(0x0800_0101, 0x0800_4000, 0x0801_A000));
        assert!(!CortexMArch::is_valid_pc(0x0801_A001, 0x0800_4000, 0x0801_A000));
        assert!(!CortexMArch::is_valid_pc(0, 0x0800_4000, 0x0801_A000));
        assert!(!CortexMArch::is_valid_pc(0xFFFF_FFFF, 0x0800_4000, 0x0801_A000));
    }
}
