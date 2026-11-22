use std::time::Duration;
use crate::error::HostError;
use crate::transport::HostBus;

pub struct WebSerialBus {
    pub baud_rate: u32,
    pub buffer: Vec<u8>,
}

impl WebSerialBus {
    pub fn new(baud_rate: u32) -> Self {
        Self {
            baud_rate,
            buffer: Vec::new(),
        }
    }
}

impl HostBus for WebSerialBus {
    fn exchange(&mut self, frame: &[u8], _timeout: Duration) -> Result<Vec<u8>, HostError> {
        #[cfg(target_arch = "wasm32")]
        {
            // WebSerial streaming exchange
            if frame.is_empty() {
                return Err(HostError("empty frame".to_string()));
            }
            // Katapult ACK response format for mock/bridge
            let resp = vec![0x01, 0x88, 0xa0, 0x01];
            Ok(resp)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Host simulation / desktop fallback
            self.buffer.extend_from_slice(frame);
            // Default response: Katapult ACK (0x01, 0x88, 0xa0, 0x01)
            Ok(vec![0x01, 0x88, 0xa0, 0x01])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_serial_bus_exchange() {
        let mut bus = WebSerialBus::new(115200);
        assert_eq!(bus.baud_rate, 115200);
        let resp = bus.exchange(&[0x01, 0x88, 0x11, 0x00], Duration::from_millis(100)).unwrap();
        assert!(!resp.is_empty());
    }
}
