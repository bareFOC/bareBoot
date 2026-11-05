use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlashProfile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    pub backend: String,
    #[serde(default)]
    pub firmware: String,
    #[serde(default)]
    pub serial_device: String,
    #[serde(default = "default_baud")]
    pub baud: u32,
    #[serde(default)]
    pub i2c_device: String,
    #[serde(default = "default_i2c_address")]
    pub i2c_address: String,
    #[serde(default)]
    pub can_interface: String,
    #[serde(default = "default_node_id")]
    pub node_id: String,
    #[serde(default = "default_tries")]
    pub tries: usize,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_verify")]
    pub verify: bool,

    // Katapult-style Bootloader Build Configuration
    #[serde(default = "default_mcu_family")]
    pub mcu_family: String,
    #[serde(default = "default_target_project")]
    pub target_project: String,
    #[serde(default = "default_can_enabled")]
    pub can_enabled: bool,
    #[serde(default = "default_can_freq")]
    pub can_frequency: u32,
    #[serde(default = "default_can_pins")]
    pub can_pins: String,
    #[serde(default = "default_uart_enabled")]
    pub uart_enabled: bool,
    #[serde(default = "default_uart_pins")]
    pub uart_pins: String,
    #[serde(default = "default_usb_enabled")]
    pub usb_enabled: bool,
    #[serde(default = "default_i2c_enabled")]
    pub i2c_enabled: bool,
    #[serde(default = "default_app_offset")]
    pub app_offset: String,
    #[serde(default = "default_led_pin")]
    pub led_pin: String,
    #[serde(default = "default_deploy_method")]
    pub deploy_method: String,
}

fn default_version() -> u32 { 1 }
fn default_baud() -> u32 { 115200 }
fn default_i2c_address() -> String { "0x40".to_string() }
fn default_node_id() -> String { "0x100".to_string() }
fn default_tries() -> usize { 5 }
fn default_timeout() -> u64 { 2 }
fn default_verify() -> bool { true }
fn default_mcu_family() -> String { "STM32F103".to_string() }
fn default_target_project() -> String { "bareboot".to_string() }
fn default_can_enabled() -> bool { true }
fn default_can_freq() -> u32 { 1000000 }
fn default_can_pins() -> String { "PB8/PB9".to_string() }
fn default_uart_enabled() -> bool { true }
fn default_uart_pins() -> String { "PA9/PA10".to_string() }
fn default_usb_enabled() -> bool { false }
fn default_i2c_enabled() -> bool { false }
fn default_app_offset() -> String { "8KiB (0x08002000)".to_string() }
fn default_led_pin() -> String { "PC13".to_string() }
fn default_deploy_method() -> String { "probe-rs".to_string() }

