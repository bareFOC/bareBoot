use std::time::Duration;

use crate::error::HostError;

pub trait HostBus {
    fn exchange(&mut self, frame: &[u8], timeout: Duration) -> Result<Vec<u8>, HostError>;
}

pub struct InMemoryBus {
    pub responses: std::collections::VecDeque<Vec<u8>>,
    pub sent: Vec<Vec<u8>>,
    pub auto_respond: bool,
    pub last_block: Vec<u8>,
}

impl InMemoryBus {
    pub fn new() -> Self {
        Self {
            responses: std::collections::VecDeque::new(),
            sent: Vec::new(),
            auto_respond: false,
            last_block: vec![0u8; 64],
        }
    }

    pub fn new_simulated() -> Self {
        Self {
            responses: std::collections::VecDeque::new(),
            sent: Vec::new(),
            auto_respond: true,
            last_block: vec![0u8; 64],
        }
    }

    pub fn push_response(&mut self, response: Vec<u8>) {
        self.responses.push_back(response);
    }
}

impl Default for InMemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

impl HostBus for InMemoryBus {
    fn exchange(&mut self, frame: &[u8], _timeout: Duration) -> Result<Vec<u8>, HostError> {
        self.sent.push(frame.to_vec());
        if let Some(resp) = self.responses.pop_front() {
            return Ok(resp);
        }

        if self.auto_respond {
            if let bareboot_protocol::FrameParse::Complete { command, payload_offset, .. } =
                bareboot_protocol::scan(frame)
            {
                match command {
                    bareboot_protocol::commands::CONNECT => {
                        let mut payload = vec![0u8; 20];
                        bareboot_protocol::write_u32_le(&mut payload, 0, bareboot_protocol::commands::CONNECT as u32);
                        bareboot_protocol::write_u32_le(&mut payload, 4, 0x00010100);
                        bareboot_protocol::write_u32_le(&mut payload, 8, 0x0800_2000);
                        bareboot_protocol::write_u32_le(&mut payload, 12, 64);
                        payload[16..20].copy_from_slice(b"sim0");
                        payload.extend_from_slice(b"\0quiet\0\0");
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                    bareboot_protocol::commands::RX_BLOCK => {
                        let addr = if frame.len() >= payload_offset + 4 {
                            bareboot_protocol::read_u32_le(frame, payload_offset)
                        } else {
                            0x0800_2000
                        };
                        if frame.len() >= payload_offset + 4 + 64 {
                            self.last_block = frame[payload_offset + 4..payload_offset + 4 + 64].to_vec();
                        }
                        let mut payload = vec![0u8; 8];
                        payload[0..4].copy_from_slice(&(bareboot_protocol::commands::RX_BLOCK as u32).to_le_bytes());
                        payload[4..8].copy_from_slice(&addr.to_le_bytes());
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                    bareboot_protocol::commands::REQ_BLOCK => {
                        let addr = if frame.len() >= payload_offset + 4 {
                            bareboot_protocol::read_u32_le(frame, payload_offset)
                        } else {
                            0x0800_2000
                        };
                        let mut payload = vec![0u8; 8 + self.last_block.len()];
                        payload[0..4].copy_from_slice(&(bareboot_protocol::commands::REQ_BLOCK as u32).to_le_bytes());
                        payload[4..8].copy_from_slice(&addr.to_le_bytes());
                        payload[8..].copy_from_slice(&self.last_block);
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                    bareboot_protocol::commands::RX_EOF => {
                        let mut payload = vec![0u8; 8];
                        payload[0..4].copy_from_slice(&(bareboot_protocol::commands::RX_EOF as u32).to_le_bytes());
                        payload[4..8].copy_from_slice(&1u32.to_le_bytes());
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                    bareboot_protocol::commands::COMPLETE => {
                        let mut payload = vec![0u8; 4];
                        payload[0..4].copy_from_slice(&(bareboot_protocol::commands::COMPLETE as u32).to_le_bytes());
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                    _ => {
                        let payload = [0u8; 4];
                        let mut resp = vec![0u8; 4 + payload.len() + 4];
                        let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                        return Ok(resp);
                    }
                }
            } else {
                let payload = [0u8; 4];
                let mut resp = vec![0u8; 4 + payload.len() + 4];
                let _ = bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut resp);
                return Ok(resp);
            }
        }

        Err(HostError("no queued response".to_string()))
    }
}

impl HostBus for Box<dyn HostBus> {
    fn exchange(&mut self, frame: &[u8], timeout: std::time::Duration) -> Result<Vec<u8>, crate::error::HostError> {
        (**self).exchange(frame, timeout)
    }
}
