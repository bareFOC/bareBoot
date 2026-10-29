use bareboot_client::{BarebootConfig, parse_config};
use std::fs;
use std::path::Path;

pub fn load_config(path: &Path) -> Result<BarebootConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let config = parse_config(&text).map_err(|e| e.to_string())?;
    config.validate().map_err(|e| e.to_string())?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &BarebootConfig) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    let text = format!(
        "target = \"{}\"\ntransport = \"{}\"\nboot_window_ms = {}\n",
        config.target, config.transport, config.boot_window_ms
    );
    fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bareboot_client::BarebootConfig;
    use std::env;

    #[test]
    fn round_trip() {
        let dir = env::temp_dir().join("bareboot-cfg-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bareboot.toml");
        let cfg = BarebootConfig { target: "stm32g0".to_string(), transport: "can".to_string(), boot_window_ms: 250 };
        save_config(&path, &cfg).unwrap();
        let loaded = load_config(&path).unwrap();
        assert_eq!(cfg, loaded);
        let _ = fs::remove_dir_all(&dir);
    }
}
