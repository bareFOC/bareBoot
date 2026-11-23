use bareboot_client::BarebootConfig;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PinLevel {
    Low,
    High,
}

fn default_pin_level() -> PinLevel {
    PinLevel::Low
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafePinConfig {
    pub pin: String,
    #[serde(default = "default_pin_level")]
    pub level: PinLevel,
    #[serde(default)]
    pub comment: Option<String>,
}

fn default_boot_window() -> u32 {
    250
}

fn default_preserve_nvm() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeDefaults {
    #[serde(default = "default_boot_window")]
    pub boot_window_ms: u32,
    #[serde(default)]
    pub node_id: Option<u32>,
    #[serde(default)]
    pub i2c_address: Option<u8>,
    #[serde(default)]
    pub uart_baud: Option<u32>,
    #[serde(default = "default_preserve_nvm")]
    pub preserve_nvm: bool,
    #[serde(default)]
    pub initial_safe_pins: Vec<SafePinConfig>,
}

impl Default for RuntimeDefaults {
    fn default() -> Self {
        Self {
            boot_window_ms: default_boot_window(),
            node_id: None,
            i2c_address: None,
            uart_baud: None,
            preserve_nvm: default_preserve_nvm(),
            initial_safe_pins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetMemoryConfig {
    #[serde(default)]
    pub flash_start: Option<u32>,
    #[serde(default)]
    pub flash_size: Option<u32>,
    #[serde(default)]
    pub bootloader_size: Option<u32>,
    #[serde(default)]
    pub app_base: Option<u32>,
    #[serde(default)]
    pub app_end: Option<u32>,
    #[serde(default)]
    pub nvm_base: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

fn default_transport() -> String {
    "can".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetConfig {
    pub target: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub runtime_defaults: RuntimeDefaults,
    #[serde(default)]
    pub memory: TargetMemoryConfig,
}

impl TargetConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.target.trim().is_empty() {
            return Err("target must not be empty".to_string());
        }
        match self.transport.as_str() {
            "can" | "i2c" | "uart" | "usb" | "multi" => {}
            other => return Err(format!("unsupported transport: {other}")),
        }
        if self.runtime_defaults.boot_window_ms < 50 {
            return Err("boot_window_ms must be at least 50".to_string());
        }
        Ok(())
    }

    pub fn to_bareboot_config(&self) -> BarebootConfig {
        BarebootConfig {
            target: self.target.clone(),
            transport: self.transport.clone(),
            boot_window_ms: self.runtime_defaults.boot_window_ms,
        }
    }
}

fn parse_u32_val(val: &toml::Value) -> Option<u32> {
    match val {
        toml::Value::Integer(i) => Some(*i as u32),
        toml::Value::String(s) => {
            let trimmed = s.trim();
            if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                trimmed.parse::<u32>().ok()
            }
        }
        _ => None,
    }
}

fn parse_u8_val(val: &toml::Value) -> Option<u8> {
    parse_u32_val(val).map(|v| v as u8)
}

fn parse_safe_pins_val(val: &toml::Value) -> Vec<SafePinConfig> {
    let mut out = Vec::new();
    if let toml::Value::Array(arr) = val {
        for item in arr {
            if let Ok(cfg) = item.clone().try_into::<SafePinConfig>() {
                out.push(cfg);
            } else if let toml::Value::String(s) = item {
                let parts: Vec<&str> = s.split(':').collect();
                let pin = parts[0].trim().to_string();
                let level = if parts.len() > 1 && parts[1].trim().eq_ignore_ascii_case("high") {
                    PinLevel::High
                } else {
                    PinLevel::Low
                };
                out.push(SafePinConfig {
                    pin,
                    level,
                    comment: None,
                });
            }
        }
    }
    out
}

/// Dual parser: parses either a standalone bareboot.toml or a barefoc.toml
/// containing [bootloader], [mcu], and [memory] sections.
pub fn parse_target_config(text: &str) -> Result<TargetConfig, String> {
    let val: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let root = val.as_table().ok_or_else(|| "expected TOML table".to_string())?;

    if let Some(bl_val) = root.get("bootloader").and_then(|v| v.as_table()) {
        // barefoc.toml format with [bootloader], [mcu], [memory]
        let mcu_table = root.get("mcu").and_then(|v| v.as_table());
        let mem_table = root.get("memory").and_then(|v| v.as_table());

        let target = bl_val
            .get("target")
            .and_then(|v| v.as_str())
            .or_else(|| mcu_table.and_then(|m| m.get("target").and_then(|v| v.as_str())))
            .or_else(|| mcu_table.and_then(|m| m.get("family").and_then(|v| v.as_str())))
            .unwrap_or("generic")
            .to_string();

        let transport = bl_val
            .get("transport")
            .and_then(|v| v.as_str())
            .unwrap_or("can")
            .to_string();

        let boot_window_ms = bl_val
            .get("boot_window_ms")
            .and_then(parse_u32_val)
            .unwrap_or(250);

        let node_id = bl_val
            .get("node_id")
            .or_else(|| bl_val.get("can_node_id"))
            .and_then(parse_u32_val);

        let i2c_address = bl_val.get("i2c_address").and_then(parse_u8_val);
        let uart_baud = bl_val.get("uart_baud").and_then(parse_u32_val);
        let preserve_nvm = bl_val
            .get("preserve_nvm")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let initial_safe_pins = bl_val
            .get("initial_safe_pins")
            .map(parse_safe_pins_val)
            .unwrap_or_default();

        let memory = if let Some(mem) = mem_table {
            TargetMemoryConfig {
                flash_start: mem.get("flash_start").and_then(parse_u32_val),
                flash_size: mem.get("flash_size").and_then(parse_u32_val),
                bootloader_size: mem.get("bootloader_size").and_then(parse_u32_val),
                app_base: mem.get("app_base").and_then(parse_u32_val),
                app_end: mem.get("app_end").and_then(parse_u32_val),
                nvm_base: mem.get("nvm_base").and_then(parse_u32_val),
                page_size: mem.get("page_size").and_then(parse_u32_val),
            }
        } else {
            TargetMemoryConfig::default()
        };

        let cfg = TargetConfig {
            target,
            transport,
            runtime_defaults: RuntimeDefaults {
                boot_window_ms,
                node_id,
                i2c_address,
                uart_baud,
                preserve_nvm,
                initial_safe_pins,
            },
            memory,
        };
        cfg.validate()?;
        Ok(cfg)
    } else {
        // Standalone bareboot.toml format
        let target = root
            .get("target")
            .and_then(|v| v.as_str())
            .or_else(|| root.get("mcu").and_then(|v| v.as_str()))
            .unwrap_or("stm32f103")
            .to_string();

        let transport = root
            .get("transport")
            .and_then(|v| v.as_str())
            .unwrap_or("can")
            .to_string();

        let boot_window_ms = root
            .get("boot_window_ms")
            .and_then(parse_u32_val)
            .unwrap_or(250);

        let node_id = root
            .get("node_id")
            .or_else(|| root.get("can_node_id"))
            .and_then(parse_u32_val);

        let i2c_address = root.get("i2c_address").and_then(parse_u8_val);
        let uart_baud = root.get("uart_baud").and_then(parse_u32_val);
        let preserve_nvm = root
            .get("preserve_nvm")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let initial_safe_pins = root
            .get("initial_safe_pins")
            .map(parse_safe_pins_val)
            .unwrap_or_default();

        let memory = if let Some(mem) = root.get("memory").and_then(|v| v.as_table()) {
            TargetMemoryConfig {
                flash_start: mem.get("flash_start").and_then(parse_u32_val),
                flash_size: mem.get("flash_size").and_then(parse_u32_val),
                bootloader_size: mem.get("bootloader_size").and_then(parse_u32_val),
                app_base: mem.get("app_base").and_then(parse_u32_val),
                app_end: mem.get("app_end").and_then(parse_u32_val),
                nvm_base: mem.get("nvm_base").and_then(parse_u32_val),
                page_size: mem.get("page_size").and_then(parse_u32_val),
            }
        } else {
            TargetMemoryConfig {
                flash_start: root.get("flash_start").and_then(parse_u32_val),
                flash_size: root.get("flash_size").and_then(parse_u32_val),
                bootloader_size: root.get("bootloader_size").and_then(parse_u32_val),
                app_base: root.get("app_base").and_then(parse_u32_val),
                app_end: root.get("app_end").and_then(parse_u32_val),
                nvm_base: root.get("nvm_base").and_then(parse_u32_val),
                page_size: root.get("page_size").and_then(parse_u32_val),
            }
        };

        let cfg = TargetConfig {
            target,
            transport,
            runtime_defaults: RuntimeDefaults {
                boot_window_ms,
                node_id,
                i2c_address,
                uart_baud,
                preserve_nvm,
                initial_safe_pins,
            },
            memory,
        };
        cfg.validate()?;
        Ok(cfg)
    }
}

fn resolve_config_file(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if path.is_dir() {
        let foc = path.join("barefoc.toml");
        if foc.exists() {
            return Some(foc);
        }
        let boot = path.join("bareboot.toml");
        if boot.exists() {
            return Some(boot);
        }
    }
    None
}

/// Load TargetConfig supporting both bareboot.toml and barefoc.toml files or directories.
pub fn load_target_config(path: &Path) -> Result<TargetConfig, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(TargetConfig {
            target: "stm32f103".to_string(),
            transport: "can".to_string(),
            runtime_defaults: RuntimeDefaults::default(),
            memory: TargetMemoryConfig::default(),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let target_file = resolve_config_file(path).unwrap_or_else(|| path.to_path_buf());
        if !target_file.exists() {
            return Ok(TargetConfig {
                target: "stm32f103".to_string(),
                transport: "can".to_string(),
                runtime_defaults: RuntimeDefaults::default(),
                memory: TargetMemoryConfig::default(),
            });
        }
        let text = fs::read_to_string(&target_file).map_err(|e| e.to_string())?;
        parse_target_config(&text)
    }
}

pub fn save_target_config(path: &Path, config: &TargetConfig) -> Result<(), String> {
    config.validate()?;
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let target_file = if path.is_dir() {
            path.join("bareboot.toml")
        } else {
            path.to_path_buf()
        };
        let text = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
        if let Some(parent) = target_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        craft_atomic::atomic_write(&target_file, text.as_bytes()).map_err(|e| e.to_string())
    }
}

pub fn load_config(path: &Path) -> Result<BarebootConfig, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        return Ok(BarebootConfig {
            target: "stm32f103".to_string(),
            transport: "can".to_string(),
            boot_window_ms: 250,
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cfg = load_target_config(path)?;
        Ok(cfg.to_bareboot_config())
    }
}

pub fn save_config(path: &Path, config: &BarebootConfig) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let text = format!(
            "target = \"{}\"\ntransport = \"{}\"\nboot_window_ms = {}\n",
            config.target, config.transport, config.boot_window_ms
        );
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        craft_atomic::atomic_write(path, text.as_bytes()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn round_trip_bareboot_config() {
        let dir = env::temp_dir().join("bareboot-engine-cfg-test-roundtrip");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bareboot.toml");
        let cfg = BarebootConfig {
            target: "stm32g0".to_string(),
            transport: "can".to_string(),
            boot_window_ms: 250,
        };
        save_config(&path, &cfg).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(cfg, loaded);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fallback_default_when_missing() {
        let path = Path::new("/nonexistent/bareboot.toml");
        let cfg = load_config(path).unwrap();
        assert_eq!(cfg.boot_window_ms, 250);
        assert_eq!(cfg.target, "stm32f103");
    }

    #[test]
    fn parse_barefoc_toml_sections() {
        let toml_str = r#"
[mcu]
target = "stm32h503kbu6"
flash_size = 131072
ram_size = 32768

[memory]
flash_start = "0x08000000"
flash_size = "0x20000"
bootloader_size = "0x2000"
app_base = "0x08002000"
app_end = "0x0801E000"
nvm_base = "0x0801E000"
page_size = 2048

[bootloader]
target = "stm32h503"
transport = "i2c"
boot_window_ms = 300
node_id = "0x30"
i2c_address = "0x30"
uart_baud = 250000
preserve_nvm = true
initial_safe_pins = [
    { pin = "PB15", level = "low", comment = "DRV_SLEEP low" },
    { pin = "PB1", level = "low", comment = "INL_A low" },
    { pin = "PB0", level = "low", comment = "INL_B low" },
    { pin = "PA7", level = "low", comment = "INL_C low" },
]
"#;

        let cfg = parse_target_config(toml_str).unwrap();
        assert_eq!(cfg.target, "stm32h503");
        assert_eq!(cfg.transport, "i2c");
        assert_eq!(cfg.runtime_defaults.boot_window_ms, 300);
        assert_eq!(cfg.runtime_defaults.node_id, Some(0x30));
        assert_eq!(cfg.runtime_defaults.i2c_address, Some(0x30));
        assert_eq!(cfg.runtime_defaults.uart_baud, Some(250000));
        assert!(cfg.runtime_defaults.preserve_nvm);
        assert_eq!(cfg.runtime_defaults.initial_safe_pins.len(), 4);
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[0].pin, "PB15");
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[0].level, PinLevel::Low);

        assert_eq!(cfg.memory.app_base, Some(0x08002000));
        assert_eq!(cfg.memory.app_end, Some(0x0801E000));
        assert_eq!(cfg.memory.page_size, Some(2048));
    }

    #[test]
    fn parse_standalone_bareboot_toml() {
        let toml_str = r#"
target = "ch32v203"
transport = "usb"
boot_window_ms = 200
node_id = 0x100
preserve_nvm = true
app_base = 0x08002000
app_end = 0x08010000
initial_safe_pins = [
    "PB15:low",
    "PA7:high",
]
"#;
        let cfg = parse_target_config(toml_str).unwrap();
        assert_eq!(cfg.target, "ch32v203");
        assert_eq!(cfg.transport, "usb");
        assert_eq!(cfg.runtime_defaults.boot_window_ms, 200);
        assert_eq!(cfg.runtime_defaults.node_id, Some(0x100));
        assert!(cfg.runtime_defaults.preserve_nvm);
        assert_eq!(cfg.runtime_defaults.initial_safe_pins.len(), 2);
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[0].pin, "PB15");
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[0].level, PinLevel::Low);
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[1].pin, "PA7");
        assert_eq!(cfg.runtime_defaults.initial_safe_pins[1].level, PinLevel::High);
    }

    #[test]
    fn load_target_config_from_directory() {
        let dir = env::temp_dir().join("bareboot-engine-dir-cfg-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let foc_file = dir.join("barefoc.toml");
        let text = r#"
[mcu]
target = "stm32h503"

[bootloader]
target = "stm32h503"
transport = "multi"
boot_window_ms = 250
"#;
        fs::write(&foc_file, text).unwrap();

        let cfg = load_target_config(&dir).unwrap();
        assert_eq!(cfg.target, "stm32h503");
        assert_eq!(cfg.transport, "multi");

        let _ = fs::remove_dir_all(&dir);
    }
}
