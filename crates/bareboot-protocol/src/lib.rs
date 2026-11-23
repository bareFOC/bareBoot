#![no_std]

pub const HEADER: [u8; 2] = [0x01, 0x88];
pub const TRAILER: [u8; 2] = [0x99, 0x03];
pub const MAX_FRAME_BYTES: usize = 192;

/// Signature identifying a Katapult/BareBoot-compatible bootloader ("CanBoot!").
pub const BAREBOOT_MAGIC: u64 = 0x2174_6f6f_426e_6143;

/// Magic token placed in SRAM mailbox to request staying in the bootloader indefinitely.
pub const REQUEST_BOOTLOADER: u64 = 0x5984_E3FA_6CA1_589B;

/// Katapult compatibility alias for REQUEST_BOOTLOADER.
pub const REQUEST_CANBOOT: u64 = 0x5984_E3FA_6CA1_589B;

/// Magic token placed in SRAM mailbox when application requests application launch or normal boot ("FISHAPP1").
pub const REQUEST_APP: u64 = 0x4649_5348_4150_5031;

/// Magic token placed in SRAM mailbox when bootloader jumps to user application.
pub const REQUEST_START_APP: u64 = 0x7b06_ec45_a9a8_243d;

/// Seed used by Katapult flashtool for 6-byte CAN UUID derivation from silicon UID.
pub const KATAPULT_UUID_SEED: u64 = 0xA162_31A7;

/// Soft-reset RAM mailbox base address on STM32H503.
pub const SOFT_RESET_MAILBOX_ADDR: usize = 0x2000_7FE0;

/// In-band UART string to trigger handsfree entry into bootloader.
pub const SERIAL_BL_REQ: &[u8] = b"~ \x1c Request Serial Bootloader!! ~";

/// 1-byte command sent over I2C/I3C to request entering bootloader.
pub const I2C_CMD_ENTER_BOOTLOADER: u8 = 0xF0;

/// MIPI I3C Common Command Codes (CCC) and defining bytes
pub mod ccc {
    /// MIPI I3C Controller broadcast address (0x7E)
    pub const I3C_BROADCAST_ADDR: u8 = 0x7E;

    // Common Command Codes (CCC)
    pub const DIRECT_ENEC: u8 = 0x00;
    pub const DIRECT_DISEC: u8 = 0x01;
    pub const DIRECT_RSTDAA: u8 = 0x06;
    pub const BCAST_ENTDAA: u8 = 0x07;
    pub const BCAST_RSTACT: u8 = 0x2A;
    pub const DIRECT_SETDASA: u8 = 0x87;
    pub const DIRECT_RSTACT: u8 = 0x9A;
    pub const DIRECT_RSTACT_ALT: u8 = 0xAA;

    // RSTACT Defining Bytes
    pub const RSTACT_DEF_NO_RESET: u8 = 0x00;
    pub const RSTACT_DEF_PERIPH_RESET: u8 = 0x01;
    pub const RSTACT_DEF_TGT_RESET: u8 = 0x02;
}

pub const I3C_BROADCAST_ADDR: u8 = ccc::I3C_BROADCAST_ADDR;
pub const CCC_BCAST_ENTDAA: u8 = ccc::BCAST_ENTDAA;
pub const CCC_BCAST_RSTACT: u8 = ccc::BCAST_RSTACT;
pub const CCC_DIRECT_RSTACT: u8 = ccc::DIRECT_RSTACT;
pub const CCC_DIRECT_SETDASA: u8 = ccc::DIRECT_SETDASA;
pub const RSTACT_DEF_TGT_RESET: u8 = ccc::RSTACT_DEF_TGT_RESET;

/// Standard CAN ID for host node reboot commands.
pub const CAN_ADMIN_REBOOT_ID: u32 = 0x3F0;

/// Reboot command opcode inside the CAN reboot frame.
pub const CAN_CMD_REBOOT: u8 = 0x02;

pub mod commands {
    pub const CONNECT: u8 = 0x11;
    pub const RX_BLOCK: u8 = 0x12;
    pub const RX_EOF: u8 = 0x13;
    pub const REQ_BLOCK: u8 = 0x14;
    pub const COMPLETE: u8 = 0x15;
    pub const GET_CANBUS_ID: u8 = 0x16;
}

pub mod responses {
    pub const ACK: u8 = 0xa0;
    pub const NACK: u8 = 0xf1;
    pub const COMMAND_ERROR: u8 = 0xf2;
    pub const COMMAND_BUSY: u8 = 0xf3;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    BufferTooSmall,
    PayloadNotWordAligned,
    WordCountTooLarge,
    InvalidFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameParse {
    NeedMore,
    Invalid { drop: usize },
    Complete {
        frame_len: usize,
        command: u8,
        word_count: usize,
        payload_offset: usize,
    },
}

pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &byte in data {
        let mut data = byte ^ (crc & 0xff) as u8;
        data ^= data << 4;
        crc = (((data as u16) << 8) | (crc >> 8))
            ^ ((data as u16) >> 4)
            ^ ((data as u16) << 3);
    }
    crc
}

