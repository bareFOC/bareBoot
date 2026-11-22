use egui::{Color32, RichText, ScrollArea, TextStyle, Ui};

pub const DOC_README: &str = include_str!("../../../README.md");
pub const DOC_HANDSFREE: &str = include_str!("../../../docs/handsfree-app-updates.md");

pub const DOC_PROTOCOL: &str = r#"# BareBoot Katapult Protocol Reference

## Frame Framing
BareBoot Katapult communication uses packet framing with a 2-byte header, opcode byte, length byte, variable payload, and a 4-byte CRC32 (Ethernet polynomial 0xEDB88320 / standard IEEE 802.3):

```
+------------+------------+--------+--------+------------------+------------+
| Header[0]  | Header[1]  | Opcode | Length | Payload (0..N B) | CRC32 (4B) |
| 0x01       | 0x88       | (u8)   | (u8)   | Data Bytes       | Little-End |
+------------+------------+--------+--------+------------------+------------+
```

## Protocol Opcodes

| Opcode | Hex  | Name          | Direction | Description                                  |
|--------|------|---------------|-----------|----------------------------------------------|
| 0x11   | 0x11 | CONNECT       | Host->MCU | Handshake, queries MCU geometry & parameters |
| 0x12   | 0x12 | RX_BLOCK      | Host->MCU | Streams a chunk of firmware data into RAM/NOR|
| 0x13   | 0x13 | RX_EOF        | Host->MCU | Signals end of binary firmware image         |
| 0x14   | 0x14 | REQ_BLOCK     | Host->MCU | Requests block checksum verification         |
| 0x15   | 0x15 | COMPLETE      | Host->MCU | Finalize, commit journal, jump to application|
| 0x16   | 0x16 | GET_CANBUS_ID | Host->MCU | Query node assigned CAN arbitration ID       |
| 0xA0   | 0xA0 | ACK           | MCU->Host | Command acknowledged successfully            |
| 0xF1   | 0xF1 | NACK          | MCU->Host | Command rejected or checksum mismatch        |
| 0xF2   | 0xF2 | COMMAND_ERROR | MCU->Host | Hard error, verify failed or flash locked    |
| 0xF3   | 0xF3 | COMMAND_BUSY  | MCU->Host | Sector erase or write in progress            |
| 0xF0   | 0xF0 | I2C_ENTER_BL  | Host->MCU | Software trigger to force bootloader stay    |
| 0x02   | 0x02 | CAN_REBOOT    | Host->MCU | Remote reboot command                        |
"#;

pub const DOC_PINOUTS: &str = r#"# Target Pinouts & Flash Layouts

## Supported Microcontrollers

| Target Family | Flash Base  | Bootloader Size | App Slot Offset | Default Interface | Pins      |
|---------------|-------------|-----------------|-----------------|-------------------|-----------|
| STM32F103C8T6 | 0x08000000  | 8 KiB           | 0x08002000      | CAN (1 Mbps)      | PB8 / PB9 |
| STM32G0B1CB   | 0x08000000  | 8 KiB           | 0x08002000      | CAN-FD / CAN      | PB0 / PB1 |
| STM32H503RB   | 0x08000000  | 16 KiB          | 0x08004000      | I2C (FastMode+)   | PB11/PB12 |
| CH32V203C8T6  | 0x08000000  | 8 KiB           | 0x08002000      | USB-FS CDC        | PA11/PA12 |
| RP2350 (ARM)  | 0x10000000  | 288 KiB         | 0x10048000      | USB CDC / UART    | GP0 / GP1 |

## Boot Window Timings
On hardware reset or power-on:
- Bootloader listens for `CONNECT` packet for configured `boot_window_ms` (default 300 ms, min 50 ms).
- If valid `CONNECT` packet is received, bootloader locks in programming mode.
- If no frame arrives before timeout expires, bootloader verifies app vector table and jumps to application.
"#;

pub const DOC_ANDROID_GUIDE: &str = r#"# Android USB OTG Field Flashing Guide

## Requirements
- Android 8.0 (API Level 26) or newer with USB Host / OTG support.
- USB OTG cable or USB-C to USB-A adapter.
- Supported target connected via USB CDC or USB-to-CAN adapter (CandleLight, GS_USB, SLCAN).

## Procedure
1. Plug the target MCU or USB-CAN dongle into the Android device.
2. When prompted by Android system dialog "Allow BareBoot to access USB device?", tap "Always allow" or "OK".
3. Open the BareBoot Workbench.
4. Select the target profile or import your compiled `.bin` or `.uf2` file via the Storage Access Framework picker.
5. Tap "Flash Target" to start programming over the native USB bulk endpoints. No root required.
"#;

pub const DOC_WEB_GUIDE: &str = r#"# WebAssembly & WebSerial Flashing Guide

