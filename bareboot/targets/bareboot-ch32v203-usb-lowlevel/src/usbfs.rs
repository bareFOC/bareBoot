
use ch32_metapac::usb::Usb;

pub struct Regs { pub usb: Usb }

impl Regs {
    pub const fn new() -> Self { Self { usb: unsafe { Usb::from_ptr(0x4002_3400 as *mut _) } } }
}

pub fn init(_regs: &Regs) {
    // placeholder register configuration
}
