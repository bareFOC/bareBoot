pub use bareboot_engine::profile::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

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
