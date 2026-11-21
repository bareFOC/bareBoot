//! Monotonic bit-flip commit state tracking.
//!
//! Provides zero-erase commit tracking in a dedicated NOR flash sector by
//! sequentially programming 32-bit words:
//! - Erased word: `0xFFFF_FFFF`
//! - `STATE_UPDATE_IN_PROGRESS`: `0x4242_0001`
//! - `STATE_VALID`: `0x4242_0002`
//!
//! The current state is determined by the last non-`0xFFFF_FFFF` word.
//! If the sector is full, it is erased and restarted from offset 0.

use crate::{CoreError, FlashTarget};

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

pub fn decode_word(word: u32) -> CommitState {
    match word {
        STATE_VALID => CommitState::Valid,
        STATE_UPDATE_IN_PROGRESS => CommitState::UpdateInProgress,
        _ => CommitState::Invalid,
    }
}

/// Scans a slice of 32-bit words representing a commit sector.
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

/// Monotonic commit journal managing sequential word writes in a dedicated flash sector.
pub struct MonotonicCommitJournal<const SECTOR_SIZE: usize = 4096> {
    sector_base: u32,
    next_offset: usize,
    state: CommitState,
}

impl<const SECTOR_SIZE: usize> MonotonicCommitJournal<SECTOR_SIZE> {
    pub const fn new(sector_base: u32) -> Self {
        Self {
            sector_base,
            next_offset: 0,
            state: CommitState::Invalid,
        }
    }

    pub fn state(&self) -> CommitState {
        self.state
    }

    pub fn next_offset(&self) -> usize {
        self.next_offset
    }

    pub fn sector_base(&self) -> u32 {
        self.sector_base
    }

    /// Read the commit sector from flash and initialize journal state.
    pub fn init<F: FlashTarget>(&mut self, flash: &mut F) -> Result<CommitState, CoreError> {
        let mut word_buf = [0u8; 4];
        let mut current_state = CommitState::Invalid;
        let mut next_off = SECTOR_SIZE;

        let num_words = SECTOR_SIZE / 4;
        for i in 0..num_words {
            let addr = self.sector_base + (i * 4) as u32;
            flash.read_bytes(addr, &mut word_buf)?;
            let word = u32::from_le_bytes(word_buf);
            if word == STATE_ERASED {
                next_off = i * 4;
                break;
            }
            current_state = decode_word(word);
            if current_state == CommitState::Invalid {
                self.state = CommitState::Invalid;
                self.next_offset = (i + 1) * 4;
                return Ok(CommitState::Invalid);
            }
        }

        self.state = current_state;
        self.next_offset = next_off;
        Ok(self.state)
    }

    /// Record `UPDATE_IN_PROGRESS` without erasing the sector, unless full.
    pub fn mark_in_progress<F: FlashTarget>(&mut self, flash: &mut F) -> Result<(), CoreError> {
        if self.next_offset + 4 > SECTOR_SIZE {
            flash.erase_page(self.sector_base)?;
            self.next_offset = 0;
        }

        let addr = self.sector_base + self.next_offset as u32;
        let bytes = STATE_UPDATE_IN_PROGRESS.to_le_bytes();
        flash.write_page_bytes(addr, &bytes)?;
        self.next_offset += 4;
        self.state = CommitState::UpdateInProgress;
        Ok(())
    }

    /// Record `STATE_VALID` without erasing the sector, unless full.
    pub fn mark_valid<F: FlashTarget>(&mut self, flash: &mut F) -> Result<(), CoreError> {
        if self.next_offset + 4 > SECTOR_SIZE {
            flash.erase_page(self.sector_base)?;
            self.next_offset = 0;
        }

        let addr = self.sector_base + self.next_offset as u32;
        let bytes = STATE_VALID.to_le_bytes();
        flash.write_page_bytes(addr, &bytes)?;
        self.next_offset += 4;
        self.state = CommitState::Valid;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFlash {
        sector: [u8; 64],
        erase_count: usize,
        write_count: usize,
    }

    impl MockFlash {
        fn new() -> Self {
            Self {
                sector: [0xFF; 64],
                erase_count: 0,
                write_count: 0,
            }
        }
    }

    impl FlashTarget for MockFlash {
        fn page_size(&self) -> usize {
            64
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
            let offset = addr as usize;
            for (i, &b) in data.iter().enumerate() {
                // Emulate NOR bit-flipping
                self.sector[offset + i] &= b;
            }
            self.write_count += 1;
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
    fn test_monotonic_journal_transitions_without_erase() {
        let mut flash = MockFlash::new();
        let mut journal = MonotonicCommitJournal::<64>::new(0);

        // Initial scan on blank flash
        let state = journal.init(&mut flash).unwrap();
        assert_eq!(state, CommitState::Invalid);
        assert_eq!(journal.next_offset(), 0);
        assert_eq!(flash.erase_count, 0);

        // First update cycle: in progress then valid
        journal.mark_in_progress(&mut flash).unwrap();
        assert_eq!(journal.state(), CommitState::UpdateInProgress);
        assert_eq!(journal.next_offset(), 4);
        assert_eq!(flash.erase_count, 0);
        assert_eq!(flash.write_count, 1);

        journal.mark_valid(&mut flash).unwrap();
        assert_eq!(journal.state(), CommitState::Valid);
        assert_eq!(journal.next_offset(), 8);
        assert_eq!(flash.erase_count, 0);
        assert_eq!(flash.write_count, 2);

        // Verify readback after re-init
        let mut journal2 = MonotonicCommitJournal::<64>::new(0);
        let state2 = journal2.init(&mut flash).unwrap();
        assert_eq!(state2, CommitState::Valid);
        assert_eq!(journal2.next_offset(), 8);

        // Second update cycle: without erase
        journal2.mark_in_progress(&mut flash).unwrap();
        assert_eq!(journal2.state(), CommitState::UpdateInProgress);
        assert_eq!(journal2.next_offset(), 12);
        assert_eq!(flash.erase_count, 0);

        journal2.mark_valid(&mut flash).unwrap();
        assert_eq!(journal2.state(), CommitState::Valid);
        assert_eq!(journal2.next_offset(), 16);
        assert_eq!(flash.erase_count, 0);
    }

    #[test]
    fn test_monotonic_journal_sector_rollover() {
        let mut flash = MockFlash::new();
        let mut journal = MonotonicCommitJournal::<16>::new(0);

        // Fill 16 bytes: 4 words total
        journal.mark_in_progress(&mut flash).unwrap(); // off 0 -> 4
        journal.mark_valid(&mut flash).unwrap();       // off 4 -> 8
        journal.mark_in_progress(&mut flash).unwrap(); // off 8 -> 12
        journal.mark_valid(&mut flash).unwrap();       // off 12 -> 16
        assert_eq!(flash.erase_count, 0);
        assert_eq!(journal.next_offset(), 16);

        // Next update must trigger sector erase rollover
        journal.mark_in_progress(&mut flash).unwrap();
        assert_eq!(flash.erase_count, 1);
        assert_eq!(journal.next_offset(), 4);
        assert_eq!(journal.state(), CommitState::UpdateInProgress);
    }
}
