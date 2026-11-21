#![no_std]
#![no_main]

use bareboot_core::{BootConfig, FlashTarget, PagedWriter};
use bareboot_core::dispatch::dispatch_one;
use bareboot_protocol::{self, FrameParse};
use core::hint::black_box;
use ch32_hal as hal;
use hal::{bind_interrupts, peripherals, Config, usb, usbd, rcc};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;
use panic_halt as _;

struct Flash;
impl FlashTarget for Flash {
    fn page_size(&self) -> usize { 256 }
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
        for i in 0..12 {
            out[i] = unsafe { core::ptr::read_volatile((0x1FFF_F7E8 + i) as *const u8) };
        }
    }
    unsafe fn boot_jump(app_entry: u32) -> ! {
        unsafe { core::arch::asm!("csrrw x0, mstatus, x0"); }
        unsafe { core::arch::asm!("csrw mtvec, {}", in(reg) app_entry); }
        unsafe { core::arch::asm!("fence.i"); }
        let reset = unsafe { core::ptr::read_volatile((app_entry + 4) as *const u32) };
        unsafe { core::arch::asm!("jr {}", in(reg) reset); }
        loop {}
    }
}

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => hal::usbd::InterruptHandler<hal::peripherals::USBD>;
});

#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        rcc: hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    });

    let driver = hal::usbd::Driver::new(p.USBD, Irqs, p.PA12, p.PA11);
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.product = Some("BareBoot USB");
    config.serial_number = Some("0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut state = embassy_usb::class::cdc_acm::State::new();

    let mut builder = embassy_usb::Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no MSOS descriptors
        &mut control_buf,
    );
    let mut class = embassy_usb::class::cdc_acm::CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();
    let mut class = class;

    let cfg = BootConfig {
        protocol_version: 0x00010100,
        app_base: 0x0800_1000,
        app_end: 0x0801_0000,
        block_size: 64,
        mcu_type: b"ch32v203c8",
        software_version: b"dev",
        uid: [0; 12],
    };
    let mut writer = PagedWriter::<Flash, 256>::new(Flash, cfg);

    let usb_future = usb.run();
    let protocol_future = async {
        let mut rx = [0u8; 256];
        let mut rx_len: usize = 0;
        loop {
            class.wait_connection().await;
            loop {
                let mut packet = [0u8; 64];
                match class.read_packet(&mut packet).await {
                    Ok(n) => {
                        for i in 0..n {
                            if rx_len < rx.len() { rx[rx_len] = packet[i]; rx_len += 1; }
                        }
                    }
                    Err(_) => break,
                }
                let mut consumed: usize = 0;
                while consumed < rx_len {
                    match bareboot_protocol::scan(&rx[consumed..rx_len]) {
                        FrameParse::Complete { frame_len, command, .. } => {
                            let mut out = [0u8; 256];
                            let res = if command == bareboot_protocol::commands::CONNECT {
                                dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                            } else if command == bareboot_protocol::commands::RX_BLOCK {
                                dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                            } else if command == bareboot_protocol::commands::RX_EOF {
                                dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                            } else if command == bareboot_protocol::commands::COMPLETE {
                                dispatch_one(&mut writer, &rx[consumed..consumed + frame_len], &mut out)
                            } else {
                                Ok(0)
                            };
                            if let Ok(n) = res {
                                for chunk in out[..n].chunks(64) {
                                    let _ = class.write_packet(chunk).await;
                                }
                            }
                            if consumed + frame_len <= rx_len {
                                for i in 0..rx_len - (consumed + frame_len) {
                                    rx[i] = rx[consumed + frame_len + i];
                                }
                                rx_len -= frame_len;
                                consumed = 0;
                            } else {
                                rx_len = 0;
                                consumed = 0;
                            }
                            break;
                        }
                        FrameParse::Invalid { drop } => {
                            consumed += drop.max(1);
                        }
                        FrameParse::NeedMore => break,
                    }
                }
                if writer.is_completed() {
                    let sp = unsafe { core::ptr::read_volatile(0x0800_1000 as *const u32) };
                    if sp != 0xffffffff {
                        unsafe { Flash::boot_jump(0x0800_1000) };
                    }
                }
            }
        }
    };
    join(usb_future, protocol_future).await;
}
