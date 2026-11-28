use bareboot_client::BarebootConfig;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

fn default_boot_window() -> u32 {
    250
}

fn default_preserve_nvm() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_bootloader_type() -> String {
    "bareboot".to_string()
}

fn default_system_clock() -> u32 {
    64_000_000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeDefaults {
    #[serde(default = "default_boot_window")]
    pub boot_window_ms: u32,
    #[serde(default)]
    pub address: Option<u8>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default = "default_preserve_nvm")]
    pub preserve_nvm: bool,
    /// Klipper-style safe startup pin specs (e.g. `"!PA9"`, `"PB1"`, `"^PB12"`).
    #[serde(default)]
    pub initial_safe_pins: Vec<String>,
    #[serde(default)]
    pub mailbox_address: Option<u32>,
    #[serde(default)]
    pub mailbox_magic: Option<u64>,
}

impl Default for RuntimeDefaults {
    fn default() -> Self {
        Self {
            boot_window_ms: default_boot_window(),
            address: None,
            bitrate: None,
            preserve_nvm: default_preserve_nvm(),
            initial_safe_pins: Vec::new(),
            mailbox_address: None,
            mailbox_magic: None,
        }
    }
}

/// Unified `[memory]` section matching `barefoc.toml` and `bareboot.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetMemoryConfig {
    #[serde(default)]
    pub flash_origin: Option<u32>,
    #[serde(default)]
    pub flash_size: Option<u32>,
    #[serde(default)]
    pub ram_origin: Option<u32>,
    #[serde(default)]
    pub ram_size: Option<u32>,
    #[serde(default)]
    pub bootloader_origin: Option<u32>,
    #[serde(default)]
    pub bootloader_size: Option<u32>,
    #[serde(default)]
    pub app_origin: Option<u32>,
    #[serde(default)]
    pub app_size: Option<u32>,
    #[serde(default)]
    pub nvm_origin: Option<u32>,
    #[serde(default)]
    pub nvm_size: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

/// Unified `[board]` section matching `barefoc.toml` and `bareboot.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetBoardConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mcu: String,
    #[serde(default)]
    pub family: String,
    #[serde(default = "default_system_clock")]
    pub system_clock_hz: u32,
}

impl Default for TargetBoardConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            mcu: "stm32f103".to_string(),
            family: "stm32f1".to_string(),
            system_clock_hz: default_system_clock(),
        }
    }
}

/// Unified `[communication]` section matching `barefoc.toml` and `bareboot.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetCommunicationConfig {
    #[serde(default)]
    pub transports: Vec<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub tx_scl_pin: Option<String>,
    #[serde(default)]
    pub rx_sda_pin: Option<String>,
    #[serde(default)]
    pub can_enable_pin: Option<String>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub address: Option<u8>,
}

/// Unified `[bootloader]` section matching `barefoc.toml` and `bareboot.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetBootloaderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(rename = "type", default = "default_bootloader_type")]
    pub bootloader_type: String,
    #[serde(default = "default_boot_window")]
    pub boot_window_ms: u32,
    #[serde(default = "default_preserve_nvm")]
    pub preserve_nvm: bool,
    #[serde(default)]
    pub initial_safe_pins: Vec<String>,
    #[serde(default)]
    pub status_led: Option<String>,
    #[serde(default)]
    pub mailbox_address: Option<u32>,
    #[serde(default)]
    pub mailbox_magic: Option<u64>,
}

impl Default for TargetBootloaderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bootloader_type: default_bootloader_type(),
            boot_window_ms: default_boot_window(),
            preserve_nvm: default_preserve_nvm(),
            initial_safe_pins: Vec::new(),
            status_led: None,
            mailbox_address: None,
            mailbox_magic: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetConfig {
    pub target: String,
    pub transport: String,
    #[serde(default)]
    pub board: TargetBoardConfig,
    #[serde(default)]
    pub communication: TargetCommunicationConfig,
    #[serde(default)]
    pub bootloader: TargetBootloaderConfig,
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
            "can" | "i2c" | "i3c" | "uart" | "usb" | "multi" => {}
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

#[derive(Debug, Clone, Serialize)]
struct UnifiedTomlFile<'a> {
    board: &'a TargetBoardConfig,
    memory: &'a TargetMemoryConfig,
    communication: &'a TargetCommunicationConfig,
    bootloader: &'a TargetBootloaderConfig,
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

fn parse_u64_val(val: &toml::Value) -> Option<u64> {
    match val {
        toml::Value::Integer(i) => Some(*i as u64),
        toml::Value::String(s) => {
            let trimmed = s.trim();
            if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16).ok()
            } else {
                trimmed.parse::<u64>().ok()
            }
        }
        _ => None,
    }
}

