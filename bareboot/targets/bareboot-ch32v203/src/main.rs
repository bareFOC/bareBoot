#![no_std]
#![no_main]

use bareboot_core::{BootConfig, FlashTarget, PagedWriter};
use bareboot_core::dispatch::dispatch_one;
use bareboot_protocol;
use core::panic::PanicInfo;
use riscv_rt::entry;

struct BluepillLikeFlash;
impl BluepillLikeFlash {
    fn unlock() {
        unsafe {
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0x4567_0123);
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0xCDEF_89AB);
        }
    }
}
impl FlashTarget for BluepillLikeFlash {
    fn page_size(&self) -> usize { 256 }
    fn app_base(&self) -> u32 { 0x0800_1000 }
    fn erase_page(&mut self, addr: u32) -> Result<(), bareboot_core::CoreError> {
        unsafe {
            const KEYR: *mut u32 = 0x4002_2004 as *mut u32;
            const MODEKEYR: *mut u32 = 0x4002_2008 as *mut u32;
            const CR: *mut u32 = 0x4002_2010 as *mut u32;
            const AR: *mut u32 = 0x4002_2014 as *mut u32;
            const SR: *const u32 = 0x4002_200C as *const u32;
            core::ptr::write_volatile(KEYR, 0x4567_0123);
            core::ptr::write_volatile(KEYR, 0xCDEF_89AB);
            core::ptr::write_volatile(MODEKEYR, 0x4567_0123);
            core::ptr::write_volatile(MODEKEYR, 0xCDEF_89AB);
            core::ptr::write_volatile(CR, 0x0002_0000); // CR_PAGE_ER
            core::ptr::write_volatile(AR, addr);
            core::ptr::write_volatile(CR, 0x0002_0040); // CR_PAGE_ER | CR_PG_STRT
            while core::ptr::read_volatile(SR) & 0x01 != 0 {}
            core::ptr::write_volatile(CR, 0);
            // Clear status (EOP / WRPRTERR)
            core::ptr::write_volatile(0x4002_200C as *mut u32, core::ptr::read_volatile(SR) | (1 << 5) | (1 << 4));
            // Lock again
            core::ptr::write_volatile(CR, (1 << 7) | (1 << 8)); // LOCK | FAST_LOCK
        }
        Ok(())
    }
    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), bareboot_core::CoreError> {
        if data.len() % 4 != 0 {
            return Err(bareboot_core::CoreError::Flash);
        }
        // Use an aligned 32-bit staging buffer, then do the CH32 fast 64-word page program.
        let mut words = [0u32; 64];
        let mut i = 0usize;
        while i < 64 {
            words[i] = u32::from_le_bytes([
                data[i * 4],
                data[i * 4 + 1],
                data[i * 4 + 2],
                data[i * 4 + 3],
            ]);
            i += 1;
        }

        unsafe {
            let keyr = 0x4002_2004 as *mut u32;
            let modekeyr = 0x4002_2008 as *mut u32;
            let cr = 0x4002_2010 as *mut u32;
            let statr = 0x4002_200C as *const u32;
            core::ptr::write_volatile(keyr, 0x4567_0123);
            core::ptr::write_volatile(keyr, 0xCDEF_89AB);
            core::ptr::write_volatile(modekeyr, 0x4567_0123);
            core::ptr::write_volatile(modekeyr, 0xCDEF_89AB);

            core::ptr::write_volatile(cr, 0x0001_0000); // CR_PAGE_PG
            let dst = addr as *mut u32;
            for idx in 0..64 {
                core::ptr::write_volatile(dst.add(idx), words[idx]);
                while core::ptr::read_volatile(statr) & 0x02 != 0 {} // WRBSY
            }

            core::ptr::write_volatile(cr, 0x0021_0000); // CR_PAGE_PG | CR_PG_STRT
            while core::ptr::read_volatile(statr) & 0x01 != 0 {} // BSY
            let stat = core::ptr::read_volatile(statr);
            core::ptr::write_volatile(0x4002_200C as *mut u32, stat | (1 << 5) | (1 << 4));
            core::ptr::write_volatile(cr, 0); // Clear FTPG
            core::ptr::write_volatile(cr, (1 << 7) | (1 << 8)); // Lock
        }
        Ok(())
    }
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), bareboot_core::CoreError> {
        for (i,b) in out.iter_mut().enumerate() { *b = unsafe { core::ptr::read_volatile((addr + i as u32) as *const u8 ) }; }
        Ok(())
    }
    fn read_uid(&mut self, out: &mut [u8; 12]) {}
    unsafe fn boot_jump(app_entry: u32) -> ! {
        unsafe { core::arch::asm!("csrw mtvec, {}", in(reg) app_entry); }
unsafe { core::arch::asm!("fence.i"); }
        let sp = unsafe { core::ptr::read_volatile(app_entry as *const u32) };
        let reset = unsafe { core::ptr::read_volatile((app_entry + 4) as *const u32) };
unsafe { core::arch::asm!("jr {}", in(reg) reset); }
        loop {}
    }
}

