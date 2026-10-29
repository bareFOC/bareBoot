#![no_std]
#![no_main]
use core::ptr;
use cortex_m_rt::entry;
use panic_halt as _;
fn unlock(){ unsafe { core::ptr::write_volatile(0x4002_2004 as *mut u32,0x45670123); core::ptr::write_volatile(0x4002_2004 as *mut u32,0xCDEF89AB); } }
fn delay(){ for _ in 0..100000 { let _=core::hint::black_box(0);} }
fn uart_get_byte()->Option<u8>{ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *const u32; if core::ptr::read_volatile(sr)&(1<<5)!=0 {Some(core::ptr::read_volatile(dr) as u8)} else {None}} }
fn uart_init(){ unsafe {
 let apb2=0x4002_1018 as *mut u32; core::ptr::write_volatile(apb2, core::ptr::read_volatile(apb2)|(1<<2)|(1<<14));
 let crh=0x4001_0804 as *mut u32; let mut v=core::ptr::read_volatile(crh); v&=!(0xF<<4); v|=0xB<<4; v&=!(0xF<<8); v|=0x4<<8; core::ptr::write_volatile(crh,v);
 let brr=0x4001_3808 as *mut u32; core::ptr::write_volatile(brr,0x0045);
 let cr1=0x4001_380C as *mut u32; core::ptr::write_volatile(cr1,(1<<13)|(1<<3)|(1<<2));
}}
fn put(b:u8){ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *mut u32; while core::ptr::read_volatile(sr)&(1<<7)==0{} core::ptr::write_volatile(dr,b as u32);} }
#[entry]
fn main()->! {
  unlock();
  let addr=0x0800_1000u32;
  uart_init();
  // erase
  unsafe {
    core::ptr::write_volatile(0x4002_2010 as *mut u32,2);
    core::ptr::write_volatile(0x4002_2014 as *mut u32,addr);
    core::ptr::write_volatile(0x4002_2010 as *mut u32,0x00000042);
    while core::ptr::read_volatile(0x4002_200C as *const u32)&1!=0 {}
    core::ptr::write_volatile(0x4002_2010 as *mut u32,0);
  }
  let mut data=[0u8;1024]; for (i,x) in data.iter_mut().enumerate(){ *x = if i%2==0 {0x34} else {0x12}; }
  unlock();
  unsafe {
    const CR:*mut u32=0x4002_2010 as *mut u32; const SR:*const u32=0x4002_200C as *const u32;
    for i in (0..data.len()).step_by(2) {
      core::ptr::write_volatile(CR,1);
      let w=u16::from_le_bytes([data[i],data[i+1]]);
      core::ptr::write_volatile((addr+i as u32) as *mut u16, w);
      while core::ptr::read_volatile(SR)&1!=0 {}
      core::ptr::write_volatile(CR,0);
    }
  }
  delay();
  let flash=unsafe{core::ptr::read_volatile(addr as *const u32)};
  if flash==0x12341234 { put(b'K'); } else { put(b'F'); }
  loop { if let Some(b)=uart_get_byte() { put(b); } }
}
