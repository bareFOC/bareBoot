use crate::{build, probe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::profile::FlashProfile;

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    ScanBus,
    Build { manifest_path: String },
    Provision,
    Flash { profile: FlashProfile },
    LoadProfiles { dir: String },
    SaveProfile { dir: String, profile: FlashProfile },
    DeleteProfile { dir: String, name: String },
    WriteConfig { path: String, text: String },
    FactoryFlash { method: String, firmware: String },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    BusNode { name: String, uuid: String },
    Progress { percent: u8 },
    Log { line: String },
    Error { message: String },
    Done { message: String },
    ProfileList { names: Vec<String> },
}

pub struct WorkerHandles {
    pub command_tx: Sender<WorkerCommand>,
    pub event_rx: Receiver<UiEvent>,
    thread: Option<JoinHandle<()>>,
}

impl WorkerHandles {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            while let Ok(cmd) = command_rx.recv() {
                match cmd {
                    WorkerCommand::ScanBus => {
                        let _ = event_tx.send(UiEvent::Log {
                            line: "[Bus] scan requested".to_string(),
                        });
                    }
                    WorkerCommand::Build { manifest_path } => {
                        build::stream_cargo_build(build::BuildJob { manifest_path }, event_tx.clone());
                    }
                    WorkerCommand::Provision => {
                        probe::run_probe_command(vec!["list".to_string()], event_tx.clone());
                    }
                    WorkerCommand::LoadProfiles { dir } => {
                        let names = crate::profile::FlashProfile::list(std::path::Path::new(&dir));
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
                            Err(e) => { let _ = event_tx.send(UiEvent::Error { message: format!("read firmware: {e}") }); continue; }
                        };
                        #[cfg(target_os = "linux")]
                        {
                            use bareboot_client::{HostBus, InMemoryBus, SerialBus, I2cBus, SocketCanBus};
                            let bus: Box<dyn HostBus> = match profile.backend.as_str() {
                                "memory" => Box::new(InMemoryBus::new()),
                                "serial" => match SerialBus::new(&profile.serial_device, profile.baud) { Ok(b)=>Box::new(b), Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:e.to_string()}); continue; } },
                                "i2c" => {
                                    let addr = u8::from_str_radix(profile.i2c_address.trim_start_matches("0x"),16).unwrap_or(0x40);
                                    match I2cBus::new(&profile.i2c_device, addr, std::time::Duration::from_secs(profile.timeout_secs)) { Ok(b)=>Box::new(b), Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:e.to_string()}); continue; } }
                                }
                                "can" => {
                                    let nid = u32::from_str_radix(profile.node_id.trim_start_matches("0x"),16).unwrap_or(0x100);
                                    match SocketCanBus::new(&profile.can_interface, nid) { Ok(b)=>Box::new(b), Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:e.to_string()}); continue; } }
                                }
                                _ => { let _=event_tx.send(UiEvent::Error{message:"unknown backend".to_string()}); continue; }
                            };
                            let tx = event_tx.clone();
                            let res = bareboot_client::run_flash(bus, &bytes, profile.tries, profile.verify,
                                |p| { let _ = tx.send(UiEvent::Progress { percent: p }); },
                                |l| { let _ = tx.send(UiEvent::Log { line: l }); });
                            if let Err(e) = res { let _ = event_tx.send(UiEvent::Error { message: e.to_string() }); }
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            if profile.backend != "memory" {
                                let _ = event_tx.send(UiEvent::Error { message: "backend only available on Linux".to_string() });
                                continue;
                            }
                            use bareboot_client::{HostBus, InMemoryBus};
                            let bus = InMemoryBus::new();
                            let tx = event_tx.clone();
                            let res = bareboot_client::run_flash(bus, &bytes, profile.tries, profile.verify,
                                |p| { let _ = tx.send(UiEvent::Progress { percent: p }); },
                                |l| { let _ = tx.send(UiEvent::Log { line: l }); });
                            if let Err(e) = res { let _ = event_tx.send(UiEvent::Error { message: e.to_string() }); }
                        }
                        let _ = event_tx.send(UiEvent::Done { message: "flash complete".to_string() });
                    }
                    WorkerCommand::FactoryFlash { method, firmware } => {
                        if method == "probe" {
                            probe::run_probe_command(vec!["flash".to_string(), firmware], event_tx.clone());
                        } else {
                            #[cfg(target_os = "linux")]
                            {
                                use std::io::Write;
                                let bytes = match std::fs::read(&firmware) { Ok(b)=>b, Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:format!("read firmware: {e}")}); continue; } };
                                let profile = crate::profile::FlashProfile { version:1, name:"factory".into(), backend:"serial".into(), firmware: firmware.clone(), serial_device: "/dev/ttyACM0".into(), baud:115200, i2c_device:String::new(), i2c_address:"0x40".into(), can_interface:String::new(), node_id:"0x100".into(), tries:5, timeout_secs:2, verify:true };
                                let _ = event_tx.send(UiEvent::Log { line: format!("serial factory flash of {firmware}: sending SERIAL_BL_REQ") });
                                let mut bus = match bareboot_client::SerialBus::new("/dev/ttyACM0", 115200) { Ok(b)=>b, Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:e.to_string()}); continue; } };
                                let _ = bus.write_raw(b"~ \x1c Request Serial Bootloader!! ~");
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                let bus2 = match bareboot_client::SerialBus::new("/dev/ttyACM0", 115200) { Ok(b)=>b, Err(e)=>{ let _=event_tx.send(UiEvent::Error{message:e.to_string()}); continue; } };
                                let bus = bus2;
                                let tx = event_tx.clone();
                                let _ = bareboot_client::run_flash(bus, &bytes, 5, true, |p|{let _=tx.send(UiEvent::Progress{percent:p});}, |l|{let _=tx.send(UiEvent::Log{line:l});});
                                let _ = event_tx.send(UiEvent::Done { message: "serial factory flash complete".to_string() });
                            }
                            #[cfg(not(target_os = "linux"))]
                            {
                                let _ = event_tx.send(UiEvent::Error { message: "serial factory only on Linux".to_string() });
                            }
                        }
                    }
                    WorkerCommand::Shutdown => break,
                }
            }
        });
        Self {
            command_tx,
            event_rx,
            thread: Some(thread),
        }
    }

    pub fn shutdown(mut self) {
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