pub fn scan(buf: &[u8]) -> FrameParse {
    // Find a Katapult header. Anything before it cannot be part of a frame.
    let Some(header_pos) = find_header(buf) else {
        // Keep a single trailing partial header byte available.
        if buf.last() == Some(&HEADER[0]) {
            return FrameParse::NeedMore;
        }
        return FrameParse::Invalid { drop: buf.len() };
    };

    if header_pos != 0 {
        return FrameParse::Invalid {
            drop: header_pos,
        };
    }

    if buf.len() < 4 {
        return FrameParse::NeedMore;
    }

    let word_count = buf[3] as usize;
    let frame_len = 4 + word_count * 4 + 4;
    if frame_len < 8 || frame_len > MAX_FRAME_BYTES {
        return FrameParse::Invalid { drop: 1 };
    }
    if buf.len() < frame_len {
        return FrameParse::NeedMore;
    }

    // CRC is over command byte, length byte, and payload.
    let crc_input_len = 2 + word_count * 4;
    let calculated = crc16_ccitt(&buf[2..2 + crc_input_len]);
    let received = u16::from_le_bytes([buf[frame_len - 4], buf[frame_len - 3]]);
    if calculated != received {
        return FrameParse::Invalid { drop: 1 };
    }

    if buf[frame_len - 2] != TRAILER[0] || buf[frame_len - 1] != TRAILER[1] {
        return FrameParse::Invalid { drop: 1 };
    }

    FrameParse::Complete {
        frame_len,
        command: buf[2],
        word_count,
        payload_offset: 4,
    }
}

fn find_header(buf: &[u8]) -> Option<usize> {
    buf.windows(2)
        .position(|window| window == HEADER)
}

pub fn encode_frame(command: u8, payload: &[u8], out: &mut [u8]) -> Result<usize, ProtocolError> {
    if payload.len() % 4 != 0 {
        return Err(ProtocolError::PayloadNotWordAligned);
    }
    let word_count = payload.len() / 4;
    if word_count > u8::MAX as usize {
        return Err(ProtocolError::WordCountTooLarge);
    }
    let frame_len = 4 + payload.len() + 4;
    if frame_len > MAX_FRAME_BYTES {
        return Err(ProtocolError::WordCountTooLarge);
    }
    if out.len() < frame_len {
        return Err(ProtocolError::BufferTooSmall);
    }

    out[0] = HEADER[0];
    out[1] = HEADER[1];
    out[2] = command;
    out[3] = word_count as u8;
    out[4..4 + payload.len()].copy_from_slice(payload);

    let crc = crc16_ccitt(&out[2..2 + 2 + payload.len()]);
    out[4 + payload.len()] = (crc & 0xff) as u8;
    out[5 + payload.len()] = (crc >> 8) as u8;
    out[6 + payload.len()] = TRAILER[0];
    out[7 + payload.len()] = TRAILER[1];

    Ok(frame_len)
}

pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

pub fn write_u32_le(data: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    data[offset..offset + 4].copy_from_slice(&bytes);
}

/// Build the variable payload returned by CONNECT.
///
/// The buffer must have room for 16 fixed bytes plus MCU and version strings,
/// each rounded up to a multiple of four bytes.
pub fn build_connect_payload(
    command: u8,
    protocol_version: u32,
    app_base: u32,
    block_size: u32,
    mcu_type: &[u8],
    software_version: &[u8],
    out: &mut [u8],
) -> Result<usize, ProtocolError> {
    let mcu_words = (mcu_type.len() + 3) / 4;
    let version_words = (software_version.len() + 3) / 4;
    let payload_len = (5 + mcu_words + version_words) * 4;
    if out.len() < payload_len {
        return Err(ProtocolError::BufferTooSmall);
    }

    // The frame payload begins with the little-endian command-derived word.
    // Protocol leaves orig_command in the first word.
    write_u32_le(out, 0, command as u32);
    write_u32_le(out, 4, protocol_version);
    write_u32_le(out, 8, app_base);
    write_u32_le(out, 12, block_size);

    let mut pos = 16;
    out[pos..pos + mcu_type.len()].copy_from_slice(mcu_type);
    pos += mcu_words * 4;
    out[pos..pos + software_version.len()].copy_from_slice(software_version);
    let _ = pos;
    Ok(payload_len)
}

/// Standard IEEE 802.3 CRC32 (polynomial 0xEDB88320)
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[inline]
fn fasthash_mix(mut h: u64) -> u64 {
    h ^= h >> 23;
    h = h.wrapping_mul(0x2127_599b_f432_5c37);
    h ^= h >> 47;
    h
}

