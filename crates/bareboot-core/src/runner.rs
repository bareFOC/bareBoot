use crate::commit::{CommitState, MonotonicCommitJournal};
use crate::dispatch::dispatch_one;
use crate::hal::{ArchBoot, BootPlatform, BusTransport, FlashRegion, PollEvent, ResetReason, TickTimer};
use crate::writer::{PagedWriter, WatchdogFeed};
use crate::{crc32_update, BootConfig, BootLoopGuard, CoreError, FlashTarget};
use bareboot_protocol::responses;

/// No-op watchdog for targets or tests without a hardware watchdog.
pub struct NoopWatchdog;

impl WatchdogFeed for NoopWatchdog {
    fn feed(&mut self) {}
}

/// Validate an application vector table using architecture-specific PC checks (`ArchBoot::is_valid_pc`).
pub fn validate_app_vector_with_arch<A: ArchBoot>(
    sp: u32,
    pc: u32,
    ram_start: u32,
    ram_end: u32,
    app_base: u32,
    flash_end: u32,
    entry_opcode: &[u8],
) -> bool {
    if sp == 0 || sp == 0xFFFF_FFFF || pc == 0 || pc == 0xFFFF_FFFF {
        return false;
    }
    if sp < ram_start || sp > ram_end {
        return false;
    }
    if !A::is_valid_pc(pc, app_base, flash_end) {
        return false;
    }
    if entry_opcode.len() < 4 {
        return false;
    }
    let entry = u32::from_le_bytes([
        entry_opcode[0],
        entry_opcode[1],
        entry_opcode[2],
        entry_opcode[3],
    ]);
    if entry == 0 || entry == 0xFFFF_FFFF {
        return false;
    }
    true
}

/// Validate an application vector table stored in `flash` using `ArchBoot`.
pub fn validate_app_vector_from_flash<F: FlashRegion, A: ArchBoot>(
    flash: &mut F,
    ram_start: u32,
    ram_end: u32,
) -> bool {
    let app_base = flash.region_base();
    let app_end = flash.region_end();
    let mut vec_bytes = [0u8; 8];
    if flash.read_bytes(app_base, &mut vec_bytes).is_err() {
        return false;
    }
    let sp = u32::from_le_bytes([vec_bytes[0], vec_bytes[1], vec_bytes[2], vec_bytes[3]]);
    let pc = u32::from_le_bytes([vec_bytes[4], vec_bytes[5], vec_bytes[6], vec_bytes[7]]);
    let entry_addr = pc & !1;
    if entry_addr < app_base || entry_addr.saturating_add(4) > app_end {
        return false;
    }
    let mut op = [0u8; 4];
    if flash.read_bytes(entry_addr, &mut op).is_err() {
        return false;
    }
    validate_app_vector_with_arch::<A>(sp, pc, ram_start, ram_end, app_base, app_end, &op)
}

/// Stream-verify IEEE 802.3 CRC32 over `[region_base, region_base + app_len)` while feeding `wd`.
pub fn verify_app_crc32<F: FlashRegion, W: WatchdogFeed>(
    flash: &mut F,
    wd: &mut W,
    app_len: u32,
    expected_crc: u32,
) -> bool {
    let base = flash.region_base();
    let end = flash.region_end();
    let max_len = end.saturating_sub(base);
    if app_len == 0 || app_len > max_len {
        return false;
    }
    let mut buf = [0u8; 256];
    let mut crc = 0xFFFF_FFFFu32;
    let mut offset = 0u32;
    while offset < app_len {
        wd.feed();
        let chunk = ((app_len - offset) as usize).min(buf.len());
        if flash.read_bytes(base + offset, &mut buf[..chunk]).is_err() {
            return false;
        }
        crc = crc32_update(crc, &buf[..chunk]);
        offset += chunk as u32;
    }
    (!crc) == expected_crc
}

struct CommitRegionAdapter<'a, CF>(&'a mut CF);

impl<CF: FlashRegion> FlashTarget for CommitRegionAdapter<'_, CF> {
    fn page_size(&self) -> usize {
        self.0.page_size()
    }

    fn app_base(&self) -> u32 {
        self.0.region_base()
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        self.0.erase_page(addr)
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        self.0.write_bytes(addr, data)
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        self.0.read_bytes(addr, out)
    }

    fn read_uid(&mut self, _out: &mut [u8; 12]) {}

    unsafe fn boot_jump(_app_entry: u32) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}

