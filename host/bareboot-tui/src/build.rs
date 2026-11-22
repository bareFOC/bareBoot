pub use bareboot_engine::build::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_targets() {
        let targets = BuildTarget::all();
        assert_eq!(targets.len(), 4);
        assert!(targets[0].name().contains("blink-test"));
        assert_eq!(targets[0].manifest_path(), "blink-test/Cargo.toml");
    }

    #[test]
    fn test_deploy_methods() {
        let methods = DeployMethod::all();
        assert_eq!(methods.len(), 4);
        assert_eq!(methods[0], DeployMethod::ProbeRs);
    }
}