/// 64-bit FastHash by Zilong Tan, matching Katapult flashtool.
pub fn fasthash64(buf: &[u8], seed: u64) -> u64 {
    const M: u64 = 0x8803_55f2_1e6d_1965;
    let len = buf.len();
    let mut h = seed ^ (len as u64).wrapping_mul(M);

    let chunks = buf.chunks_exact(8);
    let remainder = chunks.remainder();
    for chunk in chunks {
        let v = u64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        h ^= fasthash_mix(v);
        h = h.wrapping_mul(M);
    }

    let mut v = 0u64;
    match remainder.len() {
        7 => {
            v ^= (remainder[6] as u64) << 48;
            v ^= (remainder[5] as u64) << 40;
            v ^= (remainder[4] as u64) << 32;
            v ^= (remainder[3] as u64) << 24;
            v ^= (remainder[2] as u64) << 16;
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        6 => {
            v ^= (remainder[5] as u64) << 40;
            v ^= (remainder[4] as u64) << 32;
            v ^= (remainder[3] as u64) << 24;
            v ^= (remainder[2] as u64) << 16;
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        5 => {
            v ^= (remainder[4] as u64) << 32;
            v ^= (remainder[3] as u64) << 24;
            v ^= (remainder[2] as u64) << 16;
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        4 => {
            v ^= (remainder[3] as u64) << 24;
            v ^= (remainder[2] as u64) << 16;
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        3 => {
            v ^= (remainder[2] as u64) << 16;
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        2 => {
            v ^= (remainder[1] as u64) << 8;
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        1 => {
            v ^= remainder[0] as u64;
            h ^= fasthash_mix(v);
            h = h.wrapping_mul(M);
        }
        _ => {}
    }

    fasthash_mix(h)
}

/// Derive 6-byte Katapult CAN UUID from silicon UID using FastHash64 and seed 0xA16231A7.
pub fn katapult_can_uuid(uid: &[u8]) -> [u8; 6] {
    let hash = fasthash64(uid, KATAPULT_UUID_SEED);
    let bytes = hash.to_le_bytes();
    let mut uuid = [0u8; 6];
    uuid.copy_from_slice(&bytes[..6]);
    uuid
}

/// Deterministic fallback 7-bit bus address from silicon UID registers: 0x10 + ((UID[0] ^ UID[1] ^ UID[2]) & 0x3F).
pub fn fallback_address(uid: &[u32; 3]) -> u8 {
    (0x10 + ((uid[0] ^ uid[1] ^ uid[2]) & 0x3F)) as u8
}

/// 32-byte cross-reset RAM mailbox placed at 0x2000_7FE0..0x2000_7FFF on STM32H503.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftResetMailbox {
    pub magic: u64,
    pub dynamic_node_id: u8,
    pub bus_type: u8,
    pub flags: u16,
    pub can_assigned_id: u32,
    pub reserved: [u32; 3],
    pub crc32: u32,
}

impl SoftResetMailbox {
    pub const SIZE: usize = 32;
    pub const PAYLOAD_LEN: usize = 28;
    pub const ADDRESS: usize = SOFT_RESET_MAILBOX_ADDR;

    pub const BUS_TYPE_I2C: u8 = 0;
    pub const BUS_TYPE_I3C: u8 = 1;
    pub const BUS_TYPE_CAN: u8 = 2;
    pub const BUS_TYPE_UART: u8 = 3;
    pub const BUS_TYPE_USB: u8 = 4;

    pub fn new(magic: u64, dynamic_node_id: u8, bus_type: u8) -> Self {
        let mut mb = Self {
            magic,
            dynamic_node_id,
            bus_type,
            flags: 0,
            can_assigned_id: 0,
            reserved: [0; 3],
            crc32: 0,
        };
        mb.crc32 = mb.compute_crc32();
        mb
    }

    pub fn compute_crc32(&self) -> u32 {
        let bytes = self.to_bytes();
        crc32(&bytes[..Self::PAYLOAD_LEN])
    }

    pub fn is_valid(&self) -> bool {
        (self.magic == REQUEST_CANBOOT || self.magic == REQUEST_APP)
            && self.crc32 == self.compute_crc32()
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&self.magic.to_le_bytes());
        bytes[8] = self.dynamic_node_id;
        bytes[9] = self.bus_type;
        bytes[10..12].copy_from_slice(&self.flags.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.can_assigned_id.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.reserved[0].to_le_bytes());
        bytes[20..24].copy_from_slice(&self.reserved[1].to_le_bytes());
        bytes[24..28].copy_from_slice(&self.reserved[2].to_le_bytes());
        bytes[28..32].copy_from_slice(&self.crc32.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let magic = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let dynamic_node_id = bytes[8];
        let bus_type = bytes[9];
        let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
        let can_assigned_id = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let reserved = [
            u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
        ];
        let crc32 = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
        Self {
            magic,
            dynamic_node_id,
            bus_type,
            flags,
            can_assigned_id,
            reserved,
            crc32,
        }
    }
}
