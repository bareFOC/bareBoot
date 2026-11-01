#![no_std]
#![no_main]
mod buffer;
mod usbfs;
use panic_halt as _;
use riscv_rt::entry;
use crate::buffer::HostResponse;
use crate::usbfs::Regs;

#[entry]
fn main() -> ! {
    let _regs = Regs::new();
    let _resp = HostResponse::default();
    usbfs::init(&_regs);
    loop { core::hint::spin_loop(); }
}
