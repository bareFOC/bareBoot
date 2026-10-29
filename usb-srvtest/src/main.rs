#![no_std]
#![no_main]

use panic_halt as _;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::usb::{Peripheral, UsbBus};
use usbd_serial::SerialPort;
use usb_device::device::{UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usbd_serial::USB_CLASS_CDC;

#[cortex_m_rt::entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.freeze(
        stm32f1xx_hal::rcc::Config::hse(8.MHz()).sysclk(48.MHz()).pclk1(24.MHz()),
        &mut flash.acr,
    );
    assert!(rcc.clocks.usbclk_valid());

    let mut gpioa = dp.GPIOA.split(&mut rcc);
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    // small delay
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
    loop {
        usb_dev.poll(&mut [&mut serial]);
        let mut buf = [0u8; 64];
        let n = serial.read(&mut buf).unwrap_or(0);
        if n > 0 {
            let _ = serial.write(&buf[..n]);
        }
    }
}