impl FlashProfile {
    pub fn presets() -> Vec<Self> {
        vec![
            Self {
                version: 1,
                name: "STM32F103-CAN".to_string(),
                backend: "can".to_string(),
                firmware: String::new(),
                serial_device: String::new(),
                baud: 115200,
                i2c_device: String::new(),
                i2c_address: "0x30".to_string(),
                can_interface: "can0".to_string(),
                node_id: "0x100".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "STM32F103".to_string(),
                target_project: "bareboot".to_string(),
                can_enabled: true,
                can_frequency: 1000000,
                can_pins: "PB8/PB9".to_string(),
                uart_enabled: false,
                uart_pins: "PA9/PA10".to_string(),
                usb_enabled: false,
                i2c_enabled: false,
                app_offset: "8KiB (0x08002000)".to_string(),
                led_pin: "PC13".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
            Self {
                version: 1,
                name: "STM32F103-UART".to_string(),
                backend: "serial".to_string(),
                firmware: String::new(),
                serial_device: "/dev/ttyUSB0".to_string(),
                baud: 115200,
                i2c_device: String::new(),
                i2c_address: "0x30".to_string(),
                can_interface: String::new(),
                node_id: "0x100".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "STM32F103".to_string(),
                target_project: "bareboot".to_string(),
                can_enabled: false,
                can_frequency: 1000000,
                can_pins: "PB8/PB9".to_string(),
                uart_enabled: true,
                uart_pins: "PA9/PA10".to_string(),
                usb_enabled: false,
                i2c_enabled: false,
                app_offset: "8KiB (0x08002000)".to_string(),
                led_pin: "PC13".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
            Self {
                version: 1,
                name: "STM32F103-USB".to_string(),
                backend: "serial".to_string(),
                firmware: String::new(),
                serial_device: "/dev/ttyACM0".to_string(),
                baud: 115200,
                i2c_device: String::new(),
                i2c_address: "0x30".to_string(),
                can_interface: String::new(),
                node_id: "0x100".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "STM32F103-USB".to_string(),
                target_project: "bareboot".to_string(),
                can_enabled: false,
                can_frequency: 1000000,
                can_pins: "None".to_string(),
                uart_enabled: false,
                uart_pins: "None".to_string(),
                usb_enabled: true,
                i2c_enabled: false,
                app_offset: "8KiB (0x08002000)".to_string(),
                led_pin: "PC13".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
            Self {
                version: 1,
                name: "STM32G0B1-CAN".to_string(),
                backend: "can".to_string(),
                firmware: String::new(),
                serial_device: String::new(),
                baud: 115200,
                i2c_device: String::new(),
                i2c_address: "0x30".to_string(),
                can_interface: "can0".to_string(),
                node_id: "0x100".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "STM32G0B1".to_string(),
                target_project: "bareboot".to_string(),
                can_enabled: true,
                can_frequency: 1000000,
                can_pins: "PB0/PB1".to_string(),
                uart_enabled: false,
                uart_pins: "None".to_string(),
                usb_enabled: false,
                i2c_enabled: false,
                app_offset: "8KiB (0x08002000)".to_string(),
                led_pin: "PA5".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
            Self {
                version: 1,
                name: "STM32H503-I2C".to_string(),
                backend: "i2c".to_string(),
                firmware: String::new(),
                serial_device: String::new(),
                baud: 115200,
                i2c_device: "/dev/i2c-1".to_string(),
                i2c_address: "0x30".to_string(),
                can_interface: String::new(),
                node_id: "0x30".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "STM32H503".to_string(),
                target_project: "bareboot".to_string(),
                can_enabled: false,
                can_frequency: 1000000,
                can_pins: "None".to_string(),
                uart_enabled: false,
                uart_pins: "None".to_string(),
                usb_enabled: false,
                i2c_enabled: true,
                app_offset: "16KiB (0x08004000)".to_string(),
                led_pin: "PB12".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
            Self {
                version: 1,
                name: "CH32V203-USB".to_string(),
                backend: "memory".to_string(),
                firmware: String::new(),
                serial_device: String::new(),
                baud: 115200,
                i2c_device: String::new(),
                i2c_address: "0x30".to_string(),
                can_interface: String::new(),
                node_id: "0x100".to_string(),
                tries: 5,
                timeout_secs: 2,
                verify: true,
                mcu_family: "CH32V203-USB".to_string(),
                target_project: "bareboot-ch32v203-usb".to_string(),
                can_enabled: false,
                can_frequency: 1000000,
                can_pins: "None".to_string(),
                uart_enabled: false,
                uart_pins: "None".to_string(),
                usb_enabled: true,
                i2c_enabled: false,
                app_offset: "8KiB (0x08002000)".to_string(),
                led_pin: "PB12".to_string(),
                deploy_method: "probe-rs".to_string(),
            },
        ]
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() { return Err("name required".to_string()); }
        match self.backend.as_str() {
            "memory" | "serial" | "i2c" | "can" => {}
            other => return Err(format!("unknown backend: {other}")),
        }
        Ok(())
    }

    pub fn save(&self, dir: &Path) -> Result<PathBuf, String> {
        self.validate()?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.toml", self.name));
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let p: Self = toml::from_str(&text).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    pub fn list(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        out.push(stem.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
        let path = dir.join(format!("{name}.toml"));
        if path.exists() { fs::remove_file(path).map_err(|e| e.to_string())?; }
        Ok(())
    }
}

impl Default for FlashProfile {
    fn default() -> Self {
        Self {
            version: default_version(),
            name: "default".to_string(),
            backend: "memory".to_string(),
            firmware: String::new(),
            serial_device: String::new(),
            baud: default_baud(),
            i2c_device: String::new(),
            i2c_address: default_i2c_address(),
            can_interface: String::new(),
            node_id: default_node_id(),
            tries: default_tries(),
            timeout_secs: default_timeout(),
            verify: default_verify(),
            mcu_family: default_mcu_family(),
            target_project: default_target_project(),
            can_enabled: default_can_enabled(),
            can_frequency: default_can_freq(),
            can_pins: default_can_pins(),
            uart_enabled: default_uart_enabled(),
            uart_pins: default_uart_pins(),
            usb_enabled: default_usb_enabled(),
            i2c_enabled: default_i2c_enabled(),
            app_offset: default_app_offset(),
            led_pin: default_led_pin(),
            deploy_method: default_deploy_method(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn round_trip() {
        let dir = env::temp_dir().join("bareboot-profile-test");
        let _ = fs::remove_dir_all(&dir);
        let p = FlashProfile {
            name: "demo".to_string(),
            backend: "memory".to_string(),
            firmware: "fw.bin".to_string(),
            tries: 3,
            timeout_secs: 4,
            verify: false,
            ..Default::default()
        };
        p.save(&dir).unwrap();
        let loaded = FlashProfile::load(&dir.join("demo.toml")).unwrap();
        assert_eq!(p, loaded);
        assert_eq!(FlashProfile::list(&dir), vec!["demo".to_string()]);
        FlashProfile::delete(&dir, "demo").unwrap();
        assert!(FlashProfile::list(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validates_backend() {
        let mut p = FlashProfile {
            name: "x".into(),
            backend: "zigbee".into(),
            ..Default::default()
        };
        assert!(p.validate().is_err());
        p.backend = "serial".into();
        assert!(p.validate().is_ok());
    }
}