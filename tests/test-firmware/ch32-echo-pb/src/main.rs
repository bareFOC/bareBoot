#![no_std]
#![no_main]
use core::ptr;
use riscv_rt::entry;
use panic_halt as _;
fn uart_init() {
  unsafe {
    let apb2 = 0x4002_1018 as *mut u32;
    core::ptr::write_volatile(apb2, core::ptr::read_volatile(apb2) | (1<<0)|(1<<2)|(1<<3)|(1<<14));
    // remap USART1 to PB6/PB7
    let pcfr1 = 0x4001_0004 as *mut u32;
    core::ptr::write_volatile(pcfr1, core::ptr::read_volatile(pcfr1) | (1<<2));
    // PB6 TX alt PP 50MHz: CR[L/H?]: PB6 is in CRL? actually PB0-7 CRL, bits 24..27
    let crl = 0x4001_0c00 as *mut u32;
    let mut v = core::ptr::read_volatile(crl);
    v &= !(0xF << 24); v |= 0xB << 24; // PB6 TX
    v &= !(0xF << 28); v |= 0x4 << 28; // PB7 RX floating
    core::ptr::write_volatile(crl, v);
    // Baud 115200 @ 8 MHz
    core::ptr::write_volatile(0x4001_3808 as *mut u32, 0x0045);
    core::ptr::write_volatile(0x4001_380C as *mut u32, (1<<13)|(1<<3)|(1<<2));
  }
}
fn put(b:u8){ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *mut u32; while core::ptr::read_volatile(sr)&(1<<7)==0{} core::ptr::write_volatile(dr,b as u32);} }
fn get()->Option<u8>{ unsafe { let sr=0x4001_3800 as *const u32; let dr=0x4001_3804 as *const u32; if core::ptr::read_volatile(sr)&(1<<5)!=0 { Some(core::ptr::read_volatile(dr) as u8)} else {None}} }
#[entry]
fn main() -> ! {
  uart_init();
  put(b'A'); put(b'_'); put(b'P'); put(b'B');
  loop {
    if let Some(b)=get() { put(b); }
  }
}