fn parse_u8_val(val: &toml::Value) -> Option<u8> {
    parse_u32_val(val).map(|v| v as u8)
}

fn parse_string_list(val: &toml::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let toml::Value::Array(arr) = val {
        for item in arr {
            if let toml::Value::String(s) = item {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out
}

fn strip_klipper_modifiers(spec: &str) -> (bool, bool, bool, &str) {
    let mut rest = spec.trim();
    let mut inverted = false;
    let mut pull_up = false;
    let mut pull_down = false;
    while let Some(b) = rest.as_bytes().first().copied() {
        match b {
            b'!' => inverted = true,
            b'^' => pull_up = true,
            b'~' => pull_down = true,
            _ => break,
        }
        rest = &rest[1..];
    }
    (inverted, pull_up, pull_down, rest.trim())
}

fn push_unique_pin(out: &mut Vec<String>, seen: &mut Vec<String>, spec: String, base_pin: &str) {
    let key = base_pin.to_ascii_uppercase();
    if !key.is_empty() && !seen.contains(&key) {
        seen.push(key);
        out.push(spec);
    }
}

/// Auto-derive Klipper-style safe startup pins from `[gate_driver]`.
fn derive_safe_pins_from_gate_driver(gd: &toml::map::Map<String, toml::Value>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = Vec::new();

    for key in [
        "sleep_pin",
        "enable_pin",
        "en_pin",
        "inl_a",
        "inl_b",
        "inl_c",
        "inh_a",
        "inh_b",
        "inh_c",
    ] {
        if let Some(raw) = gd.get(key).and_then(|v| v.as_str()) {
            let (inverted, _pu, _pd, base) = strip_klipper_modifiers(raw);
            if base.is_empty() {
                continue;
            }
            let safe_spec = if inverted {
                base.to_string()
            } else {
                format!("!{base}")
            };
            push_unique_pin(&mut out, &mut seen, safe_spec, base);
        }
    }

    if let Some(raw) = gd.get("fault_pin").and_then(|v| v.as_str()) {
        let (_inv, pull_up, pull_down, base) = strip_klipper_modifiers(raw);
        if !base.is_empty() {
            if pull_up {
                push_unique_pin(&mut out, &mut seen, format!("^{base}"), base);
            } else if pull_down {
                push_unique_pin(&mut out, &mut seen, format!("~{base}"), base);
            }
        }
    }

    out
}

/// Unified parser for both `bareboot.toml` and `barefoc.toml` using `[board]`, `[memory]`,
/// `[communication]`, `[bootloader]`, and optional `[gate_driver]`.
pub fn parse_target_config(text: &str) -> Result<TargetConfig, String> {
    let val: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let root = val.as_table().ok_or_else(|| "expected TOML table".to_string())?;

    let board_table = root
        .get("board")
        .and_then(|v| v.as_table())
        .ok_or_else(|| "missing [board] section".to_string())?;
    let comm_table = root.get("communication").and_then(|v| v.as_table());
    let bl_table = root.get("bootloader").and_then(|v| v.as_table());
    let mem_table = root.get("memory").and_then(|v| v.as_table());
    let gd_table = root.get("gate_driver").and_then(|v| v.as_table());

    let board = TargetBoardConfig {
        name: board_table
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        mcu: board_table
            .get("mcu")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        family: board_table
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        system_clock_hz: board_table
            .get("system_clock_hz")
            .and_then(parse_u32_val)
            .unwrap_or_else(default_system_clock),
    };

    let target = if !board.mcu.is_empty() {
        board.mcu.clone()
    } else if !board.family.is_empty() {
        board.family.clone()
    } else {
        board.name.clone()
    };

    let transports = comm_table
        .and_then(|c| c.get("transports"))
        .map(parse_string_list)
        .unwrap_or_default();
    let single_transport = comm_table
        .and_then(|c| c.get("transport"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let transport = single_transport
        .clone()
        .or_else(|| transports.first().cloned())
        .unwrap_or_else(|| "can".to_string());

    let communication = TargetCommunicationConfig {
        transports: if transports.is_empty() {
            vec![transport.clone()]
        } else {
            transports
        },
        transport: single_transport,
        tx_scl_pin: comm_table
            .and_then(|c| c.get("tx_scl_pin"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        rx_sda_pin: comm_table
            .and_then(|c| c.get("rx_sda_pin"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        can_enable_pin: comm_table
            .and_then(|c| c.get("can_enable_pin"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        bitrate: comm_table.and_then(|c| c.get("bitrate")).and_then(parse_u32_val),
        address: comm_table.and_then(|c| c.get("address")).and_then(parse_u8_val),
    };

    let boot_window_ms = bl_table
        .and_then(|b| b.get("boot_window_ms"))
        .and_then(parse_u32_val)
        .unwrap_or_else(default_boot_window);
    let preserve_nvm = bl_table
        .and_then(|b| b.get("preserve_nvm"))
        .and_then(|v| v.as_bool())
        .unwrap_or_else(default_preserve_nvm);
    let initial_safe_pins = if let Some(explicit) = bl_table.and_then(|b| b.get("initial_safe_pins")) {
        parse_string_list(explicit)
    } else if let Some(gd) = gd_table {
        derive_safe_pins_from_gate_driver(gd)
    } else {
        Vec::new()
    };

    let bootloader = TargetBootloaderConfig {
        enabled: bl_table
            .and_then(|b| b.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        bootloader_type: bl_table
            .and_then(|b| b.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bareboot")
            .to_string(),
        boot_window_ms,
        preserve_nvm,
        initial_safe_pins: initial_safe_pins.clone(),
        status_led: bl_table
            .and_then(|b| b.get("status_led"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        mailbox_address: bl_table
            .and_then(|b| b.get("mailbox_address"))
            .and_then(parse_u32_val),
        mailbox_magic: bl_table
            .and_then(|b| b.get("mailbox_magic"))
            .and_then(parse_u64_val),
    };

    let memory = if let Some(mem) = mem_table {
        TargetMemoryConfig {
            flash_origin: mem.get("flash_origin").and_then(parse_u32_val),
            flash_size: mem.get("flash_size").and_then(parse_u32_val),
            ram_origin: mem.get("ram_origin").and_then(parse_u32_val),
            ram_size: mem.get("ram_size").and_then(parse_u32_val),
            bootloader_origin: mem.get("bootloader_origin").and_then(parse_u32_val),
            bootloader_size: mem.get("bootloader_size").and_then(parse_u32_val),
            app_origin: mem.get("app_origin").and_then(parse_u32_val),
            app_size: mem.get("app_size").and_then(parse_u32_val),
            nvm_origin: mem.get("nvm_origin").and_then(parse_u32_val),
            nvm_size: mem.get("nvm_size").and_then(parse_u32_val),
            page_size: mem.get("page_size").and_then(parse_u32_val),
        }
    } else {
        TargetMemoryConfig::default()
    };

    let cfg = TargetConfig {
        target,
        transport,
        board,
        communication: communication.clone(),
        bootloader: bootloader.clone(),
        runtime_defaults: RuntimeDefaults {
            boot_window_ms,
            address: communication.address,
            bitrate: communication.bitrate,
            preserve_nvm,
            initial_safe_pins,
            mailbox_address: bootloader.mailbox_address,
            mailbox_magic: bootloader.mailbox_magic,
        },
        memory,
    };
    cfg.validate()?;
    Ok(cfg)
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

/// Load TargetConfig supporting both `bareboot.toml` and `barefoc.toml` files or directories.
pub fn load_target_config(path: &Path) -> Result<TargetConfig, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(TargetConfig {
            target: "stm32f103".to_string(),
            transport: "can".to_string(),
            board: TargetBoardConfig::default(),
            communication: TargetCommunicationConfig::default(),
            bootloader: TargetBootloaderConfig::default(),
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
                board: TargetBoardConfig::default(),
                communication: TargetCommunicationConfig::default(),
                bootloader: TargetBootloaderConfig::default(),
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
        let unified = UnifiedTomlFile {
            board: &config.board,
            memory: &config.memory,
            communication: &config.communication,
            bootloader: &config.bootloader,
        };
        let text = toml::to_string_pretty(&unified).map_err(|e| e.to_string())?;
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
            "[board]\nmcu = \"{}\"\n\n[communication]\ntransports = [\"{}\"]\n\n[bootloader]\nboot_window_ms = {}\n",
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
    fn parse_unified_bareboot_toml_sections() {
        let toml_str = r#"
[board]
name = "custom-h503"
mcu = "stm32h503kbu6"
family = "stm32h5"
system_clock_hz = 250000000

[memory]
flash_origin = 0x08000000
flash_size = 131072
bootloader_origin = 0x08000000
bootloader_size = 8192
app_origin = 0x08002000
app_size = 114688
nvm_origin = 0x0801E000
nvm_size = 8192
page_size = 8192

[communication]
transports = ["i2c", "i3c"]
address = 0x30
bitrate = 1000000

[bootloader]
enabled = true
type = "bareboot"
boot_window_ms = 300
preserve_nvm = true
initial_safe_pins = ["!PB15", "!PB1", "!PB0", "!PA7"]
mailbox_address = 0x20007FE0
mailbox_magic = 0x5984E3FA6CA1589B
"#;

        let cfg = parse_target_config(toml_str).unwrap();
        assert_eq!(cfg.target, "stm32h503kbu6");
        assert_eq!(cfg.transport, "i2c");
        assert_eq!(cfg.runtime_defaults.boot_window_ms, 300);
        assert_eq!(cfg.runtime_defaults.address, Some(0x30));
        assert_eq!(cfg.runtime_defaults.bitrate, Some(1_000_000));
        assert!(cfg.runtime_defaults.preserve_nvm);
        assert_eq!(
            cfg.runtime_defaults.initial_safe_pins,
            vec!["!PB15", "!PB1", "!PB0", "!PA7"]
        );
        assert_eq!(cfg.memory.flash_origin, Some(0x08000000));
        assert_eq!(cfg.memory.app_origin, Some(0x08002000));
        assert_eq!(cfg.memory.app_size, Some(114688));
        assert_eq!(cfg.memory.page_size, Some(8192));
    }

    #[test]
    fn auto_derive_safe_pins_from_barefoc_gate_driver() {
        let toml_str = r#"
[board]
name = "fishyservo-stm32h5"
mcu = "stm32h503kbu6"
family = "stm32h5"
system_clock_hz = 250000000

[memory]
flash_origin = 0x08000000
flash_size = 131072
bootloader_size = 8192
app_origin = 0x08002000
app_size = 106496
nvm_origin = 0x0801C000

[communication]
transports = ["i3c", "i2c"]
address = 0x40
bitrate = 1000000

[bootloader]
enabled = true
type = "bareboot"
boot_window_ms = 250

[gate_driver]
driver_type = "drv8411a"
sleep_pin = "PA9"
enable_pin = "!PB1"
inl_a = "PB5"
inl_b = "PB15"
inl_c = "PA7"
inh_a = "PA15"
inh_b = "PA8"
inh_c = "PA6"
fault_pin = "^!PB12"
"#;
        let cfg = parse_target_config(toml_str).unwrap();
        assert_eq!(cfg.target, "stm32h503kbu6");
        assert_eq!(cfg.transport, "i3c");
        assert_eq!(cfg.memory.flash_origin, Some(0x0800_0000));
        assert_eq!(cfg.memory.app_origin, Some(0x0800_2000));
        assert_eq!(cfg.memory.app_size, Some(106_496));
        assert_eq!(cfg.memory.nvm_origin, Some(0x0801_C000));
        assert_eq!(
            cfg.runtime_defaults.initial_safe_pins,
            vec![
                "!PA9", "PB1", "!PB5", "!PB15", "!PA7", "!PA15", "!PA8", "!PA6", "^PB12"
            ]
        );
    }

    #[test]
    fn load_target_config_from_directory() {
        let dir = env::temp_dir().join("bareboot-engine-dir-cfg-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let foc_file = dir.join("barefoc.toml");
        let text = r#"
[board]
mcu = "stm32h503"
family = "stm32h5"

[communication]
transports = ["multi"]

[bootloader]
boot_window_ms = 250
"#;
        fs::write(&foc_file, text).unwrap();

        let cfg = load_target_config(&dir).unwrap();
        assert_eq!(cfg.target, "stm32h503");
        assert_eq!(cfg.transport, "multi");
        assert_eq!(cfg.bootloader.status_led, None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_bootloader_status_led() {
        let toml_str = r#"
[board]
mcu = "stm32h503kbu6"
family = "stm32h5"

[communication]
transports = ["i3c"]

[bootloader]
boot_window_ms = 250
status_led = "!PA1"
"#;
        let cfg = parse_target_config(toml_str).unwrap();
        assert_eq!(cfg.bootloader.status_led.as_deref(), Some("!PA1"));
    }
}
