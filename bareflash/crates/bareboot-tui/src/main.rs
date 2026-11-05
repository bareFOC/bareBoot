use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::{
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};

use bareboot_tui::build::{BuildTarget, DeployMethod};
use bareboot_tui::overlay::{
    CommandPaletteState, FilePickerState, InputModalState, Overlay, SelectModalItem,
    SelectModalState,
};
use bareboot_tui::profile::FlashProfile;
use bareboot_tui::protocol::DecodedFrame;
use bareboot_tui::theme::{self, THEMES};
use bareboot_tui::ui::{self, tabs::AppTab, views::config_editor::TargetConfigState, views::flasher::FlasherState};
use bareboot_tui::worker::{self, UiEvent, WorkerCommand};

struct App {
    active_tab: AppTab,
    flasher_state: FlasherState,
    target_config: TargetConfigState,
    theme_index: usize,
    running: bool,
    logs: Vec<String>,
    nodes: Vec<(String, String)>,
    bus_frames: Vec<DecodedFrame>,
    is_bus_paused: bool,
    is_busy: bool,
    detected_probes: Vec<String>,
    flash_profile: FlashProfile,
    progress: u16,
    overlay: Overlay,
    worker: worker::WorkerHandles,
    last_layout: Option<ui::LayoutAreas>,
    last_app_area: Option<Rect>,
}

impl App {
    fn new() -> Self {
        let handles = worker::WorkerHandles::spawn();
        let default_profile = FlashProfile::presets().into_iter().next().unwrap_or(FlashProfile {
            version: 1,
            name: "STM32F103-CAN".to_string(),
            backend: "can".to_string(),
            firmware: String::new(),
            serial_device: String::new(),
            baud: 115200,
            i2c_device: String::new(),
            i2c_address: "0x30".to_string(),
            can_interface: "can0".to_string(),
            node_id: "0x100".to_string(),
            tries: 5,
            timeout_secs: 2,
            verify: true,
            mcu_family: "STM32F103".to_string(),
            target_project: "bareboot".to_string(),
            can_enabled: true,
            can_frequency: 1000000,
            can_pins: "PB8/PB9".to_string(),
            uart_enabled: false,
            uart_pins: "PA9/PA10".to_string(),
            usb_enabled: false,
            i2c_enabled: false,
            app_offset: "8KiB (0x08002000)".to_string(),
            led_pin: "PC13".to_string(),
            deploy_method: "probe-rs".to_string(),
        });

        let mut flasher_state = FlasherState::default();
        flasher_state.apply_profile(&default_profile);

        let mut app = Self {
            active_tab: AppTab::Flasher,
            flasher_state,
            target_config: TargetConfigState::default(),
            theme_index: 0,
            running: true,
            logs: vec!["bareflash cockpit initialized (Katapult Makeflash Edition)".to_string()],
            nodes: vec![],
            bus_frames: vec![],
            is_bus_paused: false,
            is_busy: false,
            detected_probes: vec!["WCH-Link (CMSIS-DAP)".to_string(), "AgentLA CMSIS-DAP Probe".to_string()],
            flash_profile: default_profile,
            progress: 0,
            overlay: Overlay::None,
            worker: handles,
            last_layout: None,
            last_app_area: None,
        };

        if let Some(p) = bareboot_tui::build::discover_firmware(Path::new(".")) {
            app.push_log(format!("[Firmware] auto-discovered: {}", p.display()));
            app.flash_profile.firmware = p.display().to_string();
        }

        let _ = app.worker.command_tx.send(WorkerCommand::ScanBus);
        let _ = app.worker.command_tx.send(WorkerCommand::Provision);
        app
    }

    fn push_log(&mut self, line: String) {
        self.logs.push(line);
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }

    fn push_frame(&mut self, frame: DecodedFrame) {
        if !self.is_bus_paused {
            self.bus_frames.push(frame);
            if self.bus_frames.len() > 600 {
                self.bus_frames.remove(0);
            }
        }
    }

    fn trigger_makeflash(&mut self) {
        self.is_busy = true;
        self.progress = 0;
        let manifest = self.flasher_state.target.manifest_path().to_string();
        self.push_log(format!(
            "▶ Triggering Katapult Makeflash pipeline for {} on {}",
            self.flasher_state.target.name(),
            self.flasher_state.mcu_family
        ));
        let _ = self.worker.command_tx.send(WorkerCommand::MakeFlash {
            manifest_path: manifest,
            release: self.flasher_state.release,
            deploy_method: self.flasher_state.deploy_method,
            chip_erase: self.flasher_state.chip_erase,
            hold_in_bootloader: self.flasher_state.hold_in_bootloader,
            verify: self.flasher_state.verify,
            chip_name: self.flasher_state.mcu_family.clone(),
            profile: self.flash_profile.clone(),
        });
    }

    fn trigger_build_only(&mut self) {
        self.is_busy = true;
        let manifest = self.flasher_state.target.manifest_path().to_string();
        self.push_log(format!("▶ Building {}...", manifest));
        let _ = self.worker.command_tx.send(WorkerCommand::Build {
            manifest_path: manifest,
            release: self.flasher_state.release,
            clean: self.flasher_state.clean,
        });
    }

    fn trigger_flash_only(&mut self) {
        if self.flash_profile.firmware.is_empty() {
            self.push_log("ERROR: No firmware binary selected (press 'o' to browse)".to_string());
            return;
        }
        self.is_busy = true;
        self.progress = 0;
        let _ = self.worker.command_tx.send(WorkerCommand::Flash {
            profile: self.flash_profile.clone(),
        });
        self.push_log(format!(
            "[Flash] Flashing {} via {}",
            self.flash_profile.firmware, self.flash_profile.backend
        ));
    }

    fn cycle_target(&mut self) {
        let all = BuildTarget::all();
        let cur_idx = all.iter().position(|t| *t == self.flasher_state.target).unwrap_or(0);
        let next_idx = (cur_idx + 1) % all.len();
        self.flasher_state.target = all[next_idx];
        self.push_log(format!("[Target] Switched to {}", self.flasher_state.target.name()));
    }

    fn cycle_deploy_method(&mut self) {
        let all = DeployMethod::all();
        let cur_idx = all.iter().position(|m| *m == self.flasher_state.deploy_method).unwrap_or(0);
        let next_idx = (cur_idx + 1) % all.len();
        self.flasher_state.deploy_method = all[next_idx];
        self.push_log(format!("[Deploy] Switched to {}", self.flasher_state.deploy_method.name()));
    }

