use std::time::Duration;

use bareboot_protocol::commands;
use crate::error::HostError;
use crate::framing;
use crate::transport::HostBus;

pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct ConnectInfo {
    pub protocol_version: u32,
    pub app_base: u32,
    pub block_size: usize,
    pub mcu_type: String,
    pub software_version: String,
}

pub struct FlashSession<B> {
    bus: B,
    connect_info: Option<ConnectInfo>,
}

impl<B: HostBus> FlashSession<B> {
    pub fn new(bus: B) -> Self {
        Self {
            bus,
            connect_info: None,
        }
    }

    pub fn connect_info(&self) -> Option<&ConnectInfo> {
        self.connect_info.as_ref()
    }

    pub fn connect(&mut self, tries: usize) -> Result<ConnectInfo, HostError> {
        let frame = framing::build_frame(commands::CONNECT, &[])?;
        let mut last_error = HostError("connect failed".to_string());
        for _ in 0..tries {
            match self.bus.exchange(&frame, DEFAULT_CONNECT_TIMEOUT) {
                Ok(response) => {
                    let payload = framing::parse_frame(&response, commands::CONNECT)?;
                    if payload.len() < 16 {
                        last_error = HostError("short CONNECT payload".to_string());
                        continue;
                    }
                    let protocol_version = u32::from_le_bytes(payload[4..8].try_into().unwrap());
                    let app_base = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                    let block_size = u32::from_le_bytes(payload[12..16].try_into().unwrap()) as usize;
                    if ![64, 128, 256, 512].contains(&block_size) {
                        last_error = HostError(format!("invalid block size: {block_size}"));
                        continue;
                    }
                    let (mcu_type, software_version) = parse_connect_strings(&payload);
                    let info = ConnectInfo {
                        protocol_version,
                        app_base,
                        block_size,
                        mcu_type,
                        software_version,
                    };
                    self.connect_info = Some(info.clone());
                    return Ok(info);
                }
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    pub fn write_block(&mut self, addr: u32, data: &[u8]) -> Result<u32, HostError> {
        let info = self
            .connect_info
            .as_ref()
            .ok_or_else(|| HostError("not connected".to_string()))?;
        if addr < info.app_base {
            return Err(HostError("block address below app base".to_string()));
        }
        if data.len() != info.block_size {
            return Err(HostError(format!(
                "expected {} bytes, got {}",
                info.block_size,
                data.len()
            )));
        }

        let mut payload = Vec::with_capacity(4 + data.len());
        payload.extend_from_slice(&addr.to_le_bytes());
        payload.extend_from_slice(data);
        let frame = framing::build_frame(commands::RX_BLOCK, &payload)?;
        let response = self.bus.exchange(&frame, Duration::from_secs(2))?;
        let ack_payload = framing::parse_frame(&response, commands::RX_BLOCK)?;
        if ack_payload.len() < 8 {
            return Err(HostError("short RX_BLOCK ACK".to_string()));
        }
        let returned_addr = u32::from_le_bytes(ack_payload[4..8].try_into().unwrap());
        if returned_addr != addr {
            return Err(HostError("RX_BLOCK address mismatch".to_string()));
        }
        Ok(returned_addr)
    }

    pub fn request_block(&mut self, addr: u32) -> Result<Vec<u8>, HostError> {
        let _info = self
            .connect_info
            .as_ref()
            .ok_or_else(|| HostError("not connected".to_string()))?;
        let payload = addr.to_le_bytes();
        let frame = framing::build_frame(commands::REQ_BLOCK, &payload)?;
        let response = self.bus.exchange(&frame, Duration::from_secs(2))?;
        let ack_payload = framing::parse_frame(&response, commands::REQ_BLOCK)?;
        if ack_payload.len() < 8 {
            return Err(HostError("short REQ_BLOCK ACK".to_string()));
        }
        let returned_addr = u32::from_le_bytes(ack_payload[4..8].try_into().unwrap());
        if returned_addr != addr {
            return Err(HostError("REQ_BLOCK address mismatch".to_string()));
        }
        Ok(ack_payload[8..].to_vec())
    }

    pub fn finish(&mut self) -> Result<u32, HostError> {
        let frame = framing::build_frame(commands::RX_EOF, &[])?;
        let response = self.bus.exchange(&frame, Duration::from_secs(2))?;
        let ack_payload = framing::parse_frame(&response, commands::RX_EOF)?;
        if ack_payload.len() < 8 {
            return Err(HostError("short EOF ACK".to_string()));
        }
        Ok(u32::from_le_bytes(ack_payload[4..8].try_into().unwrap()))
    }

    pub fn complete(&mut self) -> Result<(), HostError> {
        let frame = framing::build_frame(commands::COMPLETE, &[])?;
        let response = self.bus.exchange(&frame, Duration::from_secs(2))?;
        framing::parse_frame(&response, commands::COMPLETE)?;
        Ok(())
    }

    pub fn verify_block(&mut self, addr: u32, data: &[u8]) -> Result<(), HostError> {
        let read = self.request_block(addr)?;
        if read != data {
            return Err(HostError("block verification mismatch".to_string()));
        }
        Ok(())
    }
}

fn parse_connect_strings(payload: &[u8]) -> (String, String) {
    let s = &payload[16..];
    let mcu_len = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    let mcu_words = (mcu_len + 3) / 4;
    let version_start = mcu_words * 4;
    let v = if version_start < s.len() { &s[version_start..] } else { &[] };
    let version_len = v.iter().position(|&b| b == 0).unwrap_or(v.len());
    (
        String::from_utf8_lossy(&s[..mcu_len]).to_string(),
        String::from_utf8_lossy(&v[..version_len]).to_string(),
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::InMemoryBus;

    fn connect_response() -> Vec<u8> {
        let mut payload = vec![0u8; 20];
        bareboot_protocol::write_u32_le(&mut payload, 0, commands::CONNECT as u32);
        bareboot_protocol::write_u32_le(&mut payload, 4, 0x00010100);
        bareboot_protocol::write_u32_le(&mut payload, 8, 0x0800_1000);
        bareboot_protocol::write_u32_le(&mut payload, 12, 64);
        payload[16..20].copy_from_slice(b"ch32");
        payload.extend_from_slice(b"\0quiet\0\0");
        let mut frame = vec![0u8; 4 + payload.len() + 4];
        bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut frame).unwrap();
        frame
    }

    #[test]
    fn connects_and_writes_block() {
        let bus = InMemoryBus::new();
        let mut session = FlashSession::new(bus);
        session.bus.push_response(connect_response());

        let info = session.connect(1).unwrap();
        assert_eq!(info.block_size, 64);
        assert_eq!(info.app_base, 0x0800_1000);

        let data = vec![0xABu8; 64];
        // ACK for RX_BLOCK
        let mut payload = vec![0u8; 8];
        payload[0..4].copy_from_slice(&(commands::RX_BLOCK as u32).to_le_bytes());
        payload[4..8].copy_from_slice(&0x0800_1000u32.to_le_bytes());
        let mut frame = vec![0u8; 4 + payload.len() + 4];
        bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut frame).unwrap();
        session.bus.push_response(frame);
        session.write_block(0x0800_1000, &data).unwrap();
        assert_eq!(session.bus.sent.len(), 2);

        // EOF response
        let mut payload = vec![0u8; 8];
        payload[0..4].copy_from_slice(&(commands::RX_EOF as u32).to_le_bytes());
        payload[4..8].copy_from_slice(&1u32.to_le_bytes());
        let mut frame = vec![0u8; 4 + payload.len() + 4];
        bareboot_protocol::encode_frame(bareboot_protocol::responses::ACK, &payload, &mut frame).unwrap();
        session.bus.push_response(frame);
        let pages = session.finish().unwrap();
        assert_eq!(pages, 1);
    }

    #[test]
    fn rejects_bad_block_address() {
        let bus = InMemoryBus::new();
        let mut session = FlashSession::new(bus);
        session.bus.push_response(connect_response());
        session.connect(1).unwrap();
        assert!(session.write_block(0x0700_0000, &vec![0; 64]).is_err());
    }
}