struct RegionCommitTarget<F, CF, const COMMIT_SIZE: usize, const COMMIT_ALIGN: usize> {
    app_flash: F,
    commit_flash: CF,
    journal: MonotonicCommitJournal<COMMIT_SIZE, COMMIT_ALIGN>,
}

impl<F: FlashRegion, CF: FlashRegion, const COMMIT_SIZE: usize, const COMMIT_ALIGN: usize>
    FlashTarget for RegionCommitTarget<F, CF, COMMIT_SIZE, COMMIT_ALIGN>
{
    fn page_size(&self) -> usize {
        self.app_flash.page_size()
    }

    fn app_base(&self) -> u32 {
        self.app_flash.region_base()
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        self.app_flash.erase_page(addr)
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        self.app_flash.write_bytes(addr, data)
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        self.app_flash.read_bytes(addr, out)
    }

    fn read_uid(&mut self, _out: &mut [u8; 12]) {}

    unsafe fn boot_jump(_app_entry: u32) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    fn on_update_start(&mut self) -> Result<(), CoreError> {
        self.journal
            .mark_in_progress(&mut CommitRegionAdapter(&mut self.commit_flash))
    }

    fn on_update_complete(&mut self, app_len: u32, app_crc32: u32) -> Result<(), CoreError> {
        self.journal.mark_valid(
            &mut CommitRegionAdapter(&mut self.commit_flash),
            app_len,
            app_crc32,
        )
    }
}

/// Outcome of a single `BootRunner::step()` iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerAction {
    Continue,
    JumpToApp(u32),
    SystemReset,
}

/// High-level bootloader status used to drive the single-LED indicator pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStatus {
    /// Initial boot window waiting for host activity before jumping to app.
    BootWindow,
    /// Parked in bootloader mode (requested via mailbox or bus activity), idle.
    Parked,
    /// Active firmware programming (RX_BLOCK / RX_EOF / COMPLETE).
    Flashing,
    /// No valid application image in flash.
    BlankApp,
    /// Boot-loop trap triggered or flash write/commit error occurred.
    BootLoopError,
}

/// Compute whether the status LED should be illuminated at `ms` milliseconds
/// for the given `BootStatus`.
pub const fn compute_led_on(status: BootStatus, ms: u32) -> bool {
    let phase = ms % 1000;
    match status {
        BootStatus::BootWindow => true,
        BootStatus::Parked => phase < 500,
        BootStatus::Flashing => (ms % 100) < 50,
        BootStatus::BlankApp => phase < 100 || (phase >= 200 && phase < 300),
        BootStatus::BootLoopError => {
            phase < 80 || (phase >= 160 && phase < 240) || (phase >= 320 && phase < 400)
        }
    }
}

/// Generic, zero-heap, monomorphized bootloader state machine.
pub struct BootRunner<
    P: BootPlatform,
    const PAGE: usize,
    const COMMIT_SIZE: usize,
    const COMMIT_ALIGN: usize,
> {
    writer: PagedWriter<RegionCommitTarget<P::Flash, P::CommitFlash, COMMIT_SIZE, COMMIT_ALIGN>, PAGE>,
    transport: P::Transport,
    watchdog: P::Watchdog,
    timer: P::Timer,
    config: BootConfig,
    boot_window_ms: u32,
    idle_timeout_ms: Option<u32>,
    boot_loop_guard_addr: Option<usize>,
    fault_rst: bool,
    boot_loop_trapped: bool,
    app_integrity_ok: bool,
    boot_window_active: bool,
    ms_elapsed: u32,
    idle_ms: u32,
    flash_activity_ms: u32,
    flash_error: bool,
    led_on: bool,
    rx_buf: [u8; 256],
    tx_buf: [u8; 256],
}

