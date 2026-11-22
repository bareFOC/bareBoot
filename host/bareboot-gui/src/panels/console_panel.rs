use egui::{Color32, RichText, ScrollArea, Ui};

pub fn render_console_panel(ui: &mut Ui, logs: &[String], on_clear: &mut bool) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Cargo Build & Diagnostics Console").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear Console").clicked() {
                *on_clear = true;
            }
            ui.label(format!("Total entries: {}", logs.len()));
        });
    });

    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if logs.is_empty() {
                ui.label(RichText::new("No build or execution output yet.").italics().color(Color32::from_rgb(140, 140, 140)));
            } else {
                for line in logs {
                    let text = line.trim_end();
                    let color = if text.contains("error:") || text.contains("Error") || text.contains("failed") {
                        Color32::from_rgb(255, 100, 100)
                    } else if text.contains("warning:") || text.contains("Warning") {
                        Color32::from_rgb(255, 200, 80)
                    } else if text.contains("completed") || text.contains("success") || text.contains("complete") {
                        Color32::from_rgb(100, 220, 120)
                    } else if text.starts_with("[Cargo]") {
                        Color32::from_rgb(140, 180, 255)
                    } else if text.starts_with("[Makeflash]") || text.starts_with("[Probe-RS]") {
                        Color32::from_rgb(200, 160, 255)
                    } else {
                        Color32::from_rgb(220, 220, 220)
                    };

                    ui.label(RichText::new(text).monospace().color(color));
                }
            }
        });
}
