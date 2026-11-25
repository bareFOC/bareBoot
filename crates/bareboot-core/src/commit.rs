//! Monotonic bit-flip commit state tracking with alignment-generic records.
//!
//! Provides zero-erase commit tracking in a dedicated NOR flash sector by
//! sequentially programming `CommitRecord` slots spaced by `max(16, SLOT_ALIGN)`
//! bytes (`SLOT_ALIGN` in `{4, 8, 16, 32}`):
//! - Erased slot: all `0xFF`
//! - `STATE_UPDATE_IN_PROGRESS`: `0x4242_0001`
//! - `STATE_VALID`: `0x4242_0002` with exact `(app_len, app_crc32)`
//!
//! The current state is determined by the last non-erased record.
//! If the sector is full or a torn/corrupt slot was observed, the sector is
//! erased on the next transition and restarted from offset 0.

use crate::{CoreError, FlashTarget};

pub const COMMIT_MAGIC: u32 = 0x4242_5354; // "BBST"
pub const STATE_ERASED: u32 = 0xFFFF_FFFF;
pub const STATE_UPDATE_IN_PROGRESS: u32 = 0x4242_0001;
pub const STATE_VALID: u32 = 0x4242_0002;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitState {
    Valid,
    UpdateInProgress,
    Invalid,
}

impl CommitState {
    pub fn is_valid(self) -> bool {
        matches!(self, CommitState::Valid)
    }
}

/// Fixed 16-byte commit record stored per journal slot (padded with `0xFF`
/// up to `SLOT_ALIGN` when `SLOT_ALIGN > 16`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitRecord {
    pub magic: u32,
    pub state: u32,
    pub app_len: u32,
    pub app_crc32: u32,
}

impl CommitRecord {
    pub const SIZE: usize = 16;

    pub const fn in_progress() -> Self {
        Self {
            magic: COMMIT_MAGIC,
            state: STATE_UPDATE_IN_PROGRESS,
            app_len: 0,
            app_crc32: 0,
        }
    }

    pub const fn valid(app_len: u32, app_crc32: u32) -> Self {
        Self {
            magic: COMMIT_MAGIC,
            state: STATE_VALID,
            app_len,
            app_crc32,
        }
    }

    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        let m = self.magic.to_le_bytes();
        let s = self.state.to_le_bytes();
        let l = self.app_len.to_le_bytes();
        let c = self.app_crc32.to_le_bytes();
        [
            m[0], m[1], m[2], m[3],
            s[0], s[1], s[2], s[3],
            l[0], l[1], l[2], l[3],
            c[0], c[1], c[2], c[3],
        ]
    }

    pub const fn from_bytes(buf: &[u8; Self::SIZE]) -> Self {
        Self {
            magic: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            state: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            app_len: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            app_crc32: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
        }
    }
}

pub fn decode_word(word: u32) -> CommitState {
    match word {
        STATE_VALID => CommitState::Valid,
        STATE_UPDATE_IN_PROGRESS => CommitState::UpdateInProgress,
        _ => CommitState::Invalid,
    }
}

/// Scans a slice of 32-bit words representing a legacy word-based commit sector.
/// Returns `(current_state, next_free_byte_offset)`.
pub fn scan_words(words: &[u32]) -> (CommitState, usize) {
    let mut state = CommitState::Invalid;
    let mut next_offset = words.len() * 4;

    for (i, &word) in words.iter().enumerate() {
        if word == STATE_ERASED {
            next_offset = i * 4;
            break;
        }
        state = decode_word(word);
        if state == CommitState::Invalid {
            return (CommitState::Invalid, (i + 1) * 4);
        }
    }

    (state, next_offset)
}

/// Alignment-generic monotonic commit journal managing sequential `CommitRecord`
/// writes in a dedicated flash sector.
///
/// `SLOT_ALIGN` must be one of `4`, `8`, `16`, or `32` bytes. Each slot occupies
/// `max(16, SLOT_ALIGN)` bytes in flash.
pub struct MonotonicCommitJournal<const SECTOR_SIZE: usize = 4096, const SLOT_ALIGN: usize = 4> {
    sector_base: u32,
    next_offset: usize,
    state: CommitState,
    app_len: u32,
    app_crc32: u32,
    needs_erase: bool,
}

