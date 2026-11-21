#![no_std]
#![no_main]
use bareboot_core::{BootConfig, PagedWriter};
use bareboot_core::FlashTarget;
use cortex_m_rt::entry;
use panic_halt as _;

struct Flash;
impl FlashTarget for Flash {
    fn page_size(&self)->usize { 1024 }
    fn app_base(&self)->u32 { 0x0800_1000 }
    fn erase_page(&mut self, addr: u32)->Result<(),bareboot_core::CoreError> {
        Bluepill::unlock();
        unsafe {
            const CR:*mut u32=0x4002_2010 as *mut u32;
            const AR:*mut u32=0x4002_2014 as *mut u32;
            const SR:*const u32=0x4002_200C as *const u32;
            core::ptr::write_volatile(CR,2);
            core::ptr::write_volatile(AR,addr);
            core::ptr::write_volatile(CR,0x42);
            while core::ptr::read_volatile(SR)&1!=0{}
            core::ptr::write_volatile(CR,0)
        }
        Ok(())
    }
    fn write_page_bytes(&mut self, addr: u32, data: &[u8])->Result<(),bareboot_core::CoreError> {
        Bluepill::unlock();
        unsafe {
            const CR:*mut u32=0x4002_2010 as *mut u32;
            const SR:*const u32=0x4002_200C as *const u32;
            for i in (0..data.len()).step_by(2){
                core::ptr::write_volatile(CR,1);
                let w=u16::from_le_bytes([data[i],data[i+1]]);
                core::ptr::write_volatile((addr+i as u32) as *mut u16,w);
                while core::ptr::read_volatile(SR)&1!=0{}
                core::ptr::write_volatile(CR,0);
            }
        }
        Ok(())
    }
    fn read_bytes(&mut self, addr:u32, out:&mut[u8])->Result<(),bareboot_core::CoreError> {
        for (i,b) in out.iter_mut().enumerate(){ *b=unsafe{core::ptr::read_volatile((addr+i as u32) as *const u8)}; }
        Ok(())
    }
    fn read_uid(&mut self, out:&mut [u8;12]){}
    unsafe fn boot_jump(_app_entry:u32)->! { loop{} }
}
struct Bluepill;
impl Bluepill { fn unlock(){ unsafe { core::ptr::write_volatile(0x4002_2004 as *mut u32,0x45670123); core::ptr::write_volatile(0x4002_2004 as *mut u32,0xCDEF89AB);} } }
#[entry]
fn main()->! {
    unsafe { core::ptr::write_volatile(0x4002_1018 as *mut u32, core::ptr::read_volatile(0x4002_1018 as *const u32) | 0x810 | 0x10); } // enable flash/RCC
    Bluepill::unlock();
    let flash=Flash;
    let cfg=BootConfig { protocol_version:0x00010100, app_base:0x0800_1000, app_end:0x0801_0000, block_size:64, mcu_type:b"stm32f103c8", software_version:b"dev", uid:[0;12] };
    let mut result: u32 = 0xDEAD;
    let mut writer=PagedWriter::<Flash,1024>::new(flash,cfg);
    let data: [u8;64]=[0x11;64];
    writer.write_block(0x0800_1000,&data).unwrap();
    match writer.finish() { Ok(p) => result = 0xC001 + p, Err(_) => result = 0xBAD0, }
    unsafe { core::ptr::write_volatile(0x2000_0000 as *mut u32, result); }
    loop {}
}
