#![no_std]
#![no_main]

//! RP2350 BareBoot target: USB CDC Katapult recovery from flash 0x00000000.
//!
//! Normal boot: validate the app vector and jump immediately (no USB init).
//! Recovery: cookie request or invalid app vector -> USB CDC Katapult writer.

use bareboot_core::dispatch::dispatch_one;
use bareboot_core::{BootConfig, CoreError, FlashTarget, PagedWriter};
use bareboot_protocol::{self, commands, FrameParse};
use bareboot_rp2350::commit_state::{
    encode_record, read as read_commit, STATE_UPDATE_IN_PROGRESS, STATE_VALID,
};
use bareboot_rp2350::{
    mailbox_from_initial_sp, should_enter_recovery, valid_app_vector, APP_BASE, APP_END,
    BOOTLOADER_XIP,
};
use core::hint::black_box;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Blocking, Flash, FLASH_BASE};
use embassy_rp::otp::OTP_DATA_RAW_BASE;
use embassy_rp::peripherals::{FLASH, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcAcmState};
use embassy_usb::Builder;
use panic_halt as _;
use static_cell::StaticCell;


/// RP2350 bootrom image definition.
///
/// The RP2350 bootrom will not execute a raw XIP image from flash unless the
/// first 256 bytes contain this signature sequence. Without it the bootrom
/// falls back to USB boot (BOOTSEL), which production units do not have.
/// Mirrors ref/katapult/src/rp2040/rpxxxx_link.lds.S.
///
/// Placed in the vector table's interrupt section so the linker extends the
/// vector table with it (within the first 4 KB of flash), where the RP2350
/// bootrom block-loop scanner finds it. Keeps `_stext` auto-adjusted by
/// cortex-m-rt, so no manual linker-script address math is needed.
#[used]
#[link_section = ".vector_table.interrupts"]
static IMAGE_DEFINITION: [u32; 5] = [
    0xffff_ded3,
    0x1021_0142,
    0x0000_01ff,
    0x0000_0000,
    0xab12_3579,
];

const FLASH_SIZE: usize = 4 * 1024 * 1024;
const PAGE_SIZE: usize = 4096;
const BLOCK_SIZE: usize = 64;
const USB_MTU: usize = 64;
const BOOT_WINDOW_MS: u64 = 5000;
const RX_BUF_LEN: usize = bareboot_protocol::MAX_FRAME_BYTES * 2 + USB_MTU;
const OUT_BUF_LEN: usize = bareboot_protocol::MAX_FRAME_BYTES;

/// Uncached XIP alias on RP2350, used for fresh flash reads after programming.
const XIP_NOCACHE_BASE: u32 = 0x1400_0000;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

// ---------------------------------------------------------------------------
// Flash target
// ---------------------------------------------------------------------------

struct RpFlash {
    flash: Flash<'static, FLASH, Blocking, FLASH_SIZE>,
}

impl RpFlash {
    fn new(flash: Flash<'static, FLASH, Blocking, FLASH_SIZE>) -> Self {
        Self { flash }
    }

    /// Convert an XIP address to a flash offset and enforce the write window.
    fn checked_offset(addr: u32, len: usize) -> Result<u32, CoreError> {
        let flash_base = FLASH_BASE as u32;
        if addr < APP_BASE || addr >= APP_END {
            return Err(CoreError::InvalidAddress);
        }
        let end = addr as usize + len;
        if end > APP_END as usize {
            return Err(CoreError::InvalidAddress);
        }
        Ok(addr - flash_base)
    }
}

impl FlashTarget for RpFlash {
    fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    fn app_base(&self) -> u32 {
        APP_BASE
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        let offset = Self::checked_offset(addr, PAGE_SIZE)?;
        if offset % PAGE_SIZE as u32 != 0 {
            return Err(CoreError::InvalidAddress);
        }
        self.flash
            .blocking_erase(offset, offset + PAGE_SIZE as u32)
            .map_err(|_| CoreError::Flash)
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        let offset = Self::checked_offset(addr, data.len())?;
        self.flash
            .blocking_write(offset, data)
            .map_err(|_| CoreError::Flash)
    }

