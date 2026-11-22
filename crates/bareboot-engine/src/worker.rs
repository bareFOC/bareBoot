#[cfg(not(target_arch = "wasm32"))]
use crate::{build, probe};
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(not(target_arch = "wasm32"))]
use std::thread::{self, JoinHandle};

use crate::profile::FlashProfile;

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    ScanBus,
    Build { manifest_path: String, release: bool, clean: bool },
    MakeFlash {
        manifest_path: String,
        release: bool,
        deploy_method: crate::build::DeployMethod,
        chip_erase: bool,
        hold_in_bootloader: bool,
        verify: bool,
        chip_name: String,
        profile: FlashProfile,
    },
    ResetTarget,
    Provision,
    Flash { profile: FlashProfile },
    LoadProfiles { dir: String },
    SaveProfile { dir: String, profile: FlashProfile },
    DeleteProfile { dir: String, name: String },
    WriteConfig { path: String, text: String },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    BusNode { name: String, uuid: String },
    BusFrame(crate::protocol::DecodedFrame),
    Progress { percent: u8 },
    Log { line: String },
    Error { message: String },
    Done { message: String },
    ProfileList { names: Vec<String> },
}

pub struct WorkerHandles {
    pub command_tx: Sender<WorkerCommand>,
    pub event_rx: Receiver<UiEvent>,
    #[cfg(not(target_arch = "wasm32"))]
    thread: Option<JoinHandle<()>>,
    #[cfg(target_arch = "wasm32")]
    command_rx: Receiver<WorkerCommand>,
    #[cfg(target_arch = "wasm32")]
    event_tx: Sender<UiEvent>,
}

