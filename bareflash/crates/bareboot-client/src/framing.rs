use bareboot_protocol::{self, FrameParse};
use crate::error::HostError;

pub fn build_frame(command: u8, payload: &[u8]) -> Result<Vec<u8>, HostError> {
    let mut buf = vec![0u8; 4 + payload.len() + 4];
    let len = bareboot_protocol::encode_frame(command, payload, &mut buf)
        .map_err(|e| HostError(format!("encode error: {e:?}")))?;
    buf.truncate(len);
    Ok(buf)
}

pub fn parse_frame(data: &[u8], expected_command: u8) -> Result<Vec<u8>, HostError> {
    match bareboot_protocol::scan(data) {
        FrameParse::Complete { command, word_count, payload_offset, .. } => {
            if command != bareboot_protocol::responses::ACK {
                return Err(HostError(format!("unexpected response command: {command:#x}")));
            }
            let payload_len = word_count * 4;
            if payload_len == 0 || payload_offset + payload_len > data.len() {
                return Err(HostError("malformed ACK payload".to_string()));
            }
            let payload = &data[payload_offset..payload_offset + payload_len];
            if bareboot_protocol::read_u32_le(payload, 0) as u8 != expected_command {
                return Err(HostError(format!(
                    "ACK command mismatch: expected {expected_command:#x}"
                )));
            }
            Ok(payload.to_vec())
        }
        FrameParse::NeedMore => Err(HostError("incomplete response frame".to_string())),
        FrameParse::Invalid { .. } => Err(HostError("invalid response frame".to_string())),
    }
}

pub fn u32_le_bytes(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}
