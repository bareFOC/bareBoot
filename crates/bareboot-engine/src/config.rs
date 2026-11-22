use bareboot_client::BarebootConfig;
#[cfg(not(target_arch = "wasm32"))]
use bareboot_client::parse_config;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::Path;

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
        if !path.exists() {
            // Return sensible default if file doesn't exist yet
            let default_cfg = BarebootConfig {
                target: "stm32f103".to_string(),
                transport: "can".to_string(),
                boot_window_ms: 250,
            };
            return Ok(default_cfg);
        }
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let config = parse_config(&text).map_err(|e| e.to_string())?;
        config.validate().map_err(|e| e.to_string())?;
        Ok(config)
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
    use bareboot_client::BarebootConfig;
    use std::env;

    #[test]
    fn round_trip() {
        let dir = env::temp_dir().join("bareboot-engine-cfg-test");
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
    }
}