fn delay_ms(_ms: u32) { for _ in 0..8000 { core::hint::black_box(0); } }

fn uart_init() {
    unsafe {
        let apb2 = 0x4002_1018 as *mut u32;
        core::ptr::write_volatile(apb2, core::ptr::read_volatile(apb2) | (1<<0) | (1<<2) | (1<<14));
        let cfg = 0x4001_0804 as *mut u32;
        let mut v = core::ptr::read_volatile(cfg);
        v &= !(0xF<<4); v |= 0xB<<4; // PA9 TX
        v &= !(0xF<<8); v |= 0x8<<8; // PA10 RX pull-up
        core::ptr::write_volatile(cfg, v);
        core::ptr::write_volatile(0x4001_0810 as *mut u32, core::ptr::read_volatile(0x4001_0810 as *const u32) | (1<<10));
        let brr = 0x4001_3808 as *mut u32;
        core::ptr::write_volatile(brr, 0x0045);
        let cr1 = 0x4001_380C as *mut u32;
        core::ptr::write_volatile(cr1, (1<<13)|(1<<3)|(1<<2));
    }
}
fn put(b:u8){ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *mut u32; while core::ptr::read_volatile(sr)&(1<<7)==0{} core::ptr::write_volatile(dr,b as u32);} }
fn get()->Option<u8>{ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *const u32; if core::ptr::read_volatile(sr)&(1<<5)!=0 {Some(core::ptr::read_volatile(dr) as u8)} else {None}} }

#[entry]
fn main() -> ! {
    uart_init();
    put(b'B'); put(b'o'); put(b'o'); put(b't'); put(b'\n');
    let mut flash = BluepillLikeFlash;
    BluepillLikeFlash::unlock();
    let cfg = BootConfig { protocol_version:0x00010100, app_base:0x0800_1000, block_size:64, mcu_type:b"ch32v203c8", software_version:b"dev", uid:[0;12] };
    let mut writer = PagedWriter::<BluepillLikeFlash, 256>::new(flash, cfg);
    let mut rx=[0u8;256]; let mut rx_len=0usize; let mut elapsed=0u32; let window=3000u32;
    loop {
        while let Some(b)=get() {
            if rx_len < rx.len() { rx[rx_len]=b; rx_len+=1; }
        }
        if rx_len>=4 {
            let wc=rx[3] as usize; let fl=4+wc*4+4;
            if fl <= rx.len() && rx_len<fl {
                while rx_len<fl {
                    if let Some(b)=get(){ if rx_len<rx.len(){rx[rx_len]=b;rx_len+=1;} } else { break; }
                }
            }
        }
        let mut consumed=0;
        while consumed < rx_len {
            match scan_wrapper(&rx[consumed..rx_len]) {
                FrameParse::Complete { frame_len, .. } => {
                    let mut out=[0u8;256];
                    if let Ok(n)=dispatch_one(&mut writer,&rx[consumed..consumed+frame_len],&mut out){
                        for b in &out[..n] { put(*b); }
                    } else {
                    }
                    consumed+=frame_len;
                }
                FrameParse::Invalid { drop } => consumed += drop.max(1),
                FrameParse::NeedMore => break,
            }
        }
        if consumed>0 { for i in 0..(rx_len-consumed){ rx[i]=rx[consumed+i]; } rx_len=rx_len.saturating_sub(consumed); }
        if writer.is_completed() {
            delay_ms(100);
            unsafe { BluepillLikeFlash::boot_jump(0x0800_1000) }
        }
        if rx_len==0 {
            elapsed=elapsed.saturating_add(1);
            delay_ms(1);
            if elapsed>=window && rx_len==0 {
                let app_sp=unsafe{ core::ptr::read_volatile(0x0800_1000 as *const u32) };
                if app_sp != 0xFFFFFFFF { unsafe { BluepillLikeFlash::boot_jump(0x0800_1000) } }
            }
        }
    }
}

use bareboot_protocol::FrameParse;
fn scan_wrapper(buf:&[u8])->bareboot_protocol::FrameParse { bareboot_protocol::scan(buf) }

#[panic_handler]
fn panic(_:&PanicInfo)->! { loop{} }
