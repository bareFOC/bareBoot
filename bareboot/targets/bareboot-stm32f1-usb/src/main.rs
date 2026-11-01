#![no_std]
#![no_main]

use bareboot_core::{BootConfig, FlashTarget, PagedWriter};
use bareboot_core::dispatch::dispatch_one;
use bareboot_protocol::{self, FrameParse};
use core::ptr;
use cortex_m_rt::entry;
use panic_halt as _;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::usb::{Peripheral, UsbBus};
use usbd_serial::SerialPort;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use usbd_serial::USB_CLASS_CDC;

struct Flash;

impl FlashTarget for Flash {
    fn page_size(&self) -> usize { 1024 }
    fn app_base(&self) -> u32 { 0x0800_1000 }
    fn erase_page(&mut self, addr: u32) -> Result<(), bareboot_core::CoreError> {
        unsafe {
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0x4567_0123);
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0xCDEF_89AB);
            core::ptr::write_volatile(0x4002_2010 as *mut u32, 2);
            core::ptr::write_volatile(0x4002_2014 as *mut u32, addr);
            core::ptr::write_volatile(0x4002_2010 as *mut u32, 0x00000042);
            while core::ptr::read_volatile(0x4002_200C as *const u32) & 1 != 0 {}
            core::ptr::write_volatile(0x4002_2010 as *mut u32, 0);
        }
        Ok(())
    }
    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), bareboot_core::CoreError> {
        unsafe {
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0x4567_0123);
            core::ptr::write_volatile(0x4002_2004 as *mut u32, 0xCDEF_89AB);
            const CR: *mut u32 = 0x4002_2010 as *mut u32;
            const SR: *const u32 = 0x4002_200C as *const u32;
            for i in (0..data.len()).step_by(2) {
                core::ptr::write_volatile(CR, 1);
                let word = u16::from_le_bytes([data[i], data[i + 1]]);
                core::ptr::write_volatile((addr + i as u32) as *mut u16, word);
                while core::ptr::read_volatile(SR) & 1 != 0 {}
                core::ptr::write_volatile(CR, 0);
            }
        }
        Ok(())
    }
    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), bareboot_core::CoreError> {
        for (i, b) in out.iter_mut().enumerate() {
            *b = unsafe { core::ptr::read_volatile((addr + i as u32) as *const u8) };
        }
        Ok(())
    }
    fn read_uid(&mut self, out: &mut [u8; 12]) {
        for i in 0..12 { out[i] = unsafe { core::ptr::read_volatile((0x1FFF_F7E8 + i) as *const u8) }; }
    }
    unsafe fn boot_jump(app_entry: u32) -> ! {
        core::arch::asm!("cpsid i");
        let sp = unsafe { core::ptr::read_volatile(app_entry as *const u32) };
        let reset = unsafe { core::ptr::read_volatile((app_entry + 4) as *const u32) };
        unsafe { core::arch::asm!("msr msp, {}", in(reg) sp); }
        let f: fn() -> ! = unsafe { core::mem::transmute(reset) };
        unsafe { f() }
    }
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.freeze(
        stm32f1xx_hal::rcc::Config::hse(8.MHz()).sysclk(48.MHz()).pclk1(24.MHz()),
        &mut flash.acr,
    );
    let mut gpioa = dp.GPIOA.split(&mut rcc);
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    for _ in 0..500000 { core::hint::black_box(0); }
    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: gpioa.pa11,
        pin_dp: usb_dp.into_floating_input(&mut gpioa.crh),
    };
    let usb_bus = UsbBus::new(usb);
    let mut serial = SerialPort::new(&usb_bus);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[usb_device::device::StringDescriptors::default()
            .manufacturer("Fake Company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    let mut flash = Flash;
    let cfg = BootConfig {
        protocol_version: 0x00010100,
        app_base: 0x0800_1000,
        block_size: 64,
        mcu_type: b"stm32f103c8",
        software_version: b"dev",
        uid: [0; 12],
    };
    let mut writer = PagedWriter::<Flash, 1024>::new(flash, cfg);

    let mut rx = [0u8; 256];
    let mut rx_len = 0usize;
    loop {
        usb_dev.poll(&mut [&mut serial]);
        let mut buf = [0u8; 64];
        if let Ok(n) = serial.read(&mut buf) {
            for i in 0..n {
                if rx_len < rx.len() { rx[rx_len] = buf[i]; rx_len += 1; }
            }
        }
        // gather more if possible
        let mut consumed = 0;
        while consumed < rx_len {
            match bareboot_protocol::scan(&rx[consumed..rx_len]) {
                FrameParse::Complete { frame_len, command, .. } => {
                    let mut out = [0u8; 256];
                    let res = dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out);
                    if let Ok(n) = res {
                        let mut sent = 0;
                        while sent < n {
                            usb_dev.poll(&mut [&mut serial]);
                            if let Ok(w) = serial.write(&out[sent..n]) { sent += w; }
                        }
                    } else {
                        // ignore
                    }
                    consumed += frame_len;
                }
                FrameParse::Invalid { drop } => consumed += drop.max(1),
                FrameParse::NeedMore => break,
            }
        }
        if consumed > 0 {
            for i in 0..rx_len - consumed {
                rx[i] = rx[consumed + i];
            }
            if consumed <= rx_len { rx_len -= consumed; } else { rx_len = 0; }
        }
        usb_dev.poll(&mut [&mut serial]);
        if writer.is_completed() {
            unsafe { Flash::boot_jump(0x0800_1000) }
        }
    }
}
