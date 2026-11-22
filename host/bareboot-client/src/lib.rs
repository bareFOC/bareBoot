#![allow(unsafe_code)]

pub mod error;
pub mod flash_runner;
pub mod framing;
pub mod session;
pub mod transport;
pub mod android_usb;
pub mod web_serial;

#[cfg(target_os = "linux")]
pub mod serial;

#[cfg(target_os = "linux")]
pub mod i2c;

#[cfg(target_os = "linux")]
pub mod can;

pub use error::HostError;
pub use flash_runner::run_flash;
pub use session::{ConnectInfo, FlashSession};
pub use transport::{HostBus, InMemoryBus};
pub use android_usb::AndroidUsbBus;
pub use web_serial::WebSerialBus;
#[cfg(target_os = "linux")]
pub use can::SocketCanBus;
#[cfg(target_os = "linux")]
pub use i2c::I2cBus;
#[cfg(target_os = "linux")]
pub use serial::SerialBus;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BarebootConfig {
    pub target: String,
    pub transport: String,
    #[serde(default = "default_boot_window")]
    pub boot_window_ms: u32,
}

fn default_boot_window() -> u32 {
    300
}

impl BarebootConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.target.trim().is_empty() {
            return Err("target must not be empty".to_string());
        }
        match self.transport.as_str() {
            "can" | "i2c" | "uart" | "usb" => {}
            other => return Err(format!("unsupported transport: {other}")),
        }
        if self.boot_window_ms < 50 {
            return Err("boot_window_ms must be at least 50".to_string());
        }
        Ok(())
    }
}

pub fn parse_config(input: &str) -> Result<BarebootConfig, toml::de::Error> {
    toml::from_str(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates_config() {
        let input = r#"
target = "ch32v203"
transport = "can"
boot_window_ms = 250
"#;
        let cfg = parse_config(input).unwrap();
        assert_eq!(cfg.target, "ch32v203");
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn rejects_unknown_transport() {
        let input = r#"
target = "stm32g0"
transport = "zigbee"
"#;
        let cfg = parse_config(input).unwrap();
        assert!(cfg.validate().is_err());
    }
}
