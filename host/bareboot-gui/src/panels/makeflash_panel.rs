use egui::{Color32, ProgressBar, RichText, ScrollArea, Ui};
use bareboot_engine::build::{BuildTarget, DeployMethod};

#[derive(Default)]
pub struct MakeflashActions {
    pub makeflash: bool,
    pub build_only: bool,
    pub flash_only: bool,
    pub reset_target: bool,
    pub scan_bus: bool,
}

pub fn render_makeflash_panel(
    ui: &mut Ui,
    selected_target: &mut BuildTarget,
    selected_deploy: &mut DeployMethod,
    chip_name: &mut String,
    chip_erase: &mut bool,
    verify: &mut bool,
    progress_percent: u8,
    is_busy: bool,
    status_msg: &str,
    dropped_file: Option<&(String, usize)>,
    actions: &mut MakeflashActions,
) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(RichText::new("Makeflash Pipeline & Hardware Flasher").strong());
            ui.add_space(4.0);

            // Drag and drop binary zone
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Firmware Payload:").strong());
                    if let Some((name, size)) = dropped_file {
                        ui.label(
                            RichText::new(format!("{name} ({size} bytes)"))
                                .color(Color32::from_rgb(100, 220, 100))
                                .strong(),
                        );
                    } else {
                        ui.label(
                            RichText::new("Drag and drop .bin / .uf2 image here, or build from source")
                                .italics()
                                .color(Color32::from_rgb(160, 160, 160)),
                        );
                    }
                });
            });

            ui.add_space(6.0);

            // Target and Deployment selection
            ui.group(|ui| {
                egui::Grid::new("makeflash_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Target Project:");
                        egui::ComboBox::from_id_salt("target_project_combo")
                            .selected_text(selected_target.name())
                            .show_ui(ui, |ui| {
                                for t in BuildTarget::all() {
                                    if ui.selectable_value(selected_target, *t, t.name()).clicked() {
                                        *chip_name = t.default_chip().to_string();
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label("Deployment Transport:");
                        egui::ComboBox::from_id_salt("deploy_method_combo")
                            .selected_text(selected_deploy.name())
                            .show_ui(ui, |ui| {
                                for m in DeployMethod::all() {
                                    ui.selectable_value(selected_deploy, *m, m.name());
                                }
                            });
                        ui.end_row();

                        ui.label("Target MCU Chip Identifier:");
                        ui.text_edit_singleline(chip_name);
                        ui.end_row();

                        ui.label("Programming Flags:");
                        ui.horizontal(|ui| {
                            ui.checkbox(verify, "Verify Flash");
                            ui.checkbox(chip_erase, "Chip Erase");
                        });
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            // Progress bar and status
            ui.group(|ui| {
                ui.label(RichText::new("Execution Progress").strong());
                let progress_fraction = progress_percent as f32 / 100.0;
                ui.add(
                    ProgressBar::new(progress_fraction)
                        .show_percentage()
                        .animate(is_busy),
                );

                if !status_msg.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(status_msg).color(Color32::from_rgb(120, 200, 255)));
                }
            });

            ui.add_space(8.0);

            // Primary action buttons
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!is_busy, |ui| {
                    if ui.button(RichText::new("Makeflash (Build & Flash)").strong()).clicked() {
                        actions.makeflash = true;
                    }
                    if ui.button("Cargo Build Only").clicked() {
                        actions.build_only = true;
                    }
                    if ui.button("Flash Target Only").clicked() {
                        actions.flash_only = true;
                    }
                    if ui.button("Reset Target (Probe)").clicked() {
                        actions.reset_target = true;
                    }
                    if ui.button("Scan Bus").clicked() {
                        actions.scan_bus = true;
                    }
                });

                if is_busy {
                    ui.spinner();
                    ui.label(RichText::new("Operation in progress...").italics());
                }
            });
        });
}
