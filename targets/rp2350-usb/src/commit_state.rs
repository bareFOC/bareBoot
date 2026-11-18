//! Persistent update commit state for BareBoot (RP2350).
//!
//! The commit record lives in the last 4 KB flash sector of BareBoot's own
//! 32 KB slot (flash offset 0x7000, XIP 0x10007000). It is never inside the
//! application slot, so a failed or interrupted transfer cannot poison a
//! previously valid application image, and the two-segment UF2 layout is
//! unchanged.
//!
//! Record format (16 bytes, one flash page write):
//!   word 0: MAGIC          (0x4242_5354, "BBST")
//!   word 1: state          (VALID = 1, UPDATE_IN_PROGRESS = 2)
//!   word 2: !state         (bitwise complement, for corruption detection)
//!   word 3: MAGIC ^ 0xFFFFFFFF
//!
//! A record is accepted only if all four words match. Anything else — erased
//! flash, a torn write, or bit rot — is treated as NOT VALID, which keeps the
//! device in recovery rather than booting a possibly corrupt application.

#[cfg(target_arch = "arm")]
use crate::boot::COMMIT_STATE_XIP;

/// Magic word identifying a BareBoot commit record ("BBST").
pub const MAGIC: u32 = 0x4242_5354;

/// State value: a complete, host-verified image is committed.
pub const STATE_VALID: u32 = 1;

/// State value: an update transfer has started but not committed.
pub const STATE_UPDATE_IN_PROGRESS: u32 = 2;

/// Size of the commit record in bytes.
pub const RECORD_LEN: usize = 16;

/// Enumeration of decoded commit states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitState {
    /// A fully verified image is committed and safe to boot.
    Valid,
    /// An update is in progress; never boot the application.
    UpdateInProgress,
    /// The record is erased, torn, corrupt, or unknown; never boot the app.
    Invalid,
}

impl CommitState {
    /// Returns true only for `Valid`.
    pub fn is_valid(self) -> bool {
        matches!(self, CommitState::Valid)
    }
}

/// Encode a state value into a 16-byte record (pure, host-testable).
pub fn encode_record(state: u32) -> [u8; RECORD_LEN] {
    let words = [MAGIC, state, !state, !MAGIC];
    let mut out = [0u8; RECORD_LEN];
    for (i, word) in words.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// Decode a 16-byte record (pure, host-testable).
///
/// Any mismatch in magic, state, complement, or inverse-magic yields
/// `CommitState::Invalid`, which is deliberately conservative.
pub fn decode_record(bytes: &[u8; RECORD_LEN]) -> CommitState {
    let word = |i: usize| -> u32 {
        u32::from_le_bytes([bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2], bytes[i * 4 + 3]])
    };
    if word(0) != MAGIC || word(3) != !MAGIC {
        return CommitState::Invalid;
    }
    let state = word(1);
    if word(2) != !state {
        return CommitState::Invalid;
    }
    match state {
        STATE_VALID => CommitState::Valid,
        STATE_UPDATE_IN_PROGRESS => CommitState::UpdateInProgress,
        _ => CommitState::Invalid,
    }
}

/// Read the commit record from flash (hardware only, ARM target).
#[cfg(target_arch = "arm")]
pub fn read() -> CommitState {
    let mut bytes = [0u8; RECORD_LEN];
    for (i, byte) in bytes.iter_mut().enumerate() {
        // XIP_NOCACHE alias (0x14000000) forces a fresh flash read.
        let addr = 0x1400_0000u32 + (COMMIT_STATE_XIP - 0x1000_0000) + i as u32;
        *byte = unsafe { core::ptr::read_volatile(addr as *const u8) };
    }
    decode_record(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let valid = encode_record(STATE_VALID);
        assert_eq!(decode_record(&valid), CommitState::Valid);
        let in_progress = encode_record(STATE_UPDATE_IN_PROGRESS);
        assert_eq!(decode_record(&in_progress), CommitState::UpdateInProgress);
    }

    #[test]
    fn erased_flash_is_invalid() {
        assert_eq!(decode_record(&[0xff; RECORD_LEN]), CommitState::Invalid);
        assert_eq!(decode_record(&[0x00; RECORD_LEN]), CommitState::Invalid);
    }

    #[test]
    fn corrupt_record_is_invalid() {
        let mut bad = encode_record(STATE_VALID);
        bad[4] ^= 0x01; // corrupt complement word
        assert_eq!(decode_record(&bad), CommitState::Invalid);
        let mut bad = encode_record(STATE_VALID);
        bad[0] ^= 0x01; // corrupt magic
        assert_eq!(decode_record(&bad), CommitState::Invalid);
        let mut bad = encode_record(STATE_VALID);
        bad[8] = 0x09; // unknown state value
        bad[9] = 0x00;
        bad[10] = 0x00;
        bad[11] = 0x00;
        bad[12] = !9u32.to_le_bytes()[0];
        // complement no longer matches -> Invalid
        assert_eq!(decode_record(&bad), CommitState::Invalid);
    }

    #[test]
    fn state_is_valid_helper() {
        assert!(CommitState::Valid.is_valid());
        assert!(!CommitState::UpdateInProgress.is_valid());
        assert!(!CommitState::Invalid.is_valid());
    }
}
