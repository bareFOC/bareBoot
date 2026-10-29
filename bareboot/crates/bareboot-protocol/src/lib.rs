#![no_std]

pub const HEADER: [u8; 2] = [0x01, 0x88];
pub const TRAILER: [u8; 2] = [0x99, 0x03];
pub const MAX_FRAME_BYTES: usize = 192;

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