impl<
        P: BootPlatform,
        const PAGE: usize,
        const COMMIT_SIZE: usize,
        const COMMIT_ALIGN: usize,
    > BootRunner<P, PAGE, COMMIT_SIZE, COMMIT_ALIGN>
{
    pub fn new(mut platform: P) -> Self {
        let config = platform.boot_config();
        let (ram_start, ram_end) = platform.ram_bounds();
        let boot_window_ms = platform.boot_window_ms();
        let idle_timeout_ms = platform.idle_timeout_ms();
        let boot_loop_guard_addr = platform.boot_loop_guard_addr();
        let fault_rst = matches!(
            platform.reset_reason(),
            ResetReason::Watchdog | ResetReason::Software
        );
        let mailbox_req = platform.read_mailbox_request();
        let boot_loop_trapped =
            boot_loop_guard_addr.map_or(false, BootLoopGuard::is_trapped);
        if mailbox_req || !fault_rst {
            if let Some(addr) = boot_loop_guard_addr {
                BootLoopGuard::clear(addr);
            }
        }

        let (mut app_flash, mut commit_flash, transport, mut watchdog, timer) =
            platform.into_parts();

        let mut journal =
            MonotonicCommitJournal::<COMMIT_SIZE, COMMIT_ALIGN>::new(commit_flash.region_base());
        let commit_valid = matches!(
            journal.init(&mut CommitRegionAdapter(&mut commit_flash)),
            Ok(CommitState::Valid)
        );
        let app_integrity_ok = commit_valid
            && validate_app_vector_from_flash::<P::Flash, P::Arch>(&mut app_flash, ram_start, ram_end)
            && verify_app_crc32(
                &mut app_flash,
                &mut watchdog,
                journal.app_len(),
                journal.app_crc32(),
            );

        let boot_window_active = !mailbox_req && !boot_loop_trapped && app_integrity_ok;
        let target = RegionCommitTarget {
            app_flash,
            commit_flash,
            journal,
        };
        let writer = PagedWriter::new(target, config);

        let mut runner = Self {
            writer,
            transport,
            watchdog,
            timer,
            config,
            boot_window_ms,
            idle_timeout_ms,
            boot_loop_guard_addr,
            fault_rst,
            boot_loop_trapped,
            app_integrity_ok,
            boot_window_active,
            ms_elapsed: 0,
            idle_ms: 0,
            flash_activity_ms: 0,
            flash_error: false,
            led_on: false,
            rx_buf: [0u8; 256],
            tx_buf: [0u8; 256],
        };
        let initial_led = compute_led_on(runner.status(), 0);
        runner.led_on = initial_led;
        runner.timer.on_heartbeat(initial_led);
        runner
    }

    pub fn status(&self) -> BootStatus {
        if self.flash_activity_ms > 0 {
            BootStatus::Flashing
        } else if self.boot_loop_trapped || self.flash_error {
            BootStatus::BootLoopError
        } else if !self.app_integrity_ok {
            BootStatus::BlankApp
        } else if self.boot_window_active {
            BootStatus::BootWindow
        } else {
            BootStatus::Parked
        }
    }

    fn sync_led(&mut self) {
        let next_led = compute_led_on(self.status(), self.ms_elapsed);
        if next_led != self.led_on {
            self.led_on = next_led;
            self.timer.on_heartbeat(next_led);
        }
    }

    pub fn is_boot_window_active(&self) -> bool {
        self.boot_window_active
    }

    pub fn is_app_integrity_ok(&self) -> bool {
        self.app_integrity_ok
    }

    pub fn is_boot_loop_trapped(&self) -> bool {
        self.boot_loop_trapped
    }

    pub fn transport(&self) -> &P::Transport {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut P::Transport {
        &mut self.transport
    }

    pub fn step(&mut self) -> RunnerAction {
        self.watchdog.feed();

        if self.transport.has_activity() {
            self.idle_ms = 0;
        }

        if self.timer.poll_1ms() {
            self.ms_elapsed = self.ms_elapsed.wrapping_add(1);
            self.idle_ms = self.idle_ms.saturating_add(1);
            self.flash_activity_ms = self.flash_activity_ms.saturating_sub(1);
            self.sync_led();
            self.transport.on_tick_1ms();

            if let Some(limit) = self.idle_timeout_ms {
                if self.app_integrity_ok && !self.boot_loop_trapped && self.idle_ms >= limit {
                    return RunnerAction::SystemReset;
                }
            }

            if self.boot_window_active && self.ms_elapsed >= self.boot_window_ms {
                if self.fault_rst {
                    if let Some(addr) = self.boot_loop_guard_addr {
                        BootLoopGuard::record_boot_attempt(addr);
                    }
                }
                return RunnerAction::JumpToApp(self.config.app_base);
            }
        }

        match self.transport.poll_rx(&mut self.rx_buf) {
            PollEvent::None => {}
            PollEvent::BusReset => {
                self.idle_ms = 0;
                self.boot_window_active = false;
                if let Some(addr) = self.boot_loop_guard_addr {
                    BootLoopGuard::clear(addr);
                }
                self.sync_led();
            }
            PollEvent::BusActivity => {
                self.idle_ms = 0;
                self.boot_window_active = false;
                self.sync_led();
            }
            PollEvent::Frame(rx_len) => {
                self.idle_ms = 0;
                let cmd = match bareboot_protocol::scan(&self.rx_buf[..rx_len]) {
                    bareboot_protocol::FrameParse::Complete { command, .. } => Some(command),
                    _ => None,
                };
                if matches!(
                    cmd,
                    Some(
                        bareboot_protocol::commands::RX_BLOCK
                            | bareboot_protocol::commands::RX_EOF
                            | bareboot_protocol::commands::COMPLETE
                    )
                ) {
                    self.flash_activity_ms = 150;
                    self.flash_error = false;
                } else if matches!(cmd, Some(bareboot_protocol::commands::CONNECT)) {
                    self.flash_error = false;
                }
                let resp_len = match dispatch_one(
                    &mut self.writer,
                    &self.rx_buf[..rx_len],
                    &mut self.tx_buf,
                ) {
                    Ok(len) => {
                        if len > 0 {
                            if let bareboot_protocol::FrameParse::Complete { command, .. } =
                                bareboot_protocol::scan(&self.tx_buf[..len])
                            {
                                if command == responses::COMMAND_ERROR || command == responses::NACK
                                {
                                    self.flash_error = true;
                                }
                            }
                        }
                        len
                    }
                    Err(_) => {
                        self.flash_error = true;
                        bareboot_protocol::encode_frame(responses::NACK, &[], &mut self.tx_buf)
                            .unwrap_or(0)
                    }
                };
                if resp_len > 0 {
                    self.boot_window_active = false;
                    let _ = self.transport.send_tx(&self.tx_buf[..resp_len]);
                }
                self.sync_led();
                if self.writer.is_completed() && self.transport.tx_idle() {
                    if let Some(addr) = self.boot_loop_guard_addr {
                        BootLoopGuard::clear(addr);
                    }
                    return RunnerAction::JumpToApp(self.config.app_base);
                }
            }
        }

        RunnerAction::Continue
    }

    pub fn run(&mut self) -> ! {
        loop {
            match self.step() {
                RunnerAction::Continue => {}
                RunnerAction::JumpToApp(addr) => {
                    self.transport.wait_tx_drain();
                    self.transport.prepare_for_jump();
                    self.watchdog.feed();
                    unsafe { P::Arch::jump_to_app(addr) }
                }
                RunnerAction::SystemReset => {
                    P::Arch::system_reset();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crc32;
    use bareboot_protocol::{commands, encode_frame, REQUEST_BOOTLOADER};

    const PAGE: usize = 64;
    const APP_SIZE: usize = 256;
    const COMMIT_SIZE: usize = 128;
    const APP_BASE: u32 = 0x0800_4000;
    const APP_END: u32 = APP_BASE + APP_SIZE as u32;
    const COMMIT_BASE: u32 = 0x0801_A000;
    const RAM_START: u32 = 0x2000_0000;
    const RAM_END: u32 = 0x2000_8000;

    struct MockArch;
    impl ArchBoot for MockArch {
        fn is_valid_pc(pc: u32, app_base: u32, app_end: u32) -> bool {
            pc != 0 && pc != 0xFFFF_FFFF && (pc & 1) == 1 && pc >= app_base && pc < app_end
        }
        unsafe fn jump_to_app(_app_base: u32) -> ! {
            unreachable!("host test uses step()")
        }
        fn system_reset() -> ! {
            unreachable!("host test uses step()")
        }
    }

    struct RiscvArch;
    impl ArchBoot for RiscvArch {
        fn is_valid_pc(pc: u32, app_base: u32, app_end: u32) -> bool {
            pc != 0 && pc != 0xFFFF_FFFF && (pc & 1) == 0 && pc >= app_base && pc < app_end
        }
        unsafe fn jump_to_app(_app_base: u32) -> ! {
            unreachable!()
        }
        fn system_reset() -> ! {
            unreachable!()
        }
    }

    struct MockMemFlash {
        base: u32,
        mem: [u8; 256],
        len: usize,
    }

    impl MockMemFlash {
        fn new(base: u32, len: usize) -> Self {
            Self {
                base,
                mem: [0xFF; 256],
                len,
            }
        }
    }

    impl FlashRegion for MockMemFlash {
        fn page_size(&self) -> usize {
            if self.len == COMMIT_SIZE {
                COMMIT_SIZE
            } else {
                PAGE
            }
        }
        fn region_base(&self) -> u32 {
            self.base
        }
        fn region_end(&self) -> u32 {
            self.base + self.len as u32
        }
        fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
            let off = (addr - self.base) as usize;
            let sz = self.page_size();
            self.mem[off..off + sz].fill(0xFF);
            Ok(())
        }
        fn write_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
            let off = (addr - self.base) as usize;
            self.mem[off..off + data.len()].copy_from_slice(data);
            Ok(())
        }
        fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
            let off = (addr - self.base) as usize;
            out.copy_from_slice(&self.mem[off..off + out.len()]);
            Ok(())
        }
    }

    struct MockTimer {
        ticks_available: u32,
    }

    impl TickTimer for MockTimer {
        fn poll_1ms(&mut self) -> bool {
            if self.ticks_available > 0 {
                self.ticks_available -= 1;
                true
            } else {
                false
            }
        }
    }

    struct MockTransport {
        rx_queue: [[u8; 192]; 8],
        rx_lens: [usize; 8],
        rx_head: usize,
        rx_tail: usize,
        tx_frames: [[u8; 192]; 8],
        tx_lens: [usize; 8],
        tx_count: usize,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                rx_queue: [[0; 192]; 8],
                rx_lens: [0; 8],
                rx_head: 0,
                rx_tail: 0,
                tx_frames: [[0; 192]; 8],
                tx_lens: [0; 8],
                tx_count: 0,
            }
        }

        fn push_rx(&mut self, frame: &[u8]) {
            self.rx_queue[self.rx_tail][..frame.len()].copy_from_slice(frame);
            self.rx_lens[self.rx_tail] = frame.len();
            self.rx_tail += 1;
        }
    }

    impl BusTransport for MockTransport {
        fn poll_rx(&mut self, buf: &mut [u8]) -> PollEvent {
            if self.rx_head < self.rx_tail {
                let len = self.rx_lens[self.rx_head];
                buf[..len].copy_from_slice(&self.rx_queue[self.rx_head][..len]);
                self.rx_head += 1;
                PollEvent::Frame(len)
            } else {
                PollEvent::None
            }
        }
        fn send_tx(&mut self, frame: &[u8]) -> Result<(), CoreError> {
            self.tx_frames[self.tx_count][..frame.len()].copy_from_slice(frame);
            self.tx_lens[self.tx_count] = frame.len();
            self.tx_count += 1;
            Ok(())
        }
        fn tx_idle(&self) -> bool {
            true
        }
    }

    struct MockPlatform {
        app_flash: MockMemFlash,
        commit_flash: MockMemFlash,
        transport: MockTransport,
        timer: MockTimer,
        reset_reason: ResetReason,
        guard_addr: Option<usize>,
        mailbox_addr: Option<u32>,
        boot_window_ms: u32,
        idle_timeout_ms: Option<u32>,
    }

    impl BootPlatform for MockPlatform {
        type Flash = MockMemFlash;
        type CommitFlash = MockMemFlash;
        type Transport = MockTransport;
        type Watchdog = NoopWatchdog;
        type Timer = MockTimer;
        type Arch = MockArch;

        fn boot_config(&self) -> BootConfig {
            BootConfig {
                protocol_version: 1,
                app_base: APP_BASE,
                app_end: APP_END,
                block_size: 64,
                mcu_type: b"mock",
                software_version: b"0.1",
                uid: [0x11; 12],
            }
        }
        fn ram_bounds(&self) -> (u32, u32) {
            (RAM_START, RAM_END)
        }
        fn boot_window_ms(&self) -> u32 {
            self.boot_window_ms
        }
        fn idle_timeout_ms(&self) -> Option<u32> {
            self.idle_timeout_ms
        }
        fn mailbox_addr(&self) -> Option<u32> {
            self.mailbox_addr
        }
        fn boot_loop_guard_addr(&self) -> Option<usize> {
            self.guard_addr
        }
        fn reset_reason(&self) -> ResetReason {
            self.reset_reason
        }
        fn into_parts(
            self,
        ) -> (
            Self::Flash,
            Self::CommitFlash,
            Self::Transport,
            Self::Watchdog,
            Self::Timer,
        ) {
            (
                self.app_flash,
                self.commit_flash,
                self.transport,
                NoopWatchdog,
                self.timer,
            )
        }
    }

    fn seed_valid_app(app_flash: &mut MockMemFlash, commit_flash: &mut MockMemFlash) {
        let mut page = [0xAAu8; PAGE];
        let sp = 0x2000_4000u32.to_le_bytes();
        let pc = (APP_BASE + 0x11).to_le_bytes(); // Thumb bit set
        page[0..4].copy_from_slice(&sp);
        page[4..8].copy_from_slice(&pc);
        page[0x10..0x14].copy_from_slice(&[0x80, 0xB5, 0x00, 0xAF]);
        app_flash.write_bytes(APP_BASE, &page).unwrap();

        let crc = crc32(&page);
        let mut journal = MonotonicCommitJournal::<COMMIT_SIZE, 16>::new(COMMIT_BASE);
        journal.init(&mut CommitRegionAdapter(commit_flash)).unwrap();
        journal
            .mark_valid(&mut CommitRegionAdapter(commit_flash), PAGE as u32, crc)
            .unwrap();
    }

    #[test]
    fn riscv_and_arm_thumb_vector_validation() {
        let valid_op = [0x80, 0xB5, 0x00, 0xAF];
        // ARM Thumb requires odd PC
        assert!(validate_app_vector_with_arch::<MockArch>(
            0x2000_4000,
            APP_BASE + 0x11,
            RAM_START,
            RAM_END,
            APP_BASE,
            APP_END,
            &valid_op
        ));
        assert!(!validate_app_vector_with_arch::<MockArch>(
            0x2000_4000,
            APP_BASE + 0x10,
            RAM_START,
            RAM_END,
            APP_BASE,
            APP_END,
            &valid_op
        ));
        // RISC-V requires even PC
        assert!(validate_app_vector_with_arch::<RiscvArch>(
            0x2000_4000,
            APP_BASE + 0x10,
            RAM_START,
            RAM_END,
            APP_BASE,
            APP_END,
            &valid_op
        ));
        assert!(!validate_app_vector_with_arch::<RiscvArch>(
            0x2000_4000,
            APP_BASE + 0x11,
            RAM_START,
            RAM_END,
            APP_BASE,
            APP_END,
            &valid_op
        ));
    }

    #[test]
    fn boot_window_expiry_jumps_to_valid_app() {
        let mut app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
        let mut commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
        seed_valid_app(&mut app_flash, &mut commit_flash);

        let platform = MockPlatform {
            app_flash,
            commit_flash,
            transport: MockTransport::new(),
            timer: MockTimer { ticks_available: 10 },
            reset_reason: ResetReason::PowerOn,
            guard_addr: None,
            mailbox_addr: None,
            boot_window_ms: 5,
            idle_timeout_ms: None,
        };
        let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
        assert!(runner.is_app_integrity_ok());
        assert!(runner.is_boot_window_active());

        for _ in 0..4 {
            assert_eq!(runner.step(), RunnerAction::Continue);
        }
        assert_eq!(runner.step(), RunnerAction::JumpToApp(APP_BASE));
    }

    #[test]
    fn boot_loop_guard_3_strike_trap() {
        let mut guard_slot = [0u32; 4];
        let gaddr = guard_slot.as_mut_ptr() as usize;

        for expected_attempts in 1..=3 {
            let mut app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
            let mut commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
            seed_valid_app(&mut app_flash, &mut commit_flash);

            let platform = MockPlatform {
                app_flash,
                commit_flash,
                transport: MockTransport::new(),
                timer: MockTimer { ticks_available: 5 },
                reset_reason: ResetReason::Watchdog,
                guard_addr: Some(gaddr),
                mailbox_addr: None,
                boot_window_ms: 2,
                idle_timeout_ms: None,
            };
            let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
            assert!(!runner.is_boot_loop_trapped());
            assert_eq!(runner.step(), RunnerAction::Continue);
            assert_eq!(runner.step(), RunnerAction::JumpToApp(APP_BASE));
            assert_eq!(BootLoopGuard::read(gaddr).unwrap().attempts, expected_attempts);
        }

        // 4th fault reset is trapped in bootloader
        let mut app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
        let mut commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
        seed_valid_app(&mut app_flash, &mut commit_flash);
        let platform = MockPlatform {
            app_flash,
            commit_flash,
            transport: MockTransport::new(),
            timer: MockTimer { ticks_available: 10 },
            reset_reason: ResetReason::Watchdog,
            guard_addr: Some(gaddr),
            mailbox_addr: None,
            boot_window_ms: 2,
            idle_timeout_ms: None,
        };
        let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
        assert!(runner.is_boot_loop_trapped());
        assert!(!runner.is_boot_window_active());
        for _ in 0..5 {
            assert_eq!(runner.step(), RunnerAction::Continue);
        }
    }

    #[test]
    fn flash_flow_block_write_eof_complete_and_crc32() {
        let app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
        let commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
        let mut transport = MockTransport::new();

        // Prepare valid 64-byte app image
        let mut page = [0x5Au8; PAGE];
        page[0..4].copy_from_slice(&0x2000_4000u32.to_le_bytes());
        page[4..8].copy_from_slice(&(APP_BASE + 0x11).to_le_bytes());
        page[0x10..0x14].copy_from_slice(&[0x80, 0xB5, 0x00, 0xAF]);

        let mut fbuf = [0u8; 192];
        let l1 = encode_frame(commands::CONNECT, &[], &mut fbuf).unwrap();
        transport.push_rx(&fbuf[..l1]);

        let mut payload = [0u8; 68];
        payload[0..4].copy_from_slice(&APP_BASE.to_le_bytes());
        payload[4..].copy_from_slice(&page);
        let l2 = encode_frame(commands::RX_BLOCK, &payload, &mut fbuf).unwrap();
        transport.push_rx(&fbuf[..l2]);

        let l3 = encode_frame(commands::RX_EOF, &[], &mut fbuf).unwrap();
        transport.push_rx(&fbuf[..l3]);

        let l4 = encode_frame(commands::COMPLETE, &[], &mut fbuf).unwrap();
        transport.push_rx(&fbuf[..l4]);

        let mut mailbox = [0u32; 4];
        mailbox[0] = (REQUEST_BOOTLOADER & 0xFFFF_FFFF) as u32;
        mailbox[1] = (REQUEST_BOOTLOADER >> 32) as u32;

        let platform = MockPlatform {
            app_flash,
            commit_flash,
            transport,
            timer: MockTimer { ticks_available: 0 },
            reset_reason: ResetReason::PowerOn,
            guard_addr: Some(mailbox.as_mut_ptr() as usize),
            mailbox_addr: None,
            boot_window_ms: 1500,
            idle_timeout_ms: None,
        };
        let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
        assert!(!runner.is_app_integrity_ok());

        assert_eq!(runner.step(), RunnerAction::Continue); // CONNECT
        assert_eq!(runner.step(), RunnerAction::Continue); // RX_BLOCK
        assert_eq!(runner.step(), RunnerAction::Continue); // RX_EOF
        assert_eq!(runner.step(), RunnerAction::JumpToApp(APP_BASE)); // COMPLETE
        assert_eq!(runner.transport().tx_count, 4);
    }

    #[test]
    fn idle_timeout_triggers_system_reset() {
        let mut app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
        let mut commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
        seed_valid_app(&mut app_flash, &mut commit_flash);

        // Park via guard cookie so boot_window_active is false, then wait for idle_timeout_ms
        let mut guard_slot = [0u32; 4];
        guard_slot[0] = (REQUEST_BOOTLOADER & 0xFFFF_FFFF) as u32;
        guard_slot[1] = (REQUEST_BOOTLOADER >> 32) as u32;

        let platform = MockPlatform {
            app_flash,
            commit_flash,
            transport: MockTransport::new(),
            timer: MockTimer { ticks_available: 10 },
            reset_reason: ResetReason::PowerOn,
            guard_addr: Some(guard_slot.as_mut_ptr() as usize),
            mailbox_addr: None,
            boot_window_ms: 2,
            idle_timeout_ms: Some(4),
        };
        let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
        assert!(!runner.is_boot_window_active());
        assert!(runner.is_app_integrity_ok());

        for _ in 0..3 {
            assert_eq!(runner.step(), RunnerAction::Continue);
        }
        assert_eq!(runner.step(), RunnerAction::SystemReset);
    }

    #[test]
    fn status_led_patterns_for_all_five_states() {
        // BootWindow: solid ON
        for ms in [0, 50, 499, 500, 999, 1234] {
            assert!(compute_led_on(BootStatus::BootWindow, ms));
        }

        // Parked: 1 Hz 50% duty (0..500 ON, 500..1000 OFF)
        assert!(compute_led_on(BootStatus::Parked, 0));
        assert!(compute_led_on(BootStatus::Parked, 499));
        assert!(!compute_led_on(BootStatus::Parked, 500));
        assert!(!compute_led_on(BootStatus::Parked, 999));
        assert!(compute_led_on(BootStatus::Parked, 1000));

        // Flashing: 10 Hz 50% duty (0..50 ON, 50..100 OFF)
        assert!(compute_led_on(BootStatus::Flashing, 0));
        assert!(compute_led_on(BootStatus::Flashing, 49));
        assert!(!compute_led_on(BootStatus::Flashing, 50));
        assert!(!compute_led_on(BootStatus::Flashing, 99));
        assert!(compute_led_on(BootStatus::Flashing, 100));

        // BlankApp: double-blink (0..100, 200..300)
        assert!(compute_led_on(BootStatus::BlankApp, 0));
        assert!(compute_led_on(BootStatus::BlankApp, 99));
        assert!(!compute_led_on(BootStatus::BlankApp, 100));
        assert!(!compute_led_on(BootStatus::BlankApp, 199));
        assert!(compute_led_on(BootStatus::BlankApp, 200));
        assert!(compute_led_on(BootStatus::BlankApp, 299));
        assert!(!compute_led_on(BootStatus::BlankApp, 300));
        assert!(!compute_led_on(BootStatus::BlankApp, 999));

        // BootLoopError: triple-blink (0..80, 160..240, 320..400)
        assert!(compute_led_on(BootStatus::BootLoopError, 0));
        assert!(compute_led_on(BootStatus::BootLoopError, 79));
        assert!(!compute_led_on(BootStatus::BootLoopError, 80));
        assert!(compute_led_on(BootStatus::BootLoopError, 160));
        assert!(compute_led_on(BootStatus::BootLoopError, 239));
        assert!(!compute_led_on(BootStatus::BootLoopError, 240));
        assert!(compute_led_on(BootStatus::BootLoopError, 320));
        assert!(compute_led_on(BootStatus::BootLoopError, 399));
        assert!(!compute_led_on(BootStatus::BootLoopError, 400));
        assert!(!compute_led_on(BootStatus::BootLoopError, 999));
    }

    #[test]
    fn runner_status_priority_transitions() {
        // Blank flash -> BlankApp
        let app_flash = MockMemFlash::new(APP_BASE, APP_SIZE);
        let commit_flash = MockMemFlash::new(COMMIT_BASE, COMMIT_SIZE);
        let mut transport = MockTransport::new();
        // Send an invalid RX_BLOCK (wrong length) to trigger flash_activity_ms=150 and flash_error=true
        let mut bad_frame = [0u8; 64];
        let flen = encode_frame(commands::RX_BLOCK, &[0u8; 4], &mut bad_frame).unwrap();
        transport.push_rx(&bad_frame[..flen]);

        let platform = MockPlatform {
            app_flash,
            commit_flash,
            transport,
            timer: MockTimer { ticks_available: 200 },
            reset_reason: ResetReason::PowerOn,
            guard_addr: None,
            mailbox_addr: None,
            boot_window_ms: 1500,
            idle_timeout_ms: None,
        };
        let mut runner = BootRunner::<MockPlatform, PAGE, COMMIT_SIZE, 16>::new(platform);
        assert_eq!(runner.status(), BootStatus::BlankApp);

        // Step processes 1 tick + the bad RX_BLOCK frame -> Flashing (150ms) takes priority over BootLoopError
        assert_eq!(runner.step(), RunnerAction::Continue);
        assert_eq!(runner.status(), BootStatus::Flashing);

        // Advance 150 ticks until flash_activity_ms decays to 0 -> BootLoopError (since flash_error=true)
        for _ in 0..150 {
            assert_eq!(runner.step(), RunnerAction::Continue);
        }
        assert_eq!(runner.status(), BootStatus::BootLoopError);
    }
}
