use bareboot_protocol::{
    self, FrameParse, ProtocolError, commands, responses, read_u32_le, write_u32_le,
    build_connect_payload,
};
use crate::CoreError;
use crate::writer::PagedWriter;

const MAX_FRAME: usize = bareboot_protocol::MAX_FRAME_BYTES;

pub fn dispatch_one<const PAGE: usize, F>(
    writer: &mut PagedWriter<F, PAGE>,
    input: &[u8],
    output: &mut [u8],
) -> Result<usize, CoreError>
where
    F: crate::FlashTarget,
{
    let parsed = bareboot_protocol::scan(input);
    let (command, word_count, payload_offset) = match parsed {
        FrameParse::Complete {
            command,
            word_count,
            payload_offset,
            ..
        } => (command, word_count, payload_offset),
        FrameParse::NeedMore => return Ok(0),
        FrameParse::Invalid { .. } => return Err(CoreError::InvalidAddress),
    };

    let payload_len = word_count * 4;
    if payload_offset + payload_len > input.len() || output.len() < MAX_FRAME {
        return Err(CoreError::BufferTooSmall);
    }
    let payload = &input[payload_offset..payload_offset + payload_len];

    match command {
        commands::CONNECT => {
            let mut ack_payload = [0u8; MAX_FRAME];
            let cfg = writer.config();
            let len = build_connect_payload(
                commands::CONNECT,
                cfg.protocol_version,
                cfg.app_base,
                cfg.block_size as u32,
                cfg.mcu_type,
                cfg.software_version,
                &mut ack_payload,
            )
            .map_err(|_| CoreError::BufferTooSmall)?;
            encode_ack(responses::ACK, &ack_payload[..len], output)
        }
        commands::RX_BLOCK => {
            if payload.len() < 4 {
                return encode_command_error(output);
            }
            let addr = read_u32_le(payload, 0);
            let data = &payload[4..];
            let block_size = writer.config().block_size;
            if data.len() != block_size {
                return encode_command_error(output);
            }
            match writer.write_block(addr, data) {
                Ok(()) => {
                    let mut ack_payload = [0u8; 8];
                    write_u32_le(&mut ack_payload, 0, commands::RX_BLOCK as u32);
                    write_u32_le(&mut ack_payload, 4, addr);
                    encode_ack(responses::ACK, &ack_payload, output)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::RX_EOF => {
            match writer.finish() {
                Ok(pages) => {
                    let mut ack_payload = [0u8; 8];
                    write_u32_le(&mut ack_payload, 0, commands::RX_EOF as u32);
                    write_u32_le(&mut ack_payload, 4, pages);
                    encode_ack(responses::ACK, &ack_payload, output)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::REQ_BLOCK => {
            if payload.len() < 4 {
                return encode_command_error(output);
            }
            let addr = read_u32_le(payload, 0);
            let block_size = writer.config().block_size;
            let mut data = [0u8; 128];
            if block_size > data.len() {
                return encode_command_error(output);
            }
            match writer.read_bytes(addr, &mut data[..block_size]) {
                Ok(()) => {
                    let mut ack_payload = [0u8; 8 + 128];
                    write_u32_le(&mut ack_payload, 0, commands::REQ_BLOCK as u32);
                    write_u32_le(&mut ack_payload, 4, addr);
                    ack_payload[8..8 + block_size].copy_from_slice(&data[..block_size]);
                    bareboot_protocol::encode_frame(responses::ACK, &ack_payload[..8 + block_size], output)
                        .map_err(|_| CoreError::BufferTooSmall)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::COMPLETE => {
            writer.finish()?;
            writer.set_completed();
            let mut ack_payload = [0u8; 4];
            write_u32_le(&mut ack_payload, 0, commands::COMPLETE as u32);
            encode_ack(responses::ACK, &ack_payload, output)
        }
        commands::GET_CANBUS_ID => {
            let uid = writer.config().uid;
            let mut ack_payload = [0u8; 12];
            write_u32_le(&mut ack_payload, 0, commands::GET_CANBUS_ID as u32);
            ack_payload[4..10].copy_from_slice(&uid[..6]);
            encode_ack(responses::ACK, &ack_payload, output)
        }
        _ => encode_command_error(output),
    }
}

fn encode_ack(code: u8, payload: &[u8], output: &mut [u8]) -> Result<usize, CoreError> {
    bareboot_protocol::encode_frame(code, payload, output)
        .map_err(|e| match e {
            ProtocolError::BufferTooSmall => CoreError::BufferTooSmall,
            _ => CoreError::Flash,
        })
}

fn encode_command_error(output: &mut [u8]) -> Result<usize, CoreError> {
    bareboot_protocol::encode_frame(responses::COMMAND_ERROR, &[], output)
        .map_err(|_| CoreError::BufferTooSmall)
}
