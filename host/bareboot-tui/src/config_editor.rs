pub use bareboot_engine::config::*;

#[cfg(test)]
mod tests {
    use super::*;
    use bareboot_client::BarebootConfig;
    use std::env;
    use std::fs;

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