## Browser Compatibility
WebSerial is supported out-of-the-box in Chromium-based browsers:
- Google Chrome (Desktop and Android)
- Microsoft Edge
- Opera
- Brave

> Note: WebSerial requires a Secure Context (HTTPS or localhost). GitHub Pages provides HTTPS by default.

## Browser Flashing Steps
1. Navigate to the BareBoot Web Workbench URL.
2. In the Makeflash / Flasher panel, choose "WebSerial" as the backend.
3. Click "Connect Serial Device". The browser displays the port selection prompt.
4. Select your USB CDC bootloader or USB-UART adapter and click "Connect".
5. Drag and drop your firmware `.bin` or `.uf2` file directly onto the workbench or click "Select Binary".
6. Click "Flash Target" to execute programming over the browser stream.
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocTopic {
    Readme,
    Handsfree,
    Protocol,
    Pinouts,
    AndroidGuide,
    WebGuide,
}

impl DocTopic {
    pub fn all() -> &'static [DocTopic] {
        &[
            DocTopic::Readme,
            DocTopic::Handsfree,
            DocTopic::Protocol,
            DocTopic::Pinouts,
            DocTopic::AndroidGuide,
            DocTopic::WebGuide,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            DocTopic::Readme => "Overview & Getting Started",
            DocTopic::Handsfree => "Handsfree App Updates Architecture",
            DocTopic::Protocol => "Katapult Protocol & Opcodes",
            DocTopic::Pinouts => "Target Pinouts & Flash Layouts",
            DocTopic::AndroidGuide => "Android USB OTG Field Guide",
            DocTopic::WebGuide => "WebAssembly & WebSerial Guide",
        }
    }

    pub fn content(&self) -> &'static str {
        match self {
            DocTopic::Readme => DOC_README,
            DocTopic::Handsfree => DOC_HANDSFREE,
            DocTopic::Protocol => DOC_PROTOCOL,
            DocTopic::Pinouts => DOC_PINOUTS,
            DocTopic::AndroidGuide => DOC_ANDROID_GUIDE,
            DocTopic::WebGuide => DOC_WEB_GUIDE,
        }
    }
}

pub fn render_markdown(ui: &mut Ui, text: &str, filter: &str) {
    let lower_filter = filter.trim().to_lowercase();
    let lines: Vec<&str> = text.lines().collect();

    let mut in_code_block = false;
    let mut code_buffer = String::new();

    for line in lines {
        if line.starts_with("```") {
            if in_code_block {
                // Flush code block
                ui.add(
                    egui::Label::new(
                        RichText::new(&code_buffer)
                            .monospace()
                            .color(Color32::from_rgb(180, 220, 180)),
                    )
                    .wrap(),
                );
                code_buffer.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_buffer.push_str(line);
            code_buffer.push('\n');
            continue;
        }

        if !lower_filter.is_empty() && !line.to_lowercase().contains(&lower_filter) {
            continue;
        }

        if line.starts_with("# ") {
            ui.add_space(8.0);
            ui.heading(RichText::new(&line[2..]).strong().color(Color32::from_rgb(240, 240, 240)));
            ui.add_space(4.0);
        } else if line.starts_with("## ") {
            ui.add_space(6.0);
            ui.label(RichText::new(&line[3..]).text_style(TextStyle::Heading).strong().color(Color32::from_rgb(200, 220, 255)));
            ui.add_space(2.0);
        } else if line.starts_with("### ") {
            ui.add_space(4.0);
            ui.label(RichText::new(&line[4..]).strong().color(Color32::from_rgb(255, 215, 0)));
        } else if line.starts_with("- ") || line.starts_with("* ") {
            ui.horizontal(|ui| {
                ui.label(RichText::new("*").color(Color32::from_rgb(150, 150, 150)));
                ui.label(&line[2..]);
            });
        } else if line.starts_with("|") {
            ui.label(RichText::new(line).monospace().color(Color32::from_rgb(210, 210, 210)));
        } else if !line.trim().is_empty() {
            ui.label(line);
        }
    }

    if in_code_block && !code_buffer.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(&code_buffer)
                    .monospace()
                    .color(Color32::from_rgb(180, 220, 180)),
            )
            .wrap(),
        );
    }
}

pub fn render_doc_panel(
    ui: &mut Ui,
    selected_topic: &mut DocTopic,
    search_query: &mut String,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Documentation Search:").strong());
        ui.text_edit_singleline(search_query);
        if ui.button("Clear").clicked() {
            search_query.clear();
        }
    });

    ui.separator();

    ui.horizontal(|ui| {
        for topic in DocTopic::all() {
            let is_selected = *selected_topic == *topic;
            let btn = ui.selectable_label(is_selected, topic.title());
            if btn.clicked() {
                *selected_topic = *topic;
            }
        }
    });

    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_markdown(ui, selected_topic.content(), search_query);
        });
}
