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
}

fn default_version() -> u32 { 1 }
fn default_baud() -> u32 { 115200 }
fn default_i2c_address() -> String { "0x40".to_string() }
fn default_node_id() -> String { "0x100".to_string() }
fn default_tries() -> usize { 5 }
fn default_timeout() -> u64 { 2 }
fn default_verify() -> bool { true }

impl FlashProfile {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn round_trip() {
        let dir = env::temp_dir().join("bareboot-profile-test");
        let _ = fs::remove_dir_all(&dir);
        let p = FlashProfile {
            version: 1,
            name: "demo".to_string(),
            backend: "memory".to_string(),
            firmware: "fw.bin".to_string(),
            serial_device: String::new(),
            baud: 115200,
            i2c_device: String::new(),
            i2c_address: "0x40".to_string(),
            can_interface: String::new(),
            node_id: "0x100".to_string(),
            tries: 3,
            timeout_secs: 4,
            verify: false,
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
            version: 1, name: "x".into(), backend: "zigbee".into(),
            firmware: String::new(), serial_device: String::new(), baud: 115200,
            i2c_device: String::new(), i2c_address: "0x40".into(),
            can_interface: String::new(), node_id: "0x100".into(),
            tries: 5, timeout_secs: 2, verify: true,
        };
        assert!(p.validate().is_err());
        p.backend = "serial".into();
        assert!(p.validate().is_ok());
    }
}