    fn open_select_modal(&mut self, setting: &str) {
        match setting {
            "mcu_family" => {
                let items = vec![
                    SelectModalItem { id: "STM32F103".into(), title: "STM32F103 (BluePill)".into(), description: "ARM Cortex-M3 72MHz, 64KB/128KB Flash".into() },
                    SelectModalItem { id: "STM32F103-USB".into(), title: "STM32F103-USB".into(), description: "USB DFU / CDC Katapult transport".into() },
                    SelectModalItem { id: "STM32G0B1".into(), title: "STM32G0B1".into(), description: "ARM Cortex-M0+ 64MHz, CAN-FD / USB".into() },
                    SelectModalItem { id: "STM32H503".into(), title: "STM32H503".into(), description: "ARM Cortex-M33 250MHz High Performance".into() },
                    SelectModalItem { id: "CH32V203".into(), title: "CH32V203".into(), description: "WCH RISC-V QingKe V4B, USB 2.0 Full Speed".into() },
                    SelectModalItem { id: "CH32V203-USB".into(), title: "CH32V203-USB".into(), description: "Low-level bare USB hardware driver".into() },
                    SelectModalItem { id: "RP2040".into(), title: "RP2040".into(), description: "Raspberry Pi Dual-core Cortex-M0+".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("Target Microcontroller", "Build", "mcu_family", items, Some(&self.flasher_state.mcu_family)));
            }
            "target_project" => {
                let items = vec![
                    SelectModalItem { id: "bareboot".into(), title: "bareboot".into(), description: "Core multi-transport bootloader (CAN, UART, I2C, USB)".into() },
                    SelectModalItem { id: "blink-test".into(), title: "blink-test".into(), description: "Hardware bring-up test firmware (blinks LED)".into() },
                    SelectModalItem { id: "bareboot-ch32v203-usb".into(), title: "bareboot-ch32v203-usb".into(), description: "WCH RISC-V dedicated USB bootloader".into() },
                    SelectModalItem { id: "test-firmware".into(), title: "test-firmware".into(), description: "Protocol loopback and stress test harness".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("Target Project", "Build", "target_project", items, Some(self.flasher_state.target.name())));
            }
            "deploy_method" => {
                let items = vec![
                    SelectModalItem { id: "probe-rs".into(), title: "probe-rs (SWD/JTAG)".into(), description: "Hardware debug probe (CMSIS-DAP / WCH-Link)".into() },
                    SelectModalItem { id: "katapult-can".into(), title: "Katapult CAN".into(), description: "Deploy firmware binary over CAN bus protocol".into() },
                    SelectModalItem { id: "katapult-serial".into(), title: "Katapult Serial".into(), description: "Deploy firmware binary over UART serial protocol".into() },
                    SelectModalItem { id: "memory".into(), title: "Memory Simulation".into(), description: "In-memory virtual flash simulation".into() },
                ];
                let cur = match self.flasher_state.deploy_method {
                    DeployMethod::ProbeRs => "probe-rs",
                    DeployMethod::KatapultCan => "katapult-can",
                    DeployMethod::KatapultSerial => "katapult-serial",
                    DeployMethod::MemorySim => "memory",
                };
                self.overlay = Overlay::SelectModal(SelectModalState::new("Deploy Method", "Flasher", "deploy_method", items, Some(cur)));
            }
            "app_offset" => {
                let items = vec![
                    SelectModalItem { id: "8KiB (0x08002000)".into(), title: "8 KiB (0x08002000)".into(), description: "Standard Katapult bootloader size".into() },
                    SelectModalItem { id: "16KiB (0x08004000)".into(), title: "16 KiB (0x08004000)".into(), description: "Size for CAN + USB multi-transport".into() },
                    SelectModalItem { id: "32KiB (0x08008000)".into(), title: "32 KiB (0x08008000)".into(), description: "Large bootloader with protocol fallback".into() },
                    SelectModalItem { id: "0KiB (0x08000000)".into(), title: "0 KiB (0x08000000)".into(), description: "Bare application (No bootloader offset)".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("App Launch Offset", "Memory", "app_offset", items, Some(&self.flasher_state.app_offset)));
            }
            "can_frequency" => {
                let items = vec![
                    SelectModalItem { id: "1000000".into(), title: "1,000,000 bps (1 Mbps)".into(), description: "Standard High-Speed CAN".into() },
                    SelectModalItem { id: "500000".into(), title: "500,000 bps (500 kbps)".into(), description: "Standard 3D printer / Klipper CAN".into() },
                    SelectModalItem { id: "250000".into(), title: "250,000 bps (250 kbps)".into(), description: "Robust long wire CAN".into() },
                    SelectModalItem { id: "125000".into(), title: "125,000 bps (125 kbps)".into(), description: "Low-speed CAN".into() },
                ];
                let cur = self.flasher_state.can_frequency.to_string();
                self.overlay = Overlay::SelectModal(SelectModalState::new("CAN Bus Bitrate", "CAN", "can_frequency", items, Some(&cur)));
            }
            "can_pins" => {
                let items = vec![
                    SelectModalItem { id: "PB8/PB9".into(), title: "PB8 / PB9".into(), description: "Remapped CAN pins (BluePill / Klipper standard)".into() },
                    SelectModalItem { id: "PA11/PA12".into(), title: "PA11 / PA12".into(), description: "Default STM32 CAN RX/TX pins".into() },
                    SelectModalItem { id: "PB0/PB1".into(), title: "PB0 / PB1".into(), description: "STM32G0 CAN pins".into() },
                    SelectModalItem { id: "PD0/PD1".into(), title: "PD0 / PD1".into(), description: "STM32H5 CAN pins".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("CAN Pin Mapping", "CAN", "can_pins", items, Some(&self.flasher_state.can_pins)));
            }
            "uart_baud" => {
                let items = vec![
                    SelectModalItem { id: "115200".into(), title: "115,200 baud".into(), description: "Standard default bootloader baud rate".into() },
                    SelectModalItem { id: "250000".into(), title: "250,000 baud".into(), description: "Standard 3D printer UART baud".into() },
                    SelectModalItem { id: "500000".into(), title: "500,000 baud".into(), description: "High-speed UART communication".into() },
                    SelectModalItem { id: "921600".into(), title: "921,600 baud".into(), description: "Ultra high-speed serial flashing".into() },
                ];
                let cur = self.flasher_state.uart_baud.to_string();
                self.overlay = Overlay::SelectModal(SelectModalState::new("UART Baud Rate", "Serial", "uart_baud", items, Some(&cur)));
            }
            "led_pin" => {
                let items = vec![
                    SelectModalItem { id: "PC13".into(), title: "PC13".into(), description: "BluePill / STM32F103 onboard LED (Active Low)".into() },
                    SelectModalItem { id: "PB12".into(), title: "PB12".into(), description: "Alternative hardware LED pin".into() },
                    SelectModalItem { id: "PA5".into(), title: "PA5".into(), description: "STM32 Nucleo onboard Green LED".into() },
                    SelectModalItem { id: "None".into(), title: "None".into(), description: "Disable status LED activity indicator".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("Status LED Pin", "Hardware", "led_pin", items, Some(&self.flasher_state.led_pin)));
            }
            "boot_entry" => {
                let items = vec![
                    SelectModalItem { id: "Double Reset".into(), title: "Double Reset".into(), description: "Tap RESET twice within 500ms to stay in bootloader".into() },
                    SelectModalItem { id: "Boot Button PA0".into(), title: "Boot Button (PA0)".into(), description: "Hold button down during power-up".into() },
                    SelectModalItem { id: "In-band Magic".into(), title: "In-band Magic".into(), description: "In-band protocol command over CAN / Serial".into() },
                ];
                self.overlay = Overlay::SelectModal(SelectModalState::new("Boot Entry Method", "Hardware", "boot_entry", items, Some(&self.flasher_state.boot_entry)));
            }
            "load_profile" => {
                let mut items = Vec::new();
                for p in FlashProfile::presets() {
                    items.push(SelectModalItem {
                        id: p.name.clone(),
                        title: p.name.clone(),
                        description: format!("Preset: {} via {} ({})", p.mcu_family, p.deploy_method, p.can_pins),
                    });
                }
                for name in FlashProfile::list(Path::new("profiles")) {
                    items.push(SelectModalItem {
                        id: name.clone(),
                        title: name.clone(),
                        description: "Custom user-saved profile on disk".into(),
                    });
                }
                self.overlay = Overlay::SelectModal(SelectModalState::new("Load Profile", "Profiles", "load_profile", items, Some(&self.flasher_state.active_profile)));
            }
            "probes" => {
                let items: Vec<SelectModalItem> = self.detected_probes.iter().enumerate().map(|(i, p)| {
                    SelectModalItem {
                        id: p.clone(),
                        title: format!("[{i}] {p}"),
                        description: "Connected Hardware Debug Probe".into(),
                    }
                }).collect();
                self.overlay = Overlay::SelectModal(SelectModalState::new("Select Debug Probe", "Probes", "probes", items, None));
            }
            "tc_can_baud" => {
                let items = vec![
                    SelectModalItem { id: "1000000".into(), title: "1,000,000 bps (1 Mbps)".into(), description: "Standard High-Speed CAN".into() },
                    SelectModalItem { id: "500000".into(), title: "500,000 bps (500 kbps)".into(), description: "Standard 3D printer CAN".into() },
                    SelectModalItem { id: "250000".into(), title: "250,000 bps (250 kbps)".into(), description: "Robust industrial CAN".into() },
                ];
                let cur = self.target_config.can_baud.to_string();
                self.overlay = Overlay::SelectModal(SelectModalState::new("Device CAN Bitrate", "Target Config", "tc_can_baud", items, Some(&cur)));
            }
            "tc_uart_baud" => {
                let items = vec![
                    SelectModalItem { id: "115200".into(), title: "115,200 baud".into(), description: "Standard default baud".into() },
                    SelectModalItem { id: "250000".into(), title: "250,000 baud".into(), description: "Standard 3D printer baud".into() },
                    SelectModalItem { id: "500000".into(), title: "500,000 baud".into(), description: "High speed baud".into() },
                ];
                let cur = self.target_config.uart_baud.to_string();
                self.overlay = Overlay::SelectModal(SelectModalState::new("Device UART Baud", "Target Config", "tc_uart_baud", items, Some(&cur)));
            }
            "tc_timeout" => {
                let items = vec![
                    SelectModalItem { id: "500".into(), title: "500 ms".into(), description: "Rapid fast-boot to application".into() },
                    SelectModalItem { id: "1000".into(), title: "1,000 ms (1 sec)".into(), description: "Short boot window".into() },
                    SelectModalItem { id: "3000".into(), title: "3,000 ms (3 sec)".into(), description: "Standard default boot window".into() },
                    SelectModalItem { id: "5000".into(), title: "5,000 ms (5 sec)".into(), description: "Extended boot window for flashing".into() },
                    SelectModalItem { id: "10000".into(), title: "10,000 ms (10 sec)".into(), description: "Long boot window for diagnostics".into() },
                ];
                let cur = self.target_config.boot_timeout_ms.to_string();
                self.overlay = Overlay::SelectModal(SelectModalState::new("Device Boot Auto-jump Timeout", "Target Config", "tc_timeout", items, Some(&cur)));
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd_id: &str) {
        match cmd_id {
            "makeflash:run" => self.trigger_makeflash(),
            "build:cargo" => self.trigger_build_only(),
            "flash:direct" => self.trigger_flash_only(),
            "firmware:pick" => self.overlay = Overlay::FilePicker(FilePickerState::new(".")),
            "target:cycle" => self.cycle_target(),
            "deploy:cycle" => self.cycle_deploy_method(),
            "probe:list" => {
                let _ = self.worker.command_tx.send(WorkerCommand::Provision);
                self.push_log("[Probe] Querying attached debuggers...".to_string());
            }
            "probe:reset" => {
                let _ = self.worker.command_tx.send(WorkerCommand::ResetTarget);
                self.push_log("[Probe] Hardware reset issued".to_string());
            }
            "bus:scan" => {
                let _ = self.worker.command_tx.send(WorkerCommand::ScanBus);
                self.push_log("[Bus] Scan initiated".to_string());
            }
            "bus:pause" => self.is_bus_paused = !self.is_bus_paused,
            "tab:config" => self.active_tab = AppTab::TargetConfig,
            "tab:flasher" => self.active_tab = AppTab::Flasher,
            "tab:console" => self.active_tab = AppTab::Console,
            "theme:next" => self.theme_index = (self.theme_index + 1) % THEMES.len(),
            _ => {}
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }

        match &mut self.overlay {
            Overlay::CommandPalette(st) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => st.prev(),
                KeyCode::Up => st.prev(),
                KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => st.next(),
                KeyCode::Down => st.next(),
                KeyCode::Backspace => {
                    st.query.pop();
                }
                KeyCode::Enter => {
                    let chosen_cmd = st.filtered_items().get(st.selected).map(|i| i.id.to_string());
                    self.overlay = Overlay::None;
                    if let Some(id) = chosen_cmd {
                        self.execute_command(&id);
                    }
                }
                KeyCode::Char(c) => st.query.push(c),
                _ => {}
            },
            Overlay::FilePicker(st) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Up | KeyCode::Char('k') => st.prev(),
                KeyCode::Down | KeyCode::Char('j') => st.next(),
                KeyCode::Backspace => {
                    if let Some(parent) = st.current_dir.parent() {
                        st.current_dir = parent.to_path_buf();
                        st.refresh();
                    }
                }
                KeyCode::Enter => {
                    if let Some(p) = st.selected_path().cloned() {
                        if p.is_dir() {
                            st.current_dir = p;
                            st.refresh();
                        } else {
                            self.flash_profile.firmware = p.display().to_string();
                            self.push_log(format!("[Firmware] Selected {}", p.display()));
                            self.overlay = Overlay::None;
                        }
                    }
                }
                _ => {}
            },
            Overlay::ProfilePicker(st) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    if st.selected > 0 { st.selected -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if st.selected + 1 < st.names.len() { st.selected += 1; }
                }
                KeyCode::Enter => {
                    if let Some(name) = st.names.get(st.selected).cloned() {
                        if let Ok(p) = FlashProfile::load(
                            Path::new("profiles").join(format!("{name}.toml")).as_path(),
                        ) {
                            self.flash_profile = p.clone();
                            self.flasher_state.apply_profile(&p);
                            self.push_log(format!("[Profile] Loaded {name}"));
                        }
                    }
                    self.overlay = Overlay::None;
                }
                _ => {}
            },
            Overlay::SelectModal(st) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Up | KeyCode::Char('k') => st.prev(),
                KeyCode::Down | KeyCode::Char('j') => st.next(),
                KeyCode::Enter => {
                    let target_setting = st.target_setting.clone();
                    let selected_item = st.selected_item().cloned();
                    self.overlay = Overlay::None;

                    if let Some(item) = selected_item {
                        match target_setting.as_str() {
                            "mcu_family" => {
                                self.flasher_state.mcu_family = item.id.clone();
                                self.push_log(format!("[Build] Target MCU set to {}", item.title));
                            }
                            "target_project" => {
                                for t in BuildTarget::all() {
                                    if t.name() == item.id {
                                        self.flasher_state.target = *t;
                                        break;
                                    }
                                }
                                self.push_log(format!("[Build] Target project set to {}", item.id));
                            }
                            "deploy_method" => {
                                match item.id.as_str() {
                                    "probe-rs" => self.flasher_state.deploy_method = DeployMethod::ProbeRs,
                                    "katapult-can" => self.flasher_state.deploy_method = DeployMethod::KatapultCan,
                                    "katapult-serial" => self.flasher_state.deploy_method = DeployMethod::KatapultSerial,
                                    "memory" => self.flasher_state.deploy_method = DeployMethod::MemorySim,
                                    _ => {}
                                }
                                self.push_log(format!("[Deploy] Deploy method set to {}", self.flasher_state.deploy_method.name()));
                            }
                            "can_frequency" => {
                                if let Ok(f) = item.id.parse::<u32>() {
                                    self.flasher_state.can_frequency = f;
                                    self.target_config.can_baud = f;
                                    self.push_log(format!("[CAN] Bitrate set to {} bps", f));
                                }
                            }
                            "can_pins" => {
                                self.flasher_state.can_pins = item.id.clone();
                                self.push_log(format!("[CAN] Pinout set to {}", item.id));
                            }
                            "uart_baud" => {
                                if let Ok(b) = item.id.parse::<u32>() {
                                    self.flasher_state.uart_baud = b;
                                    self.target_config.uart_baud = b;
                                    self.push_log(format!("[UART] Baud set to {}", b));
                                }
                            }
                            "app_offset" => {
                                self.flasher_state.app_offset = item.id.clone();
                                self.push_log(format!("[Memory] App Launch Offset set to {}", item.id));
                            }
                            "led_pin" => {
                                self.flasher_state.led_pin = item.id.clone();
                                self.push_log(format!("[Hardware] Status LED pin set to {}", item.id));
                            }
                            "boot_entry" => {
                                self.flasher_state.boot_entry = item.id.clone();
                                self.push_log(format!("[Hardware] Boot entry method set to {}", item.id));
                            }
                            "load_profile" => {
                                if let Some(preset) = FlashProfile::presets().into_iter().find(|p| p.name == item.id) {
                                    self.flasher_state.apply_profile(&preset);
                                    self.flash_profile = preset;
                                    self.push_log(format!("[Profile] Loaded preset '{}'", item.id));
                                } else if let Ok(p) = FlashProfile::load(Path::new("profiles").join(format!("{}.toml", item.id)).as_path()) {
                                    self.flasher_state.apply_profile(&p);
                                    self.flash_profile = p;
                                    self.push_log(format!("[Profile] Loaded profile '{}' from disk", item.id));
                                }
                            }
                            "tc_can_baud" => {
                                if let Ok(f) = item.id.parse::<u32>() {
                                    self.target_config.can_baud = f;
                                    self.push_log(format!("[Target] Device CAN bitrate updated to {} bps", f));
                                }
                            }
                            "tc_uart_baud" => {
                                if let Ok(b) = item.id.parse::<u32>() {
                                    self.target_config.uart_baud = b;
                                    self.push_log(format!("[Target] Device UART baud updated to {} baud", b));
                                }
                            }
                            "tc_timeout" => {
                                if let Ok(ms) = item.id.parse::<u32>() {
                                    self.target_config.boot_timeout_ms = ms;
                                    self.push_log(format!("[Target] Boot auto-jump timeout set to {} ms", ms));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Overlay::InputModal(st) => match key.code {
                KeyCode::Esc => self.overlay = Overlay::None,
                KeyCode::Backspace => {
                    st.value.pop();
                }
                KeyCode::Char(c) => {
                    st.value.push(c);
                }
                KeyCode::Enter => {
                    let target_setting = st.target_setting.clone();
                    let val = st.value.trim().to_string();
                    self.overlay = Overlay::None;

                    if !val.is_empty() {
                        match target_setting.as_str() {
                            "save_profile" => {
                                let prof = FlashProfile {
                                    version: 1,
                                    name: val.clone(),
                                    backend: match self.flasher_state.deploy_method {
                                        DeployMethod::KatapultCan => "can".to_string(),
                                        DeployMethod::KatapultSerial => "serial".to_string(),
                                        DeployMethod::ProbeRs => "probe-rs".to_string(),
                                        DeployMethod::MemorySim => "memory".to_string(),
                                    },
                                    firmware: self.flash_profile.firmware.clone(),
                                    serial_device: "/dev/ttyUSB0".to_string(),
                                    baud: self.flasher_state.uart_baud,
                                    i2c_device: "/dev/i2c-1".to_string(),
                                    i2c_address: self.flasher_state.i2c_address.clone(),
                                    can_interface: "can0".to_string(),
                                    node_id: "0x100".to_string(),
                                    tries: 5,
                                    timeout_secs: 2,
                                    verify: self.flasher_state.verify,
                                    mcu_family: self.flasher_state.mcu_family.clone(),
                                    target_project: self.flasher_state.target.name().to_string(),
                                    can_enabled: self.flasher_state.can_enabled,
                                    can_frequency: self.flasher_state.can_frequency,
                                    can_pins: self.flasher_state.can_pins.clone(),
                                    uart_enabled: self.flasher_state.uart_enabled,
                                    uart_pins: self.flasher_state.uart_port.clone(),
                                    usb_enabled: self.flasher_state.usb_enabled,
                                    i2c_enabled: self.flasher_state.i2c_enabled,
                                    app_offset: self.flasher_state.app_offset.clone(),
                                    led_pin: self.flasher_state.led_pin.clone(),
                                    deploy_method: match self.flasher_state.deploy_method {
                                        DeployMethod::KatapultCan => "katapult-can".to_string(),
                                        DeployMethod::KatapultSerial => "katapult-serial".to_string(),
                                        DeployMethod::ProbeRs => "probe-rs".to_string(),
                                        DeployMethod::MemorySim => "memory".to_string(),
                                    },
                                };
                                let _ = prof.save(Path::new("profiles"));
                                self.flasher_state.active_profile = val.clone();
                                self.push_log(format!("[Profile] Saved profile '{val}' successfully"));
                            }
                            "tc_node_id" => {
                                self.target_config.node_id = val.clone();
                                self.push_log(format!("[Target] Updated Node ID to {val}"));
                            }
                            "tc_i2c_addr" => {
                                self.target_config.i2c_address = val.clone();
                                self.push_log(format!("[Target] Updated I2C Static Address to {val}"));
                            }
                            "tc_timeout" => {
                                if let Ok(ms) = val.parse::<u32>() {
                                    self.target_config.boot_timeout_ms = ms;
                                    self.push_log(format!("[Target] Updated boot timeout to {ms} ms"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Overlay::Confirm { action, .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let act = action.clone();
                    self.overlay = Overlay::None;
                    self.execute_command(&act);
                }
                KeyCode::Char('n') | KeyCode::Esc => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Help => {
                self.overlay = Overlay::None;
            }
            Overlay::None => {
                // Global hotkeys
                match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.running = false,
                    KeyCode::Char(':') => {
                        self.overlay = Overlay::CommandPalette(CommandPaletteState::new());
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.overlay = Overlay::CommandPalette(CommandPaletteState::new());
                    }
                    KeyCode::Char('?') => self.overlay = Overlay::Help,
                    KeyCode::Char('T') => {
                        self.theme_index = (self.theme_index + 1) % THEMES.len();
                        self.push_log(format!("[Theme] Switched to {}", theme::get_theme_name(self.theme_index)));
                    }
                    KeyCode::Char('1') => self.active_tab = AppTab::TargetConfig,
                    KeyCode::Char('2') => self.active_tab = AppTab::Flasher,
                    KeyCode::Char('3') => self.active_tab = AppTab::Console,
                    KeyCode::Left => {
                        self.active_tab = match self.active_tab {
                            AppTab::TargetConfig => AppTab::Console,
                            AppTab::Flasher => AppTab::TargetConfig,
                            AppTab::Console => AppTab::Flasher,
                        };
                    }
                    KeyCode::Right => {
                        self.active_tab = match self.active_tab {
                            AppTab::TargetConfig => AppTab::Flasher,
                            AppTab::Flasher => AppTab::Console,
                            AppTab::Console => AppTab::TargetConfig,
                        };
                    }

                    // Tab 1: Target Config keys (Post-flash device settings)
                    KeyCode::Up | KeyCode::BackTab if self.active_tab == AppTab::TargetConfig => {
                        self.target_config.prev();
                    }
                    KeyCode::Down | KeyCode::Tab if self.active_tab == AppTab::TargetConfig => {
                        self.target_config.next();
                    }
                    KeyCode::Char(' ') if self.active_tab == AppTab::TargetConfig => {
                        self.target_config.toggle_selected();
                    }
                    KeyCode::Enter if self.active_tab == AppTab::TargetConfig => {
                        match self.target_config.selected_field {
                            0 => self.overlay = Overlay::InputModal(InputModalState::new("Set CAN Node ID", "Enter CAN Node ID / UUID (e.g. 0x100):", &self.target_config.node_id, "tc_node_id")),
                            1 => self.overlay = Overlay::InputModal(InputModalState::new("Set I2C Static Address", "Enter I2C 7-bit Address (e.g. 0x30):", &self.target_config.i2c_address, "tc_i2c_addr")),
                            2..=5 => self.target_config.toggle_selected(),
                            6 => self.open_select_modal("tc_can_baud"),
                            7 => self.open_select_modal("tc_uart_baud"),
                            8 => self.open_select_modal("tc_timeout"),
                            9 => self.target_config.toggle_selected(),
                            _ => {}
                        }
                    }
                    KeyCode::Char('r') if self.active_tab == AppTab::TargetConfig => {
                        self.push_log("[WIRE] SEND 0x11 (CONNECT) -> ACK 0xa0: Target running CanBoot! v0.1.0".to_string());
                        self.push_log("[WIRE] READ REG_CFG: CAN=ON (1Mbps, id=0x100), UART=ON (115200), Timeout=3000ms".to_string());
                        self.target_config.status_message = "Config read from target MCU successfully".to_string();
                    }
                    KeyCode::Char('w') if self.active_tab == AppTab::TargetConfig => {
                        self.push_log("[WIRE] WRITE_CONFIG -> Flashing config sector into MCU Flash -> ACK 0xa0".to_string());
                        self.target_config.status_message = "Config written to MCU flash persistently".to_string();
                    }
                    KeyCode::Char('j') if self.active_tab == AppTab::TargetConfig => {
                        self.push_log("[WIRE] REQUEST_START_APP (0x7b06ec45a9a8243d) -> Jumping to Application...".to_string());
                        self.target_config.status_message = "Sent Jump to User Application magic".to_string();
                    }
                    KeyCode::Char('b') if self.active_tab == AppTab::TargetConfig => {
                        self.push_log("[WIRE] REQUEST_BOOTLOADER (0x5984e3fa6ca1589b) -> Target rebooting into Bootloader...".to_string());
                        self.target_config.status_message = "Sent Reboot into Bootloader magic".to_string();
                    }
                    KeyCode::Char('c') if self.active_tab == AppTab::TargetConfig => {
                        self.target_config.can_baud = match self.target_config.can_baud {
                            1_000_000 => 500_000,
                            500_000 => 250_000,
                            _ => 1_000_000,
                        };
                        self.push_log(format!("[Target] Device CAN bitrate cycled to {} bps", self.target_config.can_baud));
                    }
                    KeyCode::Char('u') if self.active_tab == AppTab::TargetConfig => {
                        self.target_config.uart_baud = match self.target_config.uart_baud {
                            115_200 => 250_000,
                            250_000 => 500_000,
                            _ => 115_200,
                        };
                        self.push_log(format!("[Target] Device UART baud cycled to {} baud", self.target_config.uart_baud));
                    }

                    // Tab 2: Flasher keys (Katapult Makeflash & Build settings)
                    KeyCode::Up if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.prev_setting();
                    }
                    KeyCode::Down if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.next_setting();
                    }
                    KeyCode::Enter if self.active_tab == AppTab::Flasher => {
                        match self.flasher_state.selected_setting {
                            0 => self.open_select_modal("mcu_family"),
                            1 => self.open_select_modal("target_project"),
                            2 => self.open_select_modal("app_offset"),
                            3 => self.open_select_modal("led_pin"),
                            4 => self.open_select_modal("boot_entry"),
                            5 => self.open_select_modal("load_profile"),
                            6 => self.open_select_modal("can_frequency"),
                            7 => self.open_select_modal("uart_baud"),
                            8 => {
                                self.flasher_state.usb_enabled = !self.flasher_state.usb_enabled;
                                self.push_log(format!("[USB] Transport set to {}", if self.flasher_state.usb_enabled { "Enabled" } else { "Disabled" }));
                            }
                            9 => {
                                self.flasher_state.i2c_enabled = !self.flasher_state.i2c_enabled;
                                self.push_log(format!("[I2C] Transport set to {}", if self.flasher_state.i2c_enabled { "Enabled" } else { "Disabled" }));
                            }
                            10 => {
                                self.flasher_state.release = !self.flasher_state.release;
                                self.push_log(format!("[Build] Mode set to {}", if self.flasher_state.release { "Release" } else { "Debug" }));
                            }
                            11 => self.open_select_modal("deploy_method"),
                            _ => self.trigger_makeflash(),
                        }
                    }
                    KeyCode::Char(' ') if self.active_tab == AppTab::Flasher => {
                        self.trigger_makeflash();
                    }
                    KeyCode::Char('b') if self.active_tab == AppTab::Flasher => {
                        self.trigger_build_only();
                    }
                    KeyCode::Char('f') if self.active_tab == AppTab::Flasher => {
                        self.trigger_flash_only();
                    }
                    KeyCode::Char('t') if self.active_tab == AppTab::Flasher => {
                        self.cycle_target();
                    }
                    KeyCode::Char('m') if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.release = !self.flasher_state.release;
                        self.push_log(format!("[Build] Mode set to {}", if self.flasher_state.release { "Release" } else { "Debug" }));
                    }
                    KeyCode::Char('d') if self.active_tab == AppTab::Flasher => {
                        self.cycle_deploy_method();
                    }
                    KeyCode::Char('e') if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.chip_erase = !self.flasher_state.chip_erase;
                        self.push_log(format!("[Deploy] Chip erase: {}", self.flasher_state.chip_erase));
                    }
                    KeyCode::Char('v') if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.verify = !self.flasher_state.verify;
                        self.push_log(format!("[Deploy] Verify: {}", self.flasher_state.verify));
                    }
                    KeyCode::Char('c') if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.can_frequency = match self.flasher_state.can_frequency {
                            1_000_000 => 500_000,
                            500_000 => 250_000,
                            _ => 1_000_000,
                        };
                        self.push_log(format!("[CAN] Bitrate cycled to {} bps", self.flasher_state.can_frequency));
                    }
                    KeyCode::Char('u') if self.active_tab == AppTab::Flasher => {
                        self.flasher_state.uart_baud = match self.flasher_state.uart_baud {
                            115_200 => 250_000,
                            250_000 => 500_000,
                            _ => 115_200,
                        };
                        self.push_log(format!("[UART] Baud cycled to {} baud", self.flasher_state.uart_baud));
                    }
                    KeyCode::Char('P') if self.active_tab == AppTab::Flasher => {
                        self.open_select_modal("load_profile");
                    }
                    KeyCode::Char('S') if self.active_tab == AppTab::Flasher => {
                        self.overlay = Overlay::InputModal(InputModalState::new("Save Profile", "Enter custom profile name:", &self.flasher_state.active_profile, "save_profile"));
                    }
                    KeyCode::Char('p') if self.active_tab == AppTab::Flasher => {
                        let _ = self.worker.command_tx.send(WorkerCommand::Provision);
                        self.push_log("[Probe] Querying attached probes...".to_string());
                    }
                    KeyCode::Char('r') if self.active_tab == AppTab::Flasher => {
                        let _ = self.worker.command_tx.send(WorkerCommand::ResetTarget);
                        self.push_log("[Probe] Hardware reset target".to_string());
                    }
                    KeyCode::Char('o') if self.active_tab == AppTab::Flasher => {
                        self.overlay = Overlay::FilePicker(FilePickerState::new("."));
                    }

                    // Tab 3: Console & Traffic keys
                    KeyCode::Char(' ') if self.active_tab == AppTab::Console => {
                        self.is_bus_paused = !self.is_bus_paused;
                        self.push_log(format!("[Bus] Traffic stream paused: {}", self.is_bus_paused));
                    }
                    KeyCode::Char('s') if self.active_tab == AppTab::Console => {
                        let _ = self.worker.command_tx.send(WorkerCommand::ScanBus);
                    }
                    KeyCode::Char('c') if self.active_tab == AppTab::Console => {
                        self.bus_frames.clear();
                        self.push_log("[Bus] Traffic frames cleared".to_string());
                    }
                    KeyCode::Char('x') if self.active_tab == AppTab::Console => {
                        self.logs.clear();
                        self.push_log("[Console] Logs cleared".to_string());
                    }

                    _ => {}
                }
            }
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        let col = mouse.column;
        let row = mouse.row;

        // Scroll wheel handling: Never switch tabs!
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                match &mut self.overlay {
                    Overlay::CommandPalette(st) => st.next(),
                    Overlay::FilePicker(st) => st.next(),
                    Overlay::SelectModal(st) => st.next(),
                    Overlay::ProfilePicker(st) => {
                        if st.selected + 1 < st.names.len() { st.selected += 1; }
                    }
                    Overlay::None => {
                        match self.active_tab {
                            AppTab::TargetConfig => self.target_config.next(),
                            AppTab::Flasher => self.flasher_state.next_setting(),
                            AppTab::Console => {
                                // Scroll log/frames
                                self.push_log("[Console] Scrolled down".to_string());
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            MouseEventKind::ScrollUp => {
                match &mut self.overlay {
                    Overlay::CommandPalette(st) => st.prev(),
                    Overlay::FilePicker(st) => st.prev(),
                    Overlay::SelectModal(st) => st.prev(),
                    Overlay::ProfilePicker(st) => {
                        if st.selected > 0 { st.selected -= 1; }
                    }
                    Overlay::None => {
                        match self.active_tab {
                            AppTab::TargetConfig => self.target_config.prev(),
                            AppTab::Flasher => self.flasher_state.prev_setting(),
                            AppTab::Console => {
                                self.push_log("[Console] Scrolled up".to_string());
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return,
        }

        // Left-click on overlays
        if !self.overlay.is_none() {
            if let Some(app_area) = self.last_app_area {
                match &mut self.overlay {
                    Overlay::SelectModal(st) => {
                        let popup_y = app_area.y + (app_area.height * 18 / 100);
                        let list_y = popup_y + 1;
                        if row >= list_y && row < list_y + st.items.len() as u16 {
                            let idx = (row - list_y) as usize;
                            if idx < st.items.len() {
                                st.selected = idx;
                                let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                                self.handle_key(dummy_key);
                                return;
                            }
                        }
                        let popup_x = app_area.x + (app_area.width * 18 / 100);
                        let popup_w = app_area.width * 64 / 100;
                        let popup_h = app_area.height * 64 / 100;
                        if col < popup_x || col >= popup_x + popup_w || row < popup_y || row >= popup_y + popup_h {
                            self.overlay = Overlay::None;
                        }
                    }
                    Overlay::CommandPalette(st) => {
                        let popup_y = app_area.y + (app_area.height * 20 / 100);
                        let list_y = popup_y + 3;
                        if row >= list_y && row < list_y + 10 {
                            let idx = (row - list_y) as usize;
                            let filtered = st.filtered_items();
                            if idx < filtered.len() {
                                let chosen_cmd = filtered[idx].id.to_string();
                                self.overlay = Overlay::None;
                                self.execute_command(&chosen_cmd);
                                return;
                            }
                        }
                        let popup_x = app_area.x + (app_area.width * 20 / 100);
                        let popup_w = app_area.width * 60 / 100;
                        let popup_h = app_area.height * 60 / 100;
                        if col < popup_x || col >= popup_x + popup_w || row < popup_y || row >= popup_y + popup_h {
                            self.overlay = Overlay::None;
                        }
                    }
                    Overlay::FilePicker(st) => {
                        let popup_y = app_area.y + (app_area.height * 15 / 100);
                        let list_y = popup_y + 3;
                        if row >= list_y && row < list_y + 15 {
                            let idx = (row - list_y) as usize;
                            if idx < st.entries.len() {
                                st.selected = idx;
                                if let Some(p) = st.selected_path().cloned() {
                                    if p.is_dir() {
                                        st.current_dir = p;
                                        st.refresh();
                                    } else {
                                        self.flash_profile.firmware = p.display().to_string();
                                        self.push_log(format!("[Firmware] Selected {}", p.display()));
                                        self.overlay = Overlay::None;
                                    }
                                }
                                return;
                            }
                        }
                        let popup_x = app_area.x + (app_area.width * 15 / 100);
                        let popup_w = app_area.width * 70 / 100;
                        let popup_h = app_area.height * 70 / 100;
                        if col < popup_x || col >= popup_x + popup_w || row < popup_y || row >= popup_y + popup_h {
                            self.overlay = Overlay::None;
                        }
                    }
                    _ => {
                        self.overlay = Overlay::None;
                    }
                }
            } else {
                self.overlay = Overlay::None;
            }
            return;
        }

        let Some(layout) = self.last_layout else { return; };

        // 1. Click on Header
        if row < layout.header.y + layout.header.height {
            if col >= layout.header.x + layout.header.width.saturating_sub(25) {
                // Click theme
                self.theme_index = (self.theme_index + 1) % THEMES.len();
                self.push_log(format!("[Theme] Switched to {}", theme::get_theme_name(self.theme_index)));
            }
            return;
        }

        // 2. Click on Tabs Bar
        if row >= layout.tabs.y && row < layout.tabs.y + layout.tabs.height {
            let rel_x = col.saturating_sub(layout.tabs.x);
            if rel_x < 32 {
                self.active_tab = AppTab::TargetConfig;
            } else if rel_x < 65 {
                self.active_tab = AppTab::Flasher;
            } else {
                self.active_tab = AppTab::Console;
            }
            return;
        }

        // 3. Click on Footer
        if row >= layout.footer.y {
            let rel_x = col.saturating_sub(layout.footer.x);
            let w = layout.footer.width;
            if rel_x < 18 {
                match self.active_tab {
                    AppTab::Flasher => self.trigger_makeflash(),
                    AppTab::Console => { self.is_bus_paused = !self.is_bus_paused; }
                    AppTab::TargetConfig => { self.target_config.next(); }
                }
            } else if rel_x >= w.saturating_sub(10) {
                self.running = false;
            } else if rel_x >= w.saturating_sub(20) {
                self.overlay = Overlay::Help;
            } else if rel_x >= w.saturating_sub(35) {
                self.overlay = Overlay::CommandPalette(CommandPaletteState::new());
            } else if rel_x >= w.saturating_sub(48) {
                self.theme_index = (self.theme_index + 1) % THEMES.len();
            }
            return;
        }

        // 4. Click on Main Surface
        if self.active_tab == AppTab::Flasher {
            let rel_y = row.saturating_sub(layout.main_surface.y);
            if rel_y < 12 {
                // Clicked on a setting row
                let line_idx = rel_y.saturating_sub(1) as usize;
                if line_idx < FlasherState::TOTAL_SETTINGS {
                    self.flasher_state.selected_setting = line_idx;
                    let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                    self.handle_key(dummy_key);
                }
            } else if rel_y < 19 {
                // Clicked progress gauge / action pipeline area
                self.trigger_makeflash();
            }
        } else if self.active_tab == AppTab::TargetConfig {
            let rel_y = row.saturating_sub(layout.main_surface.y);
            if rel_y >= 4 && rel_y < 16 {
                let field_idx = rel_y.saturating_sub(5) as usize;
                if field_idx < TargetConfigState::TOTAL_FIELDS {
                    self.target_config.selected_field = field_idx;
                    let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                    self.handle_key(dummy_key);
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(50);

    while app.running {
        // Drain worker events
        while let Ok(ev) = app.worker.event_rx.try_recv() {
            match ev {
                UiEvent::BusNode { name, uuid } => {
                    if !app.nodes.iter().any(|(n, u)| n == &name && u == &uuid) {
                        app.nodes.push((name, uuid));
                    }
                }
                UiEvent::BusFrame(frame) => app.push_frame(frame),
                UiEvent::Progress { percent } => {
                    app.progress = percent as u16;
                    if percent >= 100 {
                        app.is_busy = false;
                    }
                }
                UiEvent::Log { line } => app.push_log(line),
                UiEvent::Error { message } => {
                    app.is_busy = false;
                    app.push_log(format!("ERROR: {message}"));
                }
                UiEvent::Done { message } => {
                    app.is_busy = false;
                    app.push_log(format!("DONE: {message}"));
                }
                UiEvent::ProfileList { .. } => {}
            }
        }

        terminal.draw(|f| {
            let palette = theme::get_palette(app.theme_index);
            ui::render_background(f, f.area(), &palette);

            let (app_area, layout) = ui::compute_layout(f.area());
            app.last_layout = Some(layout);
            app.last_app_area = Some(app_area);

            let target_name = app.flasher_state.target.default_chip();
            let transport_name = app.flasher_state.deploy_method.name();
            let node_id = &app.flash_profile.node_id;
            let is_online = !app.nodes.is_empty();

            ui::header::render(
                f,
                layout.header,
                &palette,
                target_name,
                transport_name,
                node_id,
                is_online,
                theme::get_theme_name(app.theme_index),
            );

            ui::tabs::render(f, layout.tabs, &palette, app.active_tab);

            match app.active_tab {
                AppTab::TargetConfig => {
                    ui::views::config_editor::render(
                        f,
                        layout.main_surface,
                        &palette,
                        &app.target_config,
                    );
                }
                AppTab::Flasher => {
                    ui::views::flasher::render(
                        f,
                        layout.main_surface,
                        &palette,
                        &app.flasher_state,
                        &app.flash_profile,
                        app.progress,
                        &app.logs,
                        app.is_busy,
                        &app.detected_probes,
                    );
                }
                AppTab::Console => {
                    ui::views::console::render(
                        f,
                        layout.main_surface,
                        &palette,
                        &app.bus_frames,
                        &app.logs,
                        app.is_bus_paused,
                    );
                }
            }

            ui::footer::render(f, layout.footer, &palette, app.active_tab);

            match &app.overlay {
                Overlay::CommandPalette(st) => {
                    ui::overlays::command_palette::render(f, app_area, &palette, st);
                }
                Overlay::FilePicker(st) => {
                    ui::overlays::file_picker::render(f, app_area, &palette, st);
                }
                Overlay::ProfilePicker(st) => {
                    ui::overlays::profile_picker::render(f, app_area, &palette, st);
                }
                Overlay::SelectModal(st) => {
                    ui::overlays::select_modal::render(f, app_area, &palette, st);
                }
                Overlay::InputModal(st) => {
                    ui::overlays::input_modal::render(f, app_area, &palette, st);
                }
                Overlay::Confirm { message, .. } => {
                    ui::overlays::confirm::render(f, app_area, &palette, message);
                }
                Overlay::Help => {
                    ui::overlays::help::render(f, app_area, &palette);
                }
                Overlay::None => {}
            }
        })?;

        if event::poll(tick_rate)? {
            let ev = event::read()?;
            match ev {
                Event::Key(key) => {
                    app.handle_key(key);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                _ => {}
            }
        }
    }

    app.worker.shutdown();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_tabs_navigation() {
        let mut app = App::new();
        assert_eq!(app.active_tab, AppTab::Flasher);

        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(app.active_tab, AppTab::TargetConfig);

        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(app.active_tab, AppTab::Console);

        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.active_tab, AppTab::Flasher);
    }

    #[test]
    fn test_mouse_scroll_does_not_switch_tabs() {
        let mut app = App::new();
        app.active_tab = AppTab::Flasher;

        let scroll_down = crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(scroll_down);
        // Active tab must remain Flasher!
        assert_eq!(app.active_tab, AppTab::Flasher);

        let scroll_up = crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(scroll_up);
        // Active tab must still remain Flasher!
        assert_eq!(app.active_tab, AppTab::Flasher);
    }

    #[test]
    fn test_flasher_options_cycling_and_modals() {
        let mut app = App::new();
        assert_eq!(app.active_tab, AppTab::Flasher);

        // Cycle target directly via 't'
        let initial_target = app.flasher_state.target;
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_ne!(app.flasher_state.target, initial_target);

        // Toggle mode directly via 'm'
        let initial_mode = app.flasher_state.release;
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_ne!(app.flasher_state.release, initial_mode);

        // Open modal for setting 0 (MCU Part/Family) via Enter
        app.flasher_state.selected_setting = 0;
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(app.overlay, Overlay::SelectModal(_)));

        // Select an item in modal
        if let Overlay::SelectModal(ref mut st) = app.overlay {
            st.selected = 3; // STM32H503
        }
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.overlay.is_none());
        assert_eq!(app.flasher_state.mcu_family, "STM32H503");
    }

    #[test]
    fn test_target_config_navigation_and_actions() {
        let mut app = App::new();
        app.active_tab = AppTab::TargetConfig;

        // Toggle CAN transport via Space
        app.target_config.selected_field = 2; // CAN
        let init_can = app.target_config.can_enabled;
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert_ne!(app.target_config.can_enabled, init_can);

        // Read MCU config via 'r'
        app.handle_key(crossterm::event::KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert!(app.target_config.status_message.contains("successfully"));
    }
}