    fn write_commit_record(&mut self, record: &[u8]) -> Result<(), CoreError> {
        // Dedicated commit-state sector at flash offset 0x7000 (the last 4 KB
        // sector of BareBoot's 32 KB slot), never inside the application slot.
        const COMMIT_OFFSET: u32 = 0x7000;
        self.flash
            .blocking_erase(COMMIT_OFFSET, COMMIT_OFFSET + PAGE_SIZE as u32)
            .map_err(|_| CoreError::Flash)?;
        self.flash
            .blocking_write(COMMIT_OFFSET, record)
            .map_err(|_| CoreError::Flash)
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        let _offset = Self::checked_offset(addr, out.len())?;
        let nocache = XIP_NOCACHE_BASE + (addr - FLASH_BASE as u32);
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = unsafe { core::ptr::read_volatile((nocache + i as u32) as *const u8) };
        }
        Ok(())
    }

    fn read_uid(&mut self, out: &mut [u8; 12]) {
        // RP2350 CHIPID0..3 are the first four raw OTP rows (0x4013_4000..).
        let base = OTP_DATA_RAW_BASE as usize;
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = unsafe { core::ptr::read_volatile((base + i) as *const u8) };
        }
    }

    unsafe fn boot_jump(app_entry: u32) -> ! {
        unsafe {
            cortex_m::interrupt::disable();
            let scb = &*cortex_m::peripheral::SCB::PTR;
            scb.vtor.write(app_entry);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();

            let sp = core::ptr::read_volatile(app_entry as *const u32);
            let pc = core::ptr::read_volatile((app_entry + 4) as *const u32);
            core::arch::asm!(
                "msr msp, {}",
                "bx {}",
                in(reg) sp,
                in(reg) pc,
                options(noreturn),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Boot helpers
// ---------------------------------------------------------------------------

fn flash_read_u32(addr: u32) -> u32 {
    let nocache = XIP_NOCACHE_BASE + (addr - FLASH_BASE as u32);
    unsafe { core::ptr::read_volatile(nocache as *const u32) }
}

fn read_app_vector() -> (u32, u32) {
    (
        black_box(flash_read_u32(APP_BASE)),
        black_box(flash_read_u32(APP_BASE + 4)),
    )
}

fn clear_cookie(mailbox: Option<usize>) {
    if let Some(addr) = mailbox {
        unsafe { core::ptr::write_volatile(addr as *mut u64, 0) };
    }
}

fn app_vector_from_writer<F: FlashTarget, const PAGE: usize>(
    writer: &mut PagedWriter<F, PAGE>,
) -> (u32, u32) {
    let mut bytes = [0u8; 8];
    if writer.read_bytes(APP_BASE, &mut bytes).is_err() {
        return (0, 0);
    }
    (
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}

/// Send a Katapult COMMAND_ERROR (0xf2) frame, then reset into BareBoot
/// recovery. No ROM USB boot / BOOTSEL fallback is used: in production the
/// device has no BOOTSEL access, so BareBoot must always be the recovery path.
async fn nack_and_recovery_reset(
    class: &mut CdcAcmClass<'static, Driver<'static, USB>>,
) -> ! {
    let mut nack = [0u8; OUT_BUF_LEN];
    if let Ok(len) = bareboot_protocol::encode_frame(
        bareboot_protocol::responses::COMMAND_ERROR,
        &[],
        &mut nack,
    ) {
        for chunk in nack[..len].chunks(USB_MTU) {
            let mut padded = [0u8; USB_MTU];
            padded[..chunk.len()].copy_from_slice(chunk);
            let _ = class.write_packet(&padded).await;
        }
    }
    // Let the NACK drain before resetting.
    embassy_time::Timer::after_millis(10).await;
    cortex_m::peripheral::SCB::sys_reset();
}

// ---------------------------------------------------------------------------
// Katapult USB CDC protocol loop
// ---------------------------------------------------------------------------

/// Returns `true` when COMPLETE was accepted and the flashed app vector is
/// valid. Returns `false` when the host disconnects.
async fn protocol_loop(
    class: &mut CdcAcmClass<'static, Driver<'static, USB>>,
    writer: &mut PagedWriter<RpFlash, PAGE_SIZE>,
) -> bool {
    let mut rx = [0u8; RX_BUF_LEN];
    let mut rx_len: usize = 0;
    // The first accepted SEND_BLOCK marks the update as in progress so a torn
    // transfer can never leave the app slot bootable on the next reset.
    let mut transfer_started = false;

    loop {
        let mut packet = [0u8; USB_MTU];
        let n = match class.read_packet(&mut packet).await {
            Ok(n) => n,
            Err(_) => return false,
        };
        if n == 0 {
            continue;
        }

        // Append the 64-byte USB packet, dropping oldest bytes if necessary.
        if rx_len + n > rx.len() {
            let overflow = rx_len + n - rx.len();
            rx.copy_within(overflow..rx_len, 0);
            rx_len -= overflow;
        }
        rx[rx_len..rx_len + n].copy_from_slice(&packet[..n]);
        rx_len += n;

        let mut consumed: usize = 0;
        while consumed < rx_len {
            match bareboot_protocol::scan(&rx[consumed..rx_len]) {
                FrameParse::Complete {
                    frame_len, command, ..
                } => {
                    // Mark the transfer in progress before any app page is
                    // erased. This is the commit-state "begin" transition.
                    if command == commands::RX_BLOCK && !transfer_started {
                        if writer
                            .write_commit_record(&encode_record(STATE_UPDATE_IN_PROGRESS))
                            .is_err()
                        {
                            nack_and_recovery_reset(class).await;
                        }
                        transfer_started = true;
                    }

                    let mut out = [0u8; OUT_BUF_LEN];
                    let result = match command {
                        commands::CONNECT
                        | commands::RX_BLOCK
                        | commands::RX_EOF
                        | commands::REQ_BLOCK
                        | commands::COMPLETE
                        | commands::GET_CANBUS_ID => {
                            dispatch_one(writer, &rx[consumed..consumed + frame_len], &mut out)
                        }
                        _ => Ok(0),
                    };

                    match result {
                        Ok(len) => {
                            for chunk in out[..len].chunks(USB_MTU) {
                                let mut padded = [0u8; USB_MTU];
                                padded[..chunk.len()].copy_from_slice(chunk);
                                if class.write_packet(&padded).await.is_err() {
                                    return false;
                                }
                            }
                        }
                        Err(CoreError::Flash) => {
                            // State remains UPDATE_IN_PROGRESS; reset into
                            // BareBoot recovery, never into ROM USB boot.
                            nack_and_recovery_reset(class).await;
                        }
                        Err(_) => {}
                    }

                    // Remove the consumed frame and keep scanning buffered data.
                    let after = consumed + frame_len;
                    rx.copy_within(after..rx_len, 0);
                    rx_len -= frame_len;
                    consumed = 0;

                    if command == commands::COMPLETE && writer.is_completed() {
                        let (sp, pc) = app_vector_from_writer(writer);
                        if valid_app_vector(sp, pc) {
                            // COMPLETE is sent by stock Katapult only after a
                            // full readback SHA1 verification. Commit the image.
                            if writer
                                .write_commit_record(&encode_record(STATE_VALID))
                                .is_err()
                            {
                                nack_and_recovery_reset(class).await;
                            }
                            return true;
                        }
                    }
                }
                FrameParse::Invalid { drop } => {
                    consumed += drop.max(1);
                }
                FrameParse::NeedMore => break,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Firmware entry
// ---------------------------------------------------------------------------

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Boot decision: cookie mailbox derived from our own initial SP word.
    let initial_sp = flash_read_u32(BOOTLOADER_XIP);
    let mailbox = mailbox_from_initial_sp(initial_sp);
    let cookie = mailbox
        .map(|addr| unsafe { core::ptr::read_volatile(addr as *const u64) })
        .unwrap_or(0);

    let (app_sp, app_pc) = read_app_vector();
    let commit = read_commit();

    if !should_enter_recovery(cookie, app_sp, app_pc, commit) {
        // Normal path: clear the mailbox and jump with no USB initialisation.
        clear_cookie(mailbox);
        unsafe { RpFlash::boot_jump(APP_BASE) };
    }

    // Recovery path. USB requires a precise 48 MHz clock; the ROSC used by
    // Default::default() is not accurate enough for USB enumeration, so use
    // the Pico 2's 12 MHz crystal to drive the USB PLL.
    let mut hal_config = embassy_rp::config::Config::default();
    hal_config.clocks.rosc = None;
    let p = embassy_rp::init(hal_config);

    let driver = Driver::new(p.USB, Irqs);
    let mut config = embassy_usb::Config::new(0x1d50, 0x6177);
    config.manufacturer = Some("BareBoot");
    config.product = Some("BareBoot RP2350");
    config.serial_number = Some("0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 1024]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 256]> = StaticCell::new();
    static CDC_STATE: StaticCell<CdcAcmState> = StaticCell::new();

    let config_desc = CONFIG_DESC.init([0; 1024]);
    let bos_desc = BOS_DESC.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 256]);
    let cdc_state = CDC_STATE.init(CdcAcmState::new());

    let mut builder = Builder::new(
        driver,
        config,
        config_desc,
        bos_desc,
        &mut [],
        control_buf,
    );
    let mut class = CdcAcmClass::new(&mut builder, cdc_state, USB_MTU as u16);
    let mut usb = builder.build();

    // CHIPID0..3 are the first raw OTP rows on RP2350.
    let mut uid = [0u8; 12];
    let otp_base = OTP_DATA_RAW_BASE as usize;
    for (i, byte) in uid.iter_mut().enumerate() {
        *byte = unsafe { core::ptr::read_volatile((otp_base + i) as *const u8) };
    }

    let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = RpFlash::new(flash);
    let cfg = BootConfig {
        protocol_version: 0x0001_0100,
        app_base: APP_BASE,
        block_size: BLOCK_SIZE,
        mcu_type: b"rp2350",
        software_version: b"dev",
        uid,
    };
    let mut writer = PagedWriter::<RpFlash, PAGE_SIZE>::new(flash, cfg);


    'recovery: loop {
        // Boot window: poll the USB device stack alongside the connection
        // wait. If no host arrives within ~5s and a valid committed app exists,
        // clear the cookie and boot. Invalid app / non-valid commit stays in
        // recovery.
        let wait_for_host = async { class.wait_connection().await };
        match select(
            usb.run(),
            with_timeout(Duration::from_millis(BOOT_WINDOW_MS), wait_for_host),
        )
        .await
        {
            Either::Second(Err(_)) => {
                let (sp, pc) = read_app_vector();
                if valid_app_vector(sp, pc) && read_commit().is_valid() {
                    clear_cookie(mailbox);
                    unsafe { RpFlash::boot_jump(APP_BASE) };
                }
                continue 'recovery;
            }
            Either::Second(Ok(())) => {}
            Either::First(_) => unreachable!("usb.run() never completes"),
        }

        // Host connected: serve Katapult commands until completion or
        // disconnect, polling the USB device stack concurrently.
        let result = {
            let protocol = protocol_loop(&mut class, &mut writer);
            select(usb.run(), protocol).await
        };

        match result {
            Either::Second(true) => {
                let (sp, pc) = app_vector_from_writer(&mut writer);
                if valid_app_vector(sp, pc) && read_commit().is_valid() {
                    clear_cookie(mailbox);
                    unsafe { RpFlash::boot_jump(APP_BASE) };
                }
                // Invalid flashed image or failed commit: remain in recovery.
            }
            Either::Second(false) => {
                // Disconnected: restart the 5s boot window.
            }
            Either::First(_) => unreachable!("usb.run() never completes"),
        }
    }
}
