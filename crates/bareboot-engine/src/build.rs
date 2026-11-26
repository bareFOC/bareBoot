use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;
use crate::profile::FlashProfile;
use crate::worker::UiEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    BlinkTest,
    Bareboot,
    BarebootCh32v203Usb,
    TestFirmware,
}

impl BuildTarget {
    pub fn all() -> &'static [BuildTarget] {
        &[
            BuildTarget::BlinkTest,
            BuildTarget::Bareboot,
            BuildTarget::BarebootCh32v203Usb,
            BuildTarget::TestFirmware,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            BuildTarget::BlinkTest => "blink-test (STM32F103 App)",
            BuildTarget::Bareboot => "bareboot (Core Bootloader)",
            BuildTarget::BarebootCh32v203Usb => "bareboot-ch32v203-usb (USB BL)",
            BuildTarget::TestFirmware => "test-firmware (RISC-V/ARM Tests)",
        }
    }

    pub fn manifest_path(&self) -> &'static str {
        match self {
            BuildTarget::BlinkTest => "blink-test/Cargo.toml",
            BuildTarget::Bareboot => "bareboot/Cargo.toml",
            BuildTarget::BarebootCh32v203Usb => "bareboot-ch32v203-usb/Cargo.toml",
            BuildTarget::TestFirmware => "test-firmware/Cargo.toml",
        }
    }

    pub fn default_chip(&self) -> &'static str {
        match self {
            BuildTarget::BlinkTest => "STM32F103C8Tx",
            BuildTarget::Bareboot => "CH32V203C8T6",
            BuildTarget::BarebootCh32v203Usb => "CH32V203C8T6",
            BuildTarget::TestFirmware => "STM32F103C8Tx",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployMethod {
    ProbeRs,       // probe-rs SWD/JTAG
    KatapultCan,   // Katapult/BareBoot protocol over CAN
    KatapultSerial,// Katapult/BareBoot protocol over UART
    MemorySim,     // In-memory simulation
}

impl DeployMethod {
    pub fn all() -> &'static [DeployMethod] {
        &[
            DeployMethod::ProbeRs,
            DeployMethod::KatapultCan,
            DeployMethod::KatapultSerial,
            DeployMethod::MemorySim,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            DeployMethod::ProbeRs => "Probe-RS (SWD/JTAG)",
            DeployMethod::KatapultCan => "Katapult CAN Bus (can0)",
            DeployMethod::KatapultSerial => "Katapult Serial (/dev/ttyUSB0)",
            DeployMethod::MemorySim => "InMemory Sim",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildJob {
    pub manifest_path: String,
    pub release: bool,
    pub clean: bool,
}

/// Format resolved `initial_safe_pins` as a comma-separated Klipper-style string
/// for `BAREBOOT_SAFE_PINS` and `CONFIG_INITIAL_PINS`.
pub fn format_safe_pins_env(cfg: &crate::config::TargetConfig) -> String {
    cfg.runtime_defaults.initial_safe_pins.join(",")
}

pub fn stream_cargo_build(job: BuildJob, output: Sender<UiEvent>) {
    stream_cargo_build_with_config(job, None, output);
}

pub fn stream_cargo_build_with_config(
    job: BuildJob,
    target_cfg: Option<&crate::config::TargetConfig>,
    output: Sender<UiEvent>,
) {
    let safe_pins_env = target_cfg.map(format_safe_pins_env).unwrap_or_default();
    thread::spawn(move || {
        if job.clean {
            let _ = output.send(UiEvent::Log {
                line: format!("[Cargo] Cleaning {}", job.manifest_path),
            });
            let _ = Command::new("cargo")
                .args(["clean", "--manifest-path", &job.manifest_path])
                .output();
        }

        let mut args = vec!["build", "--manifest-path", &job.manifest_path];
        if job.release {
            args.push("--release");
        }

        let _ = output.send(UiEvent::Log {
            line: format!("[Cargo] Running cargo {}", args.join(" ")),
        });

        let mut command = Command::new("cargo");
        command.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());
        if !safe_pins_env.is_empty() {
            command.env("BAREBOOT_SAFE_PINS", &safe_pins_env);
            command.env("CONFIG_INITIAL_PINS", &safe_pins_env);
        }

        match command.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                if let Some(stderr) = child.stderr.take() {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                match child.wait() {
                    Ok(st) if st.success() => {
                        let _ = output.send(UiEvent::Done {
                            message: format!("Build completed successfully: {}", job.manifest_path),
                        });
                    }
                    Ok(st) => {
                        let _ = output.send(UiEvent::Error {
                            message: format!("Cargo build exited with code {:?}", st.code()),
                        });
                    }
                    Err(e) => {
                        let _ = output.send(UiEvent::Error {
                            message: format!("Wait failed: {e}"),
                        });
                    }
                }
            }
            Err(err) => {
                let _ = output.send(UiEvent::Error {
                    message: format!("failed to spawn cargo: {err}"),
                });
            }
        }
    });
}

pub fn stream_makeflash(
    manifest_path: String,
    release: bool,
    deploy_method: DeployMethod,
    chip_erase: bool,
    _hold_in_bootloader: bool,
    verify: bool,
    chip_name: String,
    profile: FlashProfile,
    output: Sender<UiEvent>,
) {
    thread::spawn(move || {
        let _ = output.send(UiEvent::Log {
            line: format!("-- Katapult Makeflash Pipeline Started: {manifest_path} --"),
        });

        // Step 1: Build
        let mut args = vec!["build", "--manifest-path", &manifest_path];
        if release {
            args.push("--release");
        }

        let mut build_cmd = Command::new("cargo");
        build_cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());

        let build_success = match build_cmd.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                if let Some(stderr) = child.stderr.take() {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let _ = output.send(UiEvent::Log { line });
                    }
                }
                child.wait().map(|st| st.success()).unwrap_or(false)
            }
            Err(e) => {
                let _ = output.send(UiEvent::Error { message: format!("build spawn failed: {e}") });
                false
            }
        };

        if !build_success {
            let _ = output.send(UiEvent::Error { message: "Makeflash aborted: build failed".to_string() });
            return;
        }

        // Step 2: Locate firmware artifact
        let project_dir = Path::new(&manifest_path).parent().unwrap_or(Path::new("."));
        let binary_path = discover_firmware(project_dir)
            .or_else(|| discover_firmware(Path::new(".")));

        let Some(bin) = binary_path else {
            let _ = output.send(UiEvent::Error { message: "No firmware binary (.bin/.elf) discovered after build".to_string() });
            return;
        };

        let bin_display = bin.display().to_string();
        let _ = output.send(UiEvent::Log {
            line: format!("[Makeflash] Discovered artifact: {bin_display}"),
        });

        // Step 3: Deploy via selected method
        match deploy_method {
            DeployMethod::ProbeRs => {
                let _ = output.send(UiEvent::Log {
                    line: format!("[Probe-RS] Flashing {bin_display} to {chip_name}..."),
                });
                let mut p_args = vec!["download".to_string(), "--chip".to_string(), chip_name.clone()];
                if chip_erase {
                    p_args.push("--chip-erase".to_string());
                }
                if verify {
                    p_args.push("--verify".to_string());
                }
                p_args.push(bin_display.clone());

                let mut cmd = Command::new("probe-rs");
                cmd.args(&p_args).stdout(Stdio::piped()).stderr(Stdio::piped());

                match cmd.spawn() {
                    Ok(mut child) => {
                        if let Some(stdout) = child.stdout.take() {
                            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                                let _ = output.send(UiEvent::Log { line });
                            }
                        }
                        if let Some(stderr) = child.stderr.take() {
                            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                                let _ = output.send(UiEvent::Log { line });
                            }
                        }
                        match child.wait() {
                            Ok(st) if st.success() => {
                                let _ = output.send(UiEvent::Progress { percent: 100 });
                                let _ = output.send(UiEvent::Done {
                                    message: format!("Makeflash complete: {chip_name} programmed via probe-rs"),
                                });
                            }
                            Ok(st) => {
                                let _ = output.send(UiEvent::Error {
                                    message: format!("probe-rs download exited with code {:?}", st.code()),
                                });
                            }
                            Err(e) => {
                                let _ = output.send(UiEvent::Error { message: format!("probe-rs wait error: {e}") });
                            }
                        }
                    }
                    Err(e) => {
                        let _ = output.send(UiEvent::Error { message: format!("failed to spawn probe-rs: {e}") });
                    }
                }
            }
            DeployMethod::KatapultCan | DeployMethod::KatapultSerial | DeployMethod::MemorySim => {
                let mut flash_prof = profile;
                flash_prof.firmware = bin_display;
                flash_prof.backend = match deploy_method {
                    DeployMethod::KatapultCan => "can".to_string(),
                    DeployMethod::KatapultSerial => "serial".to_string(),
                    _ => "memory".to_string(),
                };
                flash_prof.verify = verify;

                let bytes = match std::fs::read(&flash_prof.firmware) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = output.send(UiEvent::Error { message: format!("read firmware: {e}") });
                        return;
                    }
                };

                #[cfg(target_os = "linux")]
                {
                    use bareboot_client::{HostBus, InMemoryBus, SerialBus, SocketCanBus};
                    let bus: Box<dyn HostBus> = match flash_prof.backend.as_str() {
                        "serial" => match SerialBus::new(&flash_prof.serial_device, flash_prof.baud) {
                            Ok(b) => Box::new(b),
                            Err(e) => {
                                let _ = output.send(UiEvent::Error { message: e.to_string() });
                                return;
                            }
                        },
                        "can" => {
                            let nid = u32::from_str_radix(flash_prof.node_id.trim_start_matches("0x"), 16).unwrap_or(0x100);
                            match SocketCanBus::new(&flash_prof.can_interface, nid) {
                                Ok(b) => Box::new(b),
                                Err(e) => {
                                    let _ = output.send(UiEvent::Error { message: e.to_string() });
                                    return;
                                }
                            }
                        }
                        _ => Box::new(InMemoryBus::new_simulated()),
                    };

                    let tx = output.clone();
                    let res = bareboot_client::run_flash(
                        bus,
                        &bytes,
                        flash_prof.tries,
                        flash_prof.verify,
                        |p| {
                            let _ = tx.send(UiEvent::Progress { percent: p });
                            let frame = crate::protocol::decode_packet(
                                "LIVE",
                                crate::protocol::Direction::Tx,
                                crate::protocol::BusType::Can,
                                &flash_prof.node_id,
                                &[0x01, 0x88, crate::protocol::opcodes::RX_BLOCK, p],
                            );
                            let _ = tx.send(UiEvent::BusFrame(frame));
                        },
                        |l| {
                            let _ = tx.send(UiEvent::Log { line: l });
                        },
                    );
                    if let Err(e) = res {
                        let _ = output.send(UiEvent::Error { message: e.to_string() });
                        return;
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    use bareboot_client::InMemoryBus;
                    let bus = InMemoryBus::new_simulated();
                    let tx = output.clone();
                    let _ = bareboot_client::run_flash(
                        bus,
                        &bytes,
                        flash_prof.tries,
                        flash_prof.verify,
                        |p| { let _ = tx.send(UiEvent::Progress { percent: p }); },
                        |l| { let _ = tx.send(UiEvent::Log { line: l }); },
                    );
                }
                let _ = output.send(UiEvent::Done { message: "Katapult protocol makeflash complete".to_string() });
            }
        }
    });
}

pub fn discover_firmware(root: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    let target_dirs = [
        root.join("target/release"),
        root.join("target/debug"),
        root.join("target/thumbv7m-none-eabi/release"),
        root.join("target/thumbv7m-none-eabi/debug"),
        root.join("target/riscv32imac-unknown-none-elf/release"),
        root.join("target/riscv32imac-unknown-none-elf/debug"),
        root.join("out"),
    ];

    for dir in &target_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if matches!(ext, "bin" | "elf" | "hex" | "uf2") {
                        if let Ok(meta) = p.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                candidates.push((mtime, p));
                            }
                        }
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, p)| p)
}

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
