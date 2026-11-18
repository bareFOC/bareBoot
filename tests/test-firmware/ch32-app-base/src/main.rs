#![no_std]
#![no_main]
use core::ptr;
use riscv_rt::entry;
use panic_halt as _;
fn uart_init(){ unsafe {
  let apb2=0x4002_1018 as *mut u32;
  core::ptr::write_volatile(apb2, core::ptr::read_volatile(apb2)|(1<<0)|(1<<2)|(1<<14));
  let cfg=0x4001_0804 as *mut u32;
  let mut v=core::ptr::read_volatile(cfg);
  v&=!(0xF<<4); v|=0xB<<4; v&=!(0xF<<8); v|=0x4<<8;
  core::ptr::write_volatile(cfg,v);
  core::ptr::write_volatile(0x4001_0810 as *mut u32, core::ptr::read_volatile(0x4001_0810 as *const u32)|(1<<10));
  let brr=0x4001_3808 as *mut u32; core::ptr::write_volatile(brr,0x0045);
  let cr1=0x4001_380C as *mut u32; core::ptr::write_volatile(cr1,(1<<13)|(1<<3)|(1<<2));
}}
fn put(b:u8){ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *mut u32; while core::ptr::read_volatile(sr)&(1<<7)==0{} core::ptr::write_volatile(dr,b as u32);} }
fn get()->Option<u8>{ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *const u32; if core::ptr::read_volatile(sr)&(1<<5)!=0 { Some(core::ptr::read_volatile(dr) as u8)} else {None}} }
#[entry]
fn main()->! {
 uart_init();
 loop { if let Some(b)=get(){ put(b);} }
}