impl<const SECTOR_SIZE: usize, const SLOT_ALIGN: usize>
    MonotonicCommitJournal<SECTOR_SIZE, SLOT_ALIGN>
{
    pub const SLOT_STRIDE: usize = if SLOT_ALIGN > CommitRecord::SIZE {
        SLOT_ALIGN
    } else {
        CommitRecord::SIZE
    };

    pub const fn new(sector_base: u32) -> Self {
        const {
            assert!(
                SLOT_ALIGN == 4 || SLOT_ALIGN == 8 || SLOT_ALIGN == 16 || SLOT_ALIGN == 32,
                "SLOT_ALIGN must be 4, 8, 16, or 32"
            );
            assert!(
                SECTOR_SIZE >= Self::SLOT_STRIDE && SECTOR_SIZE % Self::SLOT_STRIDE == 0,
                "SECTOR_SIZE must be a positive multiple of slot stride"
            );
        }
        Self {
            sector_base,
            next_offset: 0,
            state: CommitState::Invalid,
            app_len: 0,
            app_crc32: 0,
            needs_erase: false,
        }
    }

    pub const fn slot_stride() -> usize {
        Self::SLOT_STRIDE
    }

    pub fn state(&self) -> CommitState {
        self.state
    }

    pub fn app_len(&self) -> u32 {
        self.app_len
    }

    pub fn app_crc32(&self) -> u32 {
        self.app_crc32
    }

    pub fn next_offset(&self) -> usize {
        self.next_offset
    }

    pub fn sector_base(&self) -> u32 {
        self.sector_base
    }

    /// Read the commit sector from flash and initialize journal state.
    pub fn init<F: FlashTarget>(&mut self, flash: &mut F) -> Result<CommitState, CoreError> {
        let mut buf = [0u8; CommitRecord::SIZE];
        let mut current_state = CommitState::Invalid;
        let mut current_len = 0u32;
        let mut current_crc = 0u32;
        let mut next_off = SECTOR_SIZE;
        let mut torn = false;

        let num_slots = SECTOR_SIZE / Self::SLOT_STRIDE;
        for i in 0..num_slots {
            let off = i * Self::SLOT_STRIDE;
            let addr = self.sector_base + off as u32;
            flash.read_bytes(addr, &mut buf)?;
            if buf == [0xFF; CommitRecord::SIZE] {
                next_off = off;
                break;
            }

            let rec = CommitRecord::from_bytes(&buf);
            if rec.magic != COMMIT_MAGIC {
                current_state = CommitState::Invalid;
                current_len = 0;
                current_crc = 0;
                next_off = off + Self::SLOT_STRIDE;
                torn = true;
                break;
            }

            match rec.state {
                STATE_VALID if rec.app_len > 0 && rec.app_len != STATE_ERASED => {
                    current_state = CommitState::Valid;
                    current_len = rec.app_len;
                    current_crc = rec.app_crc32;
                }
                STATE_UPDATE_IN_PROGRESS => {
                    current_state = CommitState::UpdateInProgress;
                    current_len = 0;
                    current_crc = 0;
                }
                _ => {
                    current_state = CommitState::Invalid;
                    current_len = 0;
                    current_crc = 0;
                    next_off = off + Self::SLOT_STRIDE;
                    torn = true;
                    break;
                }
            }
        }

        self.state = current_state;
        self.app_len = current_len;
        self.app_crc32 = current_crc;
        self.next_offset = next_off;
        self.needs_erase = torn;
        Ok(self.state)
    }

    fn append_record<F: FlashTarget>(
        &mut self,
        flash: &mut F,
        rec: CommitRecord,
    ) -> Result<(), CoreError> {
        if self.needs_erase || self.next_offset + Self::SLOT_STRIDE > SECTOR_SIZE {
            flash.erase_page(self.sector_base)?;
            self.next_offset = 0;
            self.needs_erase = false;
        }

        let mut slot = [0xFFu8; 32];
        slot[..CommitRecord::SIZE].copy_from_slice(&rec.to_bytes());
        let addr = self.sector_base + self.next_offset as u32;
        flash.write_page_bytes(addr, &slot[..Self::SLOT_STRIDE])?;
        self.next_offset += Self::SLOT_STRIDE;
        Ok(())
    }

    /// Record `UPDATE_IN_PROGRESS` without erasing the sector, unless full or torn.
    pub fn mark_in_progress<F: FlashTarget>(&mut self, flash: &mut F) -> Result<(), CoreError> {
        self.append_record(flash, CommitRecord::in_progress())?;
        self.state = CommitState::UpdateInProgress;
        self.app_len = 0;
        self.app_crc32 = 0;
        Ok(())
    }

    /// Record `STATE_VALID(app_len, app_crc32)` without erasing the sector, unless full or torn.
    pub fn mark_valid<F: FlashTarget>(
        &mut self,
        flash: &mut F,
        app_len: u32,
        app_crc32: u32,
    ) -> Result<(), CoreError> {
        if app_len == 0 || app_len == STATE_ERASED {
            return Err(CoreError::InvalidAddress);
        }
        self.append_record(flash, CommitRecord::valid(app_len, app_crc32))?;
        self.state = CommitState::Valid;
        self.app_len = app_len;
        self.app_crc32 = app_crc32;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAlignedFlash<const ALIGN: usize, const SIZE: usize = 256> {
        sector: [u8; SIZE],
        erase_count: usize,
        write_count: usize,
        last_write_len: usize,
    }

    impl<const ALIGN: usize, const SIZE: usize> MockAlignedFlash<ALIGN, SIZE> {
        fn new() -> Self {
            Self {
                sector: [0xFF; SIZE],
                erase_count: 0,
                write_count: 0,
                last_write_len: 0,
            }
        }
    }

    impl<const ALIGN: usize, const SIZE: usize> FlashTarget for MockAlignedFlash<ALIGN, SIZE> {
        fn page_size(&self) -> usize {
            SIZE
        }
        fn app_base(&self) -> u32 {
            0x1000
        }
        fn erase_page(&mut self, _addr: u32) -> Result<(), CoreError> {
            self.sector.fill(0xFF);
            self.erase_count += 1;
            Ok(())
        }
        fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
            if addr as usize % ALIGN != 0 || data.len() % ALIGN != 0 {
                return Err(CoreError::InvalidAddress);
            }
            let offset = addr as usize;
            for (i, &b) in data.iter().enumerate() {
                // Enforce NOR one-to-zero bit programming without intervening erase
                if (self.sector[offset + i] & b) != b {
                    return Err(CoreError::Flash);
                }
                self.sector[offset + i] &= b;
            }
            self.write_count += 1;
            self.last_write_len = data.len();
            Ok(())
        }
        fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
            let offset = addr as usize;
            out.copy_from_slice(&self.sector[offset..offset + out.len()]);
            Ok(())
        }
        fn read_uid(&mut self, out: &mut [u8; 12]) {
            out.fill(0xAA);
        }
        unsafe fn boot_jump(_app_entry: u32) -> ! {
            unreachable!()
        }
    }

    #[test]
    fn test_scan_words() {
        let empty = [STATE_ERASED; 16];
        let (state, offset) = scan_words(&empty);
        assert_eq!(state, CommitState::Invalid);
        assert_eq!(offset, 0);

        let in_progress = [STATE_UPDATE_IN_PROGRESS, STATE_ERASED, STATE_ERASED];
        let (state, offset) = scan_words(&in_progress);
        assert_eq!(state, CommitState::UpdateInProgress);
        assert_eq!(offset, 4);

        let valid = [STATE_UPDATE_IN_PROGRESS, STATE_VALID, STATE_ERASED];
        let (state, offset) = scan_words(&valid);
        assert_eq!(state, CommitState::Valid);
        assert_eq!(offset, 8);

        let corrupt = [STATE_UPDATE_IN_PROGRESS, 0x1234_5678, STATE_ERASED];
        let (state, _) = scan_words(&corrupt);
        assert_eq!(state, CommitState::Invalid);
    }

    #[test]
    fn test_monotonic_journal_transitions_without_erase_16b_align() {
        let mut flash = MockAlignedFlash::<16, 128>::new();
        let mut journal = MonotonicCommitJournal::<128, 16>::new(0);

        // Initial scan on blank flash
        let state = journal.init(&mut flash).unwrap();
        assert_eq!(state, CommitState::Invalid);
        assert_eq!(journal.next_offset(), 0);
        assert_eq!(flash.erase_count, 0);

        // First update cycle: in progress then valid with length & CRC32
        journal.mark_in_progress(&mut flash).unwrap();
        assert_eq!(journal.state(), CommitState::UpdateInProgress);
        assert_eq!(journal.next_offset(), 16);
        assert_eq!(flash.erase_count, 0);
        assert_eq!(flash.write_count, 1);
        assert_eq!(flash.last_write_len, 16);

        journal.mark_valid(&mut flash, 0x1_2000, 0xDEAD_BEEF).unwrap();
        assert_eq!(journal.state(), CommitState::Valid);
        assert_eq!(journal.app_len(), 0x1_2000);
        assert_eq!(journal.app_crc32(), 0xDEAD_BEEF);
        assert_eq!(journal.next_offset(), 32);
        assert_eq!(flash.erase_count, 0);
        assert_eq!(flash.write_count, 2);

        // Verify readback after re-init
        let mut journal2 = MonotonicCommitJournal::<128, 16>::new(0);
        let state2 = journal2.init(&mut flash).unwrap();
        assert_eq!(state2, CommitState::Valid);
        assert_eq!(journal2.app_len(), 0x1_2000);
        assert_eq!(journal2.app_crc32(), 0xDEAD_BEEF);
        assert_eq!(journal2.next_offset(), 32);

        // Second update cycle: without erase
        journal2.mark_in_progress(&mut flash).unwrap();
        assert_eq!(journal2.state(), CommitState::UpdateInProgress);
        assert_eq!(journal2.next_offset(), 48);
        assert_eq!(flash.erase_count, 0);

        journal2.mark_valid(&mut flash, 0x1_4000, 0xCAFE_BABE).unwrap();
        assert_eq!(journal2.state(), CommitState::Valid);
        assert_eq!(journal2.app_len(), 0x1_4000);
        assert_eq!(journal2.app_crc32(), 0xCAFE_BABE);
        assert_eq!(journal2.next_offset(), 64);
        assert_eq!(flash.erase_count, 0);
    }

    #[test]
    fn test_monotonic_journal_all_alignments_4_8_16_32() {
        let mut f4 = MockAlignedFlash::<4, 64>::new();
        let mut j4 = MonotonicCommitJournal::<64, 4>::new(0);
        j4.mark_in_progress(&mut f4).unwrap();
        j4.mark_valid(&mut f4, 1024, 0x1122_3344).unwrap();
        assert_eq!(j4.next_offset(), 32);
        assert_eq!(f4.last_write_len, 16);

        let mut f8 = MockAlignedFlash::<8, 64>::new();
        let mut j8 = MonotonicCommitJournal::<64, 8>::new(0);
        j8.mark_in_progress(&mut f8).unwrap();
        j8.mark_valid(&mut f8, 2048, 0x5566_7788).unwrap();
        assert_eq!(j8.next_offset(), 32);
        assert_eq!(f8.last_write_len, 16);

        let mut f32 = MockAlignedFlash::<32, 128>::new();
        let mut j32 = MonotonicCommitJournal::<128, 32>::new(0);
        j32.mark_in_progress(&mut f32).unwrap();
        j32.mark_valid(&mut f32, 4096, 0x99AA_BBCC).unwrap();
        assert_eq!(j32.next_offset(), 64);
        assert_eq!(f32.last_write_len, 32);

        let mut j32_reinit = MonotonicCommitJournal::<128, 32>::new(0);
        assert_eq!(j32_reinit.init(&mut f32).unwrap(), CommitState::Valid);
        assert_eq!(j32_reinit.app_len(), 4096);
        assert_eq!(j32_reinit.app_crc32(), 0x99AA_BBCC);
    }

    #[test]
    fn test_monotonic_journal_sector_rollover() {
        let mut flash = MockAlignedFlash::<16, 64>::new();
        let mut journal = MonotonicCommitJournal::<64, 16>::new(0);

        // Fill 64 bytes: 4 slots of 16 bytes
        journal.mark_in_progress(&mut flash).unwrap(); // off 0 -> 16
        journal.mark_valid(&mut flash, 100, 1).unwrap(); // off 16 -> 32
        journal.mark_in_progress(&mut flash).unwrap(); // off 32 -> 48
        journal.mark_valid(&mut flash, 200, 2).unwrap(); // off 48 -> 64
        assert_eq!(flash.erase_count, 0);
        assert_eq!(journal.next_offset(), 64);

        // Next update must trigger sector erase rollover
        journal.mark_in_progress(&mut flash).unwrap();
        assert_eq!(flash.erase_count, 1);
        assert_eq!(journal.next_offset(), 16);
        assert_eq!(journal.state(), CommitState::UpdateInProgress);
    }

    #[test]
    fn test_torn_record_recovery_forces_clean_erase_on_next_update() {
        let mut flash = MockAlignedFlash::<16, 64>::new();
        let mut journal = MonotonicCommitJournal::<64, 16>::new(0);
        journal.mark_in_progress(&mut flash).unwrap(); // slot 0
        // Simulate torn write at slot 1 (non-matching magic)
        flash.sector[16..20].copy_from_slice(&0x1234_5678u32.to_le_bytes());

        let mut boot_journal = MonotonicCommitJournal::<64, 16>::new(0);
        assert_eq!(boot_journal.init(&mut flash).unwrap(), CommitState::Invalid);

        // Subsequent OTA recovery erases the torn sector cleanly and succeeds
        boot_journal.mark_in_progress(&mut flash).unwrap();
        assert_eq!(flash.erase_count, 1);
        assert_eq!(boot_journal.next_offset(), 16);
        boot_journal.mark_valid(&mut flash, 4096, 0xA5A5_5A5A).unwrap();

        let mut verify = MonotonicCommitJournal::<64, 16>::new(0);
        assert_eq!(verify.init(&mut flash).unwrap(), CommitState::Valid);
        assert_eq!(verify.app_len(), 4096);
        assert_eq!(verify.app_crc32(), 0xA5A5_5A5A);
    }
}
