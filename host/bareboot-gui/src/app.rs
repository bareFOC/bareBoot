use std::path::PathBuf;
use egui::{CentralPanel, Color32, Context, Id, Layout as EguiLayout, Panel, RichText, Ui};

use bareboot_client::BarebootConfig;
use bareboot_engine::build::{BuildTarget, DeployMethod};
use bareboot_engine::profile::FlashProfile;
use bareboot_engine::protocol::DecodedFrame;
use bareboot_engine::worker::{UiEvent, WorkerCommand, WorkerHandles};

use craft_ui::docking::{DockArea, DockStyle, Layout, Node, Permissions};
use craft_ui::layout::{SplitAxis, SplitSize};

use crate::docs::{self, DocTopic};
use crate::panels::{
    render_config_panel, render_console_panel, render_makeflash_panel, render_traffic_panel,
    MakeflashActions,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelId {
    TargetConfig,
    MakeflashControl,
    BuildConsole,
    BusTraffic,
    Documentation,
}

impl PanelId {
    pub fn title(&self) -> &'static str {
        match self {
            PanelId::TargetConfig => "Target Config",
            PanelId::MakeflashControl => "Makeflash Pipeline",
            PanelId::BuildConsole => "Build Console",
            PanelId::BusTraffic => "Bus Traffic Sniffer",
            PanelId::Documentation => "Documentation & Manuals",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkbenchTab {
    Flasher,
    Config,
    Console,
    Traffic,
    Docs,
}

pub type MobileTab = WorkbenchTab;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Tabbed,
    Docked,
}

pub struct BarebootApp {
    pub worker: WorkerHandles,
    pub dock_layout: Layout<PanelId>,
    pub active_tab: WorkbenchTab,
    pub view_mode: ViewMode,

    pub active_profile: FlashProfile,
    pub active_config: BarebootConfig,
    pub available_profiles: Vec<String>,
    pub profile_dir: PathBuf,
    pub config_path: PathBuf,

    pub selected_target: BuildTarget,
    pub selected_deploy: DeployMethod,
    pub chip_name: String,
    pub chip_erase: bool,
    pub verify_flash: bool,

    pub progress_percent: u8,
    pub is_busy: bool,
    pub status_msg: String,

    pub bus_frames: Vec<DecodedFrame>,
    pub bus_nodes: Vec<(String, String)>,
    pub traffic_filter: String,

    pub console_logs: Vec<String>,

    pub doc_topic: DocTopic,
    pub doc_search: String,

    pub dropped_file: Option<(String, usize)>,
}

impl BarebootApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let worker = WorkerHandles::spawn();

        let profile_dir = PathBuf::from("profiles");
        let config_path = PathBuf::from("configs/bareboot.toml");

        // Request initial profile list from worker
        let _ = worker.command_tx.send(WorkerCommand::LoadProfiles {
            dir: profile_dir.display().to_string(),
        });

        let active_profile = FlashProfile::default();
        let active_config = bareboot_engine::load_config(&config_path).unwrap_or(BarebootConfig {
            target: "stm32f103".to_string(),
            transport: "can".to_string(),
            boot_window_ms: 250,
        });

        let default_target = BuildTarget::BlinkTest;
        let default_chip = default_target.default_chip().to_string();

        let dock_layout = Self::default_dock_layout();

        Self {
            worker,
            dock_layout,
            active_tab: WorkbenchTab::Flasher,
            view_mode: ViewMode::Tabbed,

            active_profile,
            active_config,
            available_profiles: Vec::new(),
            profile_dir,
            config_path,

            selected_target: default_target,
            selected_deploy: DeployMethod::MemorySim,
            chip_name: default_chip,
            chip_erase: false,
            verify_flash: true,

            progress_percent: 0,
            is_busy: false,
            status_msg: "Ready".to_string(),

            bus_frames: Vec::new(),
            bus_nodes: Vec::new(),
            traffic_filter: String::new(),

            console_logs: vec!["[Workbench] BareBoot graphical workbench initialized.".to_string()],

            doc_topic: DocTopic::Readme,
            doc_search: String::new(),

            dropped_file: None,
        }
    }

    fn default_dock_layout() -> Layout<PanelId> {
        let left = Node::Tabs {
            panels: vec![PanelId::TargetConfig],
            active: 0,
        };
        let center_top = Node::Tabs {
            panels: vec![PanelId::MakeflashControl],
            active: 0,
        };
        let center_bottom = Node::Tabs {
            panels: vec![PanelId::BuildConsole],
            active: 0,
        };
        let center = Node::Split {
            axis: SplitAxis::Vertical,
            size: SplitSize::Ratio(0.48),
            first: Box::new(center_top),
            second: Box::new(center_bottom),
        };
        let right = Node::Tabs {
            panels: vec![PanelId::BusTraffic, PanelId::Documentation],
            active: 0,
        };
        let right_split = Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.60),
            first: Box::new(center),
            second: Box::new(right),
        };
        let root = Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.28),
            first: Box::new(left),
            second: Box::new(right_split),
        };

        Layout {
            root: Some(root),
            floating: Vec::new(),
        }
    }

    fn process_events(&mut self) {
        self.worker.poll();
        while let Ok(evt) = self.worker.event_rx.try_recv() {
            match evt {
                UiEvent::BusNode { name, uuid } => {
                    if !self.bus_nodes.iter().any(|(n, _)| n == &name) {
                        self.bus_nodes.push((name, uuid));
                    }
                }
                UiEvent::BusFrame(frame) => {
                    self.bus_frames.push(frame);
                    if self.bus_frames.len() > 500 {
                        self.bus_frames.remove(0);
                    }
                }
                UiEvent::Progress { percent } => {
                    self.progress_percent = percent;
                }
                UiEvent::Log { line } => {
                    self.console_logs.push(line);
                    if self.console_logs.len() > 2000 {
                        self.console_logs.remove(0);
                    }
                }
                UiEvent::Error { message } => {
                    self.status_msg = format!("Error: {message}");
                    self.is_busy = false;
                    self.console_logs.push(format!("[ERROR] {message}"));
                }
                UiEvent::Done { message } => {
                    self.status_msg = message.clone();
                    self.is_busy = false;
                    self.progress_percent = 100;
                    self.console_logs.push(format!("[DONE] {message}"));
                }
                UiEvent::ProfileList { names } => {
                    self.available_profiles = names;
                    if !self.available_profiles.is_empty() && self.active_profile.name == "default" {
                        if let Some(first) = self.available_profiles.first() {
                            let p_path = self.profile_dir.join(format!("{first}.toml"));
                            if let Ok(loaded) = FlashProfile::load(&p_path) {
                                self.active_profile = loaded;
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let path = file.path();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("firmware.bin")
                .to_string();

            if let Ok(data) = std::fs::read(path) {
                let size = data.len();
                self.console_logs.push(format!("[Upload] Received firmware image {name} ({size} bytes)"));
                self.status_msg = format!("Loaded image: {name} ({size} B)");

                let dest = PathBuf::from(&name);
                if let Ok(()) = std::fs::write(&dest, &data) {
                    self.active_profile.firmware = dest.display().to_string();
                }
                self.dropped_file = Some((name, size));
            }
        }
    }

    fn render_panel_body(&mut self, ui: &mut Ui, panel: &PanelId) {
        match panel {
            PanelId::TargetConfig => {
                let mut on_save = false;
                let mut on_reload = false;
                let mut on_delete = false;
                let mut on_reset_presets = false;

                render_config_panel(
                    ui,
                    &mut self.active_profile,
                    &mut self.active_config,
                    &self.available_profiles,
                    &mut on_save,
                    &mut on_reload,
                    &mut on_delete,
                    &mut on_reset_presets,
                    &self.status_msg,
                );

                if on_save {
                    let _ = self.worker.command_tx.send(WorkerCommand::SaveProfile {
                        dir: self.profile_dir.display().to_string(),
                        profile: self.active_profile.clone(),
                    });
                    let _ = bareboot_engine::save_config(&self.config_path, &self.active_config);
                    self.status_msg = format!("Profile '{}' saved", self.active_profile.name);
                    let _ = self.worker.command_tx.send(WorkerCommand::LoadProfiles {
                        dir: self.profile_dir.display().to_string(),
                    });
                }
                if on_reload {
                    let path = self.profile_dir.join(format!("{}.toml", self.active_profile.name));
                    if let Ok(loaded) = FlashProfile::load(&path) {
                        self.active_profile = loaded;
                        self.status_msg = format!("Loaded profile '{}'", self.active_profile.name);
                    }
                }
                if on_delete {
                    let _ = self.worker.command_tx.send(WorkerCommand::DeleteProfile {
                        dir: self.profile_dir.display().to_string(),
                        name: self.active_profile.name.clone(),
                    });
                    let _ = self.worker.command_tx.send(WorkerCommand::LoadProfiles {
                        dir: self.profile_dir.display().to_string(),
                    });
                }
                if on_reset_presets {
                    let _ = FlashProfile::ensure_presets(&self.profile_dir);
                    let _ = self.worker.command_tx.send(WorkerCommand::LoadProfiles {
                        dir: self.profile_dir.display().to_string(),
                    });
                    self.status_msg = "Profiles reset to factory presets".to_string();
                }
            }
            PanelId::MakeflashControl => {
                let mut actions = MakeflashActions::default();
                render_makeflash_panel(
                    ui,
                    &mut self.selected_target,
                    &mut self.selected_deploy,
                    &mut self.chip_name,
                    &mut self.chip_erase,
                    &mut self.verify_flash,
                    self.progress_percent,
                    self.is_busy,
                    &self.status_msg,
                    self.dropped_file.as_ref(),
                    &mut actions,
                );

                if actions.makeflash {
                    self.is_busy = true;
                    self.progress_percent = 0;
                    self.status_msg = "Makeflash pipeline starting...".to_string();
                    let _ = self.worker.command_tx.send(WorkerCommand::MakeFlash {
                        manifest_path: self.selected_target.manifest_path().to_string(),
                        release: true,
                        deploy_method: self.selected_deploy,
                        chip_erase: self.chip_erase,
                        hold_in_bootloader: false,
                        verify: self.verify_flash,
                        chip_name: self.chip_name.clone(),
                        profile: self.active_profile.clone(),
                    });
                } else if actions.build_only {
                    self.is_busy = true;
                    self.status_msg = "Cargo build starting...".to_string();
                    let _ = self.worker.command_tx.send(WorkerCommand::Build {
                        manifest_path: self.selected_target.manifest_path().to_string(),
                        release: true,
                        clean: false,
                    });
                } else if actions.flash_only {
                    self.is_busy = true;
                    self.progress_percent = 0;
                    self.status_msg = "Starting target flash...".to_string();
                    let _ = self.worker.command_tx.send(WorkerCommand::Flash {
                        profile: self.active_profile.clone(),
                    });
                } else if actions.reset_target {
                    self.is_busy = true;
                    self.status_msg = "Resetting target MCU...".to_string();
                    let _ = self.worker.command_tx.send(WorkerCommand::ResetTarget);
                } else if actions.scan_bus {
                    self.is_busy = true;
                    self.status_msg = "Scanning bus for nodes...".to_string();
                    let _ = self.worker.command_tx.send(WorkerCommand::ScanBus);
                }
            }
            PanelId::BuildConsole => {
                let mut on_clear = false;
                render_console_panel(ui, &self.console_logs, &mut on_clear);
                if on_clear {
                    self.console_logs.clear();
                }
            }
            PanelId::BusTraffic => {
                let mut on_clear = false;
                render_traffic_panel(
                    ui,
                    &self.bus_frames,
                    &self.bus_nodes,
                    &mut self.traffic_filter,
                    &mut on_clear,
                );
                if on_clear {
                    self.bus_frames.clear();
                }
            }
            PanelId::Documentation => {
                docs::render_doc_panel(ui, &mut self.doc_topic, &mut self.doc_search);
            }
        }
    }
}

impl eframe::App for BarebootApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.process_events();
        self.handle_drag_and_drop(&ctx);

        // Top status and banner bar
        Panel::top("top_header_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("BareBoot Workbench").strong().size(15.0));
                ui.separator();
                ui.label(format!("Profile: {}", self.active_profile.name));
                ui.separator();
                ui.label(format!("Backend: {}", self.active_profile.backend));
                ui.separator();
                ui.label(format!("Target: {}", self.selected_target.name()));

                ui.with_layout(EguiLayout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_busy {
                        ui.spinner();
                        ui.colored_label(Color32::from_rgb(255, 200, 100), "BUSY");
                    } else {
                        ui.colored_label(Color32::from_rgb(100, 255, 120), "READY");
                    }
                    ui.label(&self.status_msg);

                    ui.separator();
                    ui.selectable_value(&mut self.view_mode, ViewMode::Docked, "Docked");
                    ui.selectable_value(&mut self.view_mode, ViewMode::Tabbed, "Tabbed");
                    ui.label(RichText::new("Layout:").strong());
                });
            });
        });

        // Tabbed View (active by default across all orientations, with landscape responsive columns)
        if self.view_mode == ViewMode::Tabbed {
            Panel::top("workbench_tab_bar").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let tabs = [
                        (WorkbenchTab::Flasher, "Flasher"),
                        (WorkbenchTab::Config, "Target Config"),
                        (WorkbenchTab::Console, "Build Console"),
                        (WorkbenchTab::Traffic, "Bus Traffic"),
                        (WorkbenchTab::Docs, "Manuals"),
                    ];
                    for (tab, label) in tabs {
                        let active = self.active_tab == tab;
                        if ui.selectable_label(active, RichText::new(label).strong()).clicked() {
                            self.active_tab = tab;
                        }
                    }
                });
            });

            CentralPanel::default().show(ui, |ui| {
                match self.active_tab {
                    WorkbenchTab::Flasher => {
                        let width = ui.available_width();
                        if width >= 850.0 {
                            // Landscape dual-column view: Pipeline controls on the left, live build console on the right
                            ui.columns(2, |cols| {
                                self.render_panel_body(&mut cols[0], &PanelId::MakeflashControl);
                                self.render_panel_body(&mut cols[1], &PanelId::BuildConsole);
                            });
                        } else {
                            // Narrow portrait view: stacked with separator
                            self.render_panel_body(ui, &PanelId::MakeflashControl);
                            ui.separator();
                            self.render_panel_body(ui, &PanelId::BuildConsole);
                        }
                    }
                    WorkbenchTab::Config => {
                        self.render_panel_body(ui, &PanelId::TargetConfig);
                    }
                    WorkbenchTab::Console => {
                        self.render_panel_body(ui, &PanelId::BuildConsole);
                    }
                    WorkbenchTab::Traffic => {
                        self.render_panel_body(ui, &PanelId::BusTraffic);
                    }
                    WorkbenchTab::Docs => {
                        self.render_panel_body(ui, &PanelId::Documentation);
                    }
                }
            });
        } else {
            // Desktop dockable workspace layout with craft-ui
            CentralPanel::default().show(ui, |ui| {
                let dock_style = DockStyle::from_ui(ui);
                let dock_area = DockArea::new(Id::new("bareboot_workbench_dock"));

                let mut dock_layout = std::mem::replace(&mut self.dock_layout, Layout { root: None, floating: Vec::new() });
                let output = dock_area.show(
                    ui,
                    &dock_layout,
                    &dock_style,
                    |panel| panel.title().to_string(),
                    |_panel| Permissions::default(),
                    |ui, panel| {
                        self.render_panel_body(ui, panel);
                    },
                );

                // Apply any docking changes (drag, drop, split, reorder)
                for action in output.actions {
                    let _ = dock_layout.apply(action);
                }
                self.dock_layout = dock_layout;
            });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workbench_tabbed_view_modes() {
        assert_eq!(ViewMode::Tabbed, ViewMode::Tabbed);
        assert_ne!(ViewMode::Tabbed, ViewMode::Docked);

        let tabs = [
            WorkbenchTab::Flasher,
            WorkbenchTab::Config,
            WorkbenchTab::Console,
            WorkbenchTab::Traffic,
            WorkbenchTab::Docs,
        ];
        assert_eq!(tabs.len(), 5);
    }

    #[test]
    fn test_dock_layout_initialization() {
        let layout = BarebootApp::default_dock_layout();
        assert!(layout.root.is_some());
        assert!(layout.contains(&PanelId::TargetConfig));
        assert!(layout.contains(&PanelId::MakeflashControl));
        assert!(layout.contains(&PanelId::BuildConsole));
        assert!(layout.contains(&PanelId::BusTraffic));
        assert!(layout.contains(&PanelId::Documentation));
    }

    #[test]
    fn test_panel_titles_and_emojis() {
        let panels = [
            PanelId::TargetConfig,
            PanelId::MakeflashControl,
            PanelId::BuildConsole,
            PanelId::BusTraffic,
            PanelId::Documentation,
        ];
        for panel in panels {
            let title = panel.title();
            assert!(!title.is_empty());
            // Strictly check zero emoji: all chars should be ASCII
            assert!(title.is_ascii(), "Title '{title}' must be ASCII and emoji-free");
        }
    }

    #[test]
    fn test_doc_topics_content_embedded() {
        for topic in DocTopic::all() {
            let text = topic.content();
            assert!(!text.is_empty(), "Topic {:?} content is empty", topic);
            assert!(text.contains('#'), "Topic {:?} markdown should contain headers", topic);
        }
    }

    #[test]
    fn test_memory_sim_flashing_workflow() {
        // Prepare dummy firmware file
        let temp_bin = std::env::temp_dir().join("bareboot_test_sim.bin");
        std::fs::write(&temp_bin, vec![0xAA; 256]).unwrap();

        let worker = WorkerHandles::spawn();
        let profile = FlashProfile {
            name: "test-sim".to_string(),
            backend: "memory".to_string(),
            firmware: temp_bin.display().to_string(),
            tries: 2,
            timeout_secs: 1,
            verify: true,
            ..Default::default()
        };

        worker.command_tx.send(WorkerCommand::Flash { profile }).unwrap();

        let mut completed = false;
        let mut got_progress = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(5) {
            if let Ok(event) = worker.event_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                match event {
                    UiEvent::Progress { percent } => {
                        got_progress = true;
                        assert!(percent <= 100);
                    }
                    UiEvent::Done { message } => {
                        assert!(message.contains("flash complete"));
                        completed = true;
                        break;
                    }
                    UiEvent::Error { message } => {
                        panic!("Unexpected flashing error in test: {message}");
                    }
                    _ => {}
                }
            }
        }

        assert!(got_progress, "Did not receive progress event");
        assert!(completed, "Flashing did not complete within timeout");
        worker.shutdown();
        let _ = std::fs::remove_file(temp_bin);
    }
}
