#![no_std]
#![no_main]
use core::ptr;
use cortex_m_rt::entry;
use panic_halt as _;
fn unlock() { unsafe {
    core::ptr::write_volatile(0x4002_2004 as *mut u32, 0x4567_0123);
    core::ptr::write_volatile(0x4002_2004 as *mut u32, 0xCDEF_89AB);
}}
fn erase(addr: u32){
    unsafe {
        core::ptr::write_volatile(0x4002_2010 as *mut u32, 2);
        core::ptr::write_volatile(0x4002_2014 as *mut u32, addr);
        core::ptr::write_volatile(0x4002_2010 as *mut u32, 0x00000042);
        while core::ptr::read_volatile(0x4002_200C as *const u32)&1 !=0 {}
        core::ptr::write_volatile(0x4002_2010 as *mut u32,0);
    }
}
fn program_half(addr:u32,val:u16){
    unlock();
    unsafe {
        const CR:*mut u32=0x4002_2010 as *mut u32; const SR:*const u32=0x4002_200C as *const u32;
        core::ptr::write_volatile(CR,1);
        core::ptr::write_volatile(addr as *mut u16, val);
        while core::ptr::read_volatile(SR)&1!=0 {}
        core::ptr::write_volatile(CR,0);
    }
}
#[entry]
fn main()->! {
    unlock();
    let addr=0x0800_1000u32;
    unsafe { core::ptr::write_volatile(0x2000_0000 as *mut u32, core::ptr::read_volatile(0x4002_2010 as *const u32)); } // CR lock? before
    erase(addr);
    for i in 0..2 {
        program_half(addr+i*2, 0x5555);
    }
    unsafe {
        core::ptr::write_volatile(0x2000_0004 as *mut u32, core::ptr::read_volatile(0x4002_2010 as *const u32));
        core::ptr::write_volatile(0x2000_0008 as *mut u32, core::ptr::read_volatile(0x0800_1000 as *const u32));
    }
    loop { for _ in 0..10000 { let _=core::hint::black_box(0);} }
}
