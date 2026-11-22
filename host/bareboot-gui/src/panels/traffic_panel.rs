use egui::{Color32, RichText, ScrollArea, Ui};
use bareboot_engine::protocol::{BusType, DecodedFrame, Direction};

pub fn render_traffic_panel(
    ui: &mut Ui,
    frames: &[DecodedFrame],
    nodes: &[(String, String)],
    filter: &mut String,
    on_clear: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Live Bus Traffic Sniffer & Packet Analyzer").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear Frames").clicked() {
                *on_clear = true;
            }
            ui.label(format!("Captured: {} frames", frames.len()));
        });
    });

    // Discovered nodes bar
    if !nodes.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Active Nodes:").strong().color(Color32::from_rgb(140, 200, 255)));
            for (name, uuid) in nodes {
                ui.label(
                    RichText::new(format!("[{name}: {uuid}]"))
                        .monospace()
                        .color(Color32::from_rgb(200, 230, 200)),
                );
            }
        });
        ui.separator();
    }

    ui.horizontal(|ui| {
        ui.label("Filter (Opcode / Node):");
        ui.text_edit_singleline(filter);
        if ui.button("Reset Filter").clicked() {
            filter.clear();
        }
    });

    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            egui::Grid::new("traffic_table_grid")
                .striped(true)
                .num_columns(7)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Time").strong());
                    ui.label(RichText::new("Dir").strong());
                    ui.label(RichText::new("Bus").strong());
                    ui.label(RichText::new("Node").strong());
                    ui.label(RichText::new("Opcode").strong());
                    ui.label(RichText::new("DLC").strong());
                    ui.label(RichText::new("Hex Payload / Details").strong());
                    ui.end_row();

                    let lower_filter = filter.trim().to_lowercase();
                    for frame in frames {
                        if !lower_filter.is_empty()
                            && !frame.opcode.to_lowercase().contains(&lower_filter)
                            && !frame.node_id.to_lowercase().contains(&lower_filter)
                        {
                            continue;
                        }

                        ui.label(RichText::new(&frame.timestamp).monospace());

                        let (dir_str, dir_color) = match frame.direction {
                            Direction::Tx => ("[TX]", Color32::from_rgb(100, 200, 255)),
                            Direction::Rx => ("[RX]", Color32::from_rgb(120, 255, 120)),
                        };
                        ui.label(RichText::new(dir_str).monospace().color(dir_color));

                        let bus_str = match frame.bus_type {
                            BusType::Can => "CAN",
                            BusType::Serial => "SERIAL",
                            BusType::I2c => "I2C",
                            BusType::Memory => "MEM",
                        };
                        ui.label(RichText::new(bus_str).monospace());

                        ui.label(RichText::new(&frame.node_id).monospace());

                        let op_color = if frame.is_error {
                            Color32::from_rgb(255, 100, 100)
                        } else if frame.is_ack {
                            Color32::from_rgb(100, 255, 150)
                        } else {
                            Color32::from_rgb(255, 220, 100)
                        };
                        ui.label(RichText::new(&frame.opcode).strong().color(op_color));

                        ui.label(RichText::new(format!("{}", frame.dlc)).monospace());

                        let detail_str = if frame.hex.is_empty() {
                            frame.detail.clone()
                        } else {
                            format!("{} ({})", frame.hex, frame.detail)
                        };
                        ui.label(RichText::new(detail_str).monospace());

                        ui.end_row();
                    }
                });
        });
}
