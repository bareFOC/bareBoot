use egui::{Color32, RichText, ScrollArea, Ui};
use bareboot_engine::profile::FlashProfile;
use bareboot_client::BarebootConfig;

pub fn render_config_panel(
    ui: &mut Ui,
    profile: &mut FlashProfile,
    config: &mut BarebootConfig,
    available_profiles: &[String],
    on_save: &mut bool,
    on_reload: &mut bool,
    on_delete: &mut bool,
    on_reset_presets: &mut bool,
    status_msg: &str,
) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(RichText::new("Target Configuration & Profile").strong());
            ui.add_space(6.0);

            if !status_msg.is_empty() {
                ui.label(RichText::new(status_msg).color(Color32::from_rgb(100, 200, 100)));
                ui.add_space(4.0);
            }

            // Profile Selector
            ui.group(|ui| {
                ui.label(RichText::new("Saved Deployment Profiles").strong());
                ui.horizontal_wrapped(|ui| {
                    for name in available_profiles {
                        let is_active = profile.name == *name;
                        if ui.selectable_label(is_active, name).clicked() {
                            profile.name = name.clone();
                            *on_reload = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("Save Profile").clicked() {
                        *on_save = true;
                    }
                    if ui.button("Reload Profile").clicked() {
                        *on_reload = true;
                    }
                    if ui.button("Delete Profile").clicked() {
                        *on_delete = true;
                    }
                    if ui.button("Reset Presets").clicked() {
                        *on_reset_presets = true;
                    }
                });
            });

            ui.add_space(8.0);

            // Core Hardware Parameters
            ui.group(|ui| {
                ui.label(RichText::new("Hardware Parameters").strong());
                egui::Grid::new("hw_config_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Profile Name:");
                        ui.text_edit_singleline(&mut profile.name);
                        ui.end_row();

                        ui.label("MCU Family:");
                        egui::ComboBox::from_id_salt("mcu_family_combo")
                            .selected_text(&profile.mcu_family)
                            .show_ui(ui, |ui| {
                                for family in ["STM32F103", "STM32G0B1", "STM32H503", "CH32V203-USB", "RP2350"] {
                                    ui.selectable_value(&mut profile.mcu_family, family.to_string(), family);
                                }
                            });
                        ui.end_row();

                        ui.label("Deployment Backend:");
                        egui::ComboBox::from_id_salt("backend_combo")
                            .selected_text(&profile.backend)
                            .show_ui(ui, |ui| {
                                for b in ["memory", "serial", "can", "i2c", "android_usb", "web_serial"] {
                                    ui.selectable_value(&mut profile.backend, b.to_string(), b);
                                }
                            });
                        ui.end_row();

                        ui.label("Firmware Artifact Path:");
                        ui.text_edit_singleline(&mut profile.firmware);
                        ui.end_row();

                        ui.label("Application Flash Offset:");
                        ui.text_edit_singleline(&mut profile.app_offset);
                        ui.end_row();

                        ui.label("Status LED Pin:");
                        ui.text_edit_singleline(&mut profile.led_pin);
                        ui.end_row();

                        ui.label("Boot Window (ms):");
                        let mut bw_str = config.boot_window_ms.to_string();
                        if ui.text_edit_singleline(&mut bw_str).changed() {
                            if let Ok(val) = bw_str.parse::<u32>() {
                                config.boot_window_ms = val;
                            }
                        }
                        if config.boot_window_ms < 50 {
                            ui.colored_label(Color32::from_rgb(255, 100, 100), "Invalid: min 50 ms");
                        }
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            // Bus Interface Parameters
            ui.group(|ui| {
                ui.label(RichText::new("Bus Interface Settings").strong());
                egui::Grid::new("bus_interface_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        // Serial
                        ui.label("Serial Device:");
                        ui.text_edit_singleline(&mut profile.serial_device);
                        ui.end_row();

                        ui.label("UART Baud Rate:");
                        egui::ComboBox::from_id_salt("baud_combo")
                            .selected_text(profile.baud.to_string())
                            .show_ui(ui, |ui| {
                                for baud in [9600, 19200, 57600, 115200, 230400, 460800, 921600, 1000000] {
                                    ui.selectable_value(&mut profile.baud, baud, baud.to_string());
                                }
                            });
                        ui.end_row();

                        ui.label("UART Pins:");
                        ui.text_edit_singleline(&mut profile.uart_pins);
                        ui.end_row();

                        // CAN
                        ui.label("CAN Interface:");
                        ui.text_edit_singleline(&mut profile.can_interface);
                        ui.end_row();

                        ui.label("CAN Arbitration ID:");
                        ui.text_edit_singleline(&mut profile.node_id);
                        ui.end_row();

                        ui.label("CAN Pins:");
                        ui.text_edit_singleline(&mut profile.can_pins);
                        ui.end_row();

                        // I2C
                        ui.label("I2C Device:");
                        ui.text_edit_singleline(&mut profile.i2c_device);
                        ui.end_row();

                        ui.label("I2C Address:");
                        ui.text_edit_singleline(&mut profile.i2c_address);
                        ui.end_row();

                        // Flashing options
                        ui.label("Retry Tries:");
                        let mut tries_str = profile.tries.to_string();
                        if ui.text_edit_singleline(&mut tries_str).changed() {
                            if let Ok(v) = tries_str.parse::<usize>() {
                                profile.tries = v;
                            }
                        }
                        ui.end_row();

                        ui.label("Timeout (seconds):");
                        let mut to_str = profile.timeout_secs.to_string();
                        if ui.text_edit_singleline(&mut to_str).changed() {
                            if let Ok(v) = to_str.parse::<u64>() {
                                profile.timeout_secs = v;
                            }
                        }
                        ui.end_row();

                        ui.label("Verify After Flash:");
                        ui.checkbox(&mut profile.verify, "Enabled");
                        ui.end_row();
                    });
            });
        });
}