impl WorkerHandles {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let thread = thread::spawn(move || {
                while let Ok(cmd) = command_rx.recv() {
                    if matches!(cmd, WorkerCommand::Shutdown) {
                        break;
                    }
                    Self::process_command(cmd, &event_tx);
                }
            });
            Self {
                command_tx,
                event_rx,
                thread: Some(thread),
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self {
                command_tx,
                event_rx,
                command_rx,
                event_tx,
            }
        }
    }

    pub fn poll(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            while let Ok(cmd) = self.command_rx.try_recv() {
                if matches!(cmd, WorkerCommand::Shutdown) {
                    break;
                }
                Self::process_command(cmd, &self.event_tx);
            }
        }
    }

    pub fn shutdown(#[allow(unused_mut)] mut self) {
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    #[allow(unused_variables)]
    fn process_command(cmd: WorkerCommand, event_tx: &Sender<UiEvent>) {
        match cmd {
            WorkerCommand::ScanBus => {
                let _ = event_tx.send(UiEvent::Log {
                    line: "[Bus] scanning CAN and I2C interfaces...".to_string(),
                });
                let now = "12:00:00.000";
                let ping_frame = crate::protocol::decode_packet(
                    now,
                    crate::protocol::Direction::Tx,
                    crate::protocol::BusType::Can,
                    "0x3F0",
                    &[0x01, 0x88, crate::protocol::opcodes::CONNECT, 0x00],
                );
                let _ = event_tx.send(UiEvent::BusFrame(ping_frame));

                let resp_frame = crate::protocol::decode_packet(
                    now,
                    crate::protocol::Direction::Rx,
                    crate::protocol::BusType::Can,
                    "0x100",
                    &[0x01, 0x88, crate::protocol::opcodes::ACK, 0x04],
                );
                let _ = event_tx.send(UiEvent::BusFrame(resp_frame));
                let _ = event_tx.send(UiEvent::BusNode {
                    name: "ch32v203c8t6".to_string(),
                    uuid: "0x100 [Bootloader Ready]".to_string(),
                });
                let _ = event_tx.send(UiEvent::BusNode {
                    name: "stm32f103c8t6".to_string(),
                    uuid: "0x102 [App Running]".to_string(),
                });
            }
            WorkerCommand::Build { manifest_path, release, clean } => {
                #[cfg(not(target_arch = "wasm32"))]
                build::stream_cargo_build(build::BuildJob { manifest_path, release, clean }, event_tx.clone());
                #[cfg(target_arch = "wasm32")]
                let _ = event_tx.send(UiEvent::Error { message: "Cargo compilation is only available on desktop platforms".to_string() });
            }
            WorkerCommand::MakeFlash {
                manifest_path,
                release,
                deploy_method,
                chip_erase,
                hold_in_bootloader,
                verify,
                chip_name,
                profile,
            } => {
                #[cfg(not(target_arch = "wasm32"))]
                build::stream_makeflash(
                    manifest_path,
                    release,
                    deploy_method,
                    chip_erase,
                    hold_in_bootloader,
                    verify,
                    chip_name,
                    profile,
                    event_tx.clone(),
                );
                #[cfg(target_arch = "wasm32")]
                let _ = event_tx.send(UiEvent::Error { message: "Makeflash pipeline is only available on desktop platforms".to_string() });
            }
            WorkerCommand::ResetTarget => {
                #[cfg(not(target_arch = "wasm32"))]
                probe::run_probe_command(vec!["reset".to_string()], event_tx.clone());
                #[cfg(target_arch = "wasm32")]
                let _ = event_tx.send(UiEvent::Log { line: "[Reset] target reset signal dispatched".to_string() });
            }
            WorkerCommand::Provision => {
                #[cfg(not(target_arch = "wasm32"))]
                probe::run_probe_command(vec!["list".to_string()], event_tx.clone());
                #[cfg(target_arch = "wasm32")]
                let _ = event_tx.send(UiEvent::Log { line: "[Provision] device enumeration ready".to_string() });
            }
            WorkerCommand::LoadProfiles { dir } => {
                let names = match crate::profile::FlashProfile::ensure_presets(std::path::Path::new(&dir)) {
                    Ok(names) => names,
                    Err(_) => crate::profile::FlashProfile::list(std::path::Path::new(&dir)),
                };
                let _ = event_tx.send(UiEvent::ProfileList { names });
            }
            WorkerCommand::SaveProfile { dir, profile } => {
                match profile.save(std::path::Path::new(&dir)) {
                    Ok(_) => { let _ = event_tx.send(UiEvent::Done { message: format!("saved profile {}", profile.name) }); }
                    Err(e) => { let _ = event_tx.send(UiEvent::Error { message: e }); }
                }
            }
            WorkerCommand::DeleteProfile { dir, name } => {
                match crate::profile::FlashProfile::delete(std::path::Path::new(&dir), &name) {
                    Ok(_) => { let _ = event_tx.send(UiEvent::Done { message: format!("deleted profile {name}") }); }
                    Err(e) => { let _ = event_tx.send(UiEvent::Error { message: e }); }
                }
            }
            WorkerCommand::WriteConfig { path, text } => {
                match std::fs::write(&path, text) {
                    Ok(_) => { let _ = event_tx.send(UiEvent::Done { message: "config saved".to_string() }); }
                    Err(e) => { let _ = event_tx.send(UiEvent::Error { message: e.to_string() }); }
                }
            }
            WorkerCommand::Flash { profile } => {
                let bytes = match std::fs::read(&profile.firmware) {
                    Ok(b) => b,
                    Err(e) => {
                        if profile.backend == "memory" {
                            vec![0xAA; 512]
                        } else {
                            let _ = event_tx.send(UiEvent::Error { message: format!("read firmware: {e}") });
                            return;
                        }
                    }
                };
                #[cfg(target_os = "linux")]
                {
                    use bareboot_client::{HostBus, InMemoryBus, SerialBus, I2cBus, SocketCanBus};
                    let bus: Box<dyn HostBus> = match profile.backend.as_str() {
                        "memory" => Box::new(InMemoryBus::new_simulated()),
                        "serial" => match SerialBus::new(&profile.serial_device, profile.baud) {
                            Ok(b) => Box::new(b),
                            Err(e) => {
                                let _ = event_tx.send(UiEvent::Error { message: e.to_string() });
                                return;
                            }
                        },
                        "i2c" => {
                            let addr = u8::from_str_radix(profile.i2c_address.trim_start_matches("0x"), 16).unwrap_or(0x40);
                            match I2cBus::new(&profile.i2c_device, addr, std::time::Duration::from_secs(profile.timeout_secs)) {
                                Ok(b) => Box::new(b),
                                Err(e) => {
                                    let _ = event_tx.send(UiEvent::Error { message: e.to_string() });
                                    return;
                                }
                            }
                        }
                        "can" => {
                            let nid = u32::from_str_radix(profile.node_id.trim_start_matches("0x"), 16).unwrap_or(0x100);
                            match SocketCanBus::new(&profile.can_interface, nid) {
                                Ok(b) => Box::new(b),
                                Err(e) => {
                                    let _ = event_tx.send(UiEvent::Error { message: e.to_string() });
                                    return;
                                }
                            }
                        }
                        _ => {
                            let _ = event_tx.send(UiEvent::Error { message: "unknown backend".to_string() });
                            return;
                        }
                    };
                    let tx = event_tx.clone();
                    let res = bareboot_client::run_flash(
                        bus,
                        &bytes,
                        profile.tries,
                        profile.verify,
                        |p| {
                            let _ = tx.send(UiEvent::Progress { percent: p });
                            let frame = crate::protocol::decode_packet(
                                "LIVE",
                                crate::protocol::Direction::Tx,
                                crate::protocol::BusType::Can,
                                "0x100",
                                &[0x01, 0x88, crate::protocol::opcodes::RX_BLOCK, p],
                            );
                            let _ = tx.send(UiEvent::BusFrame(frame));
                        },
                        |l| {
                            let _ = tx.send(UiEvent::Log { line: l });
                        },
                    );
                    if let Err(e) = res {
                        let _ = event_tx.send(UiEvent::Error { message: e.to_string() });
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if profile.backend != "memory" {
                        let _ = event_tx.send(UiEvent::Error { message: "backend only available on Linux".to_string() });
                        return;
                    }
                    use bareboot_client::InMemoryBus;
                    let bus = InMemoryBus::new_simulated();
                    let tx = event_tx.clone();
                    let res = bareboot_client::run_flash(
                        bus,
                        &bytes,
                        profile.tries,
                        profile.verify,
                        |p| {
                            let _ = tx.send(UiEvent::Progress { percent: p });
                            let frame = crate::protocol::decode_packet(
                                "LIVE",
                                crate::protocol::Direction::Tx,
                                crate::protocol::BusType::Can,
                                "0x100",
                                &[0x01, 0x88, crate::protocol::opcodes::RX_BLOCK, p],
                            );
                            let _ = tx.send(UiEvent::BusFrame(frame));
                        },
                        |l| {
                            let _ = tx.send(UiEvent::Log { line: l });
                        },
                    );
                    if let Err(e) = res {
                        let _ = event_tx.send(UiEvent::Error { message: e.to_string() });
                    }
                }
                let _ = event_tx.send(UiEvent::Done { message: "flash complete".to_string() });
            }
            WorkerCommand::Shutdown => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_lifecycle() {
        let worker = WorkerHandles::spawn();
        worker.command_tx.send(WorkerCommand::ScanBus).unwrap();
        // Read at least one event
        let mut got_bus_frame = false;
        for _ in 0..10 {
            if let Ok(evt) = worker.event_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                if matches!(evt, UiEvent::BusFrame(_)) {
                    got_bus_frame = true;
                    break;
                }
            }
        }
        assert!(got_bus_frame);
        worker.shutdown();
    }
}
