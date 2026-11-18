use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::theme::Palette;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetConfigState {
    pub device_uuid: String,
    pub target_mcu: String,
    pub connection_status: String,
    pub node_id: String,
    pub i2c_address: String,
    pub can_enabled: bool,
    pub uart_enabled: bool,
    pub usb_enabled: bool,
    pub i2c_enabled: bool,
    pub can_baud: u32,
    pub uart_baud: u32,
    pub boot_timeout_ms: u32,
    pub force_stay: bool,
    pub double_reset: bool,
    pub selected_field: usize,
    pub status_message: String,
}

impl Default for TargetConfigState {
    fn default() -> Self {
        Self {
            device_uuid: "0x1209:414d-6:00000001".to_string(),
            target_mcu: "STM32F103 (CanBoot! v0.1.0)".to_string(),
            connection_status: "CONNECTED [CAN / Serial]".to_string(),
            node_id: "0x100".to_string(),
            i2c_address: "0x30".to_string(),
            can_enabled: true,
            uart_enabled: true,
            usb_enabled: true,
            i2c_enabled: false,
            can_baud: 1_000_000,
            uart_baud: 115_200,
            boot_timeout_ms: 3000,
            force_stay: false,
            double_reset: true,
            selected_field: 0,
            status_message: "Ready to read/write device registers over wire".to_string(),
        }
    }
}

impl TargetConfigState {
    pub const TOTAL_FIELDS: usize = 10;

    pub fn next(&mut self) {
        self.selected_field = (self.selected_field + 1) % Self::TOTAL_FIELDS;
    }

    pub fn prev(&mut self) {
        self.selected_field = if self.selected_field == 0 {
            Self::TOTAL_FIELDS - 1
        } else {
            self.selected_field - 1
        };
    }

    pub fn toggle_selected(&mut self) {
        match self.selected_field {
            2 => self.can_enabled = !self.can_enabled,
            3 => self.uart_enabled = !self.uart_enabled,
            4 => self.usb_enabled = !self.usb_enabled,
            5 => self.i2c_enabled = !self.i2c_enabled,
            8 => self.force_stay = !self.force_stay,
            9 => self.double_reset = !self.double_reset,
            _ => {}
        }
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &TargetConfigState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Connected Target Hardware Card
            Constraint::Min(12),   // Device Bootloader Runtime Settings Form
            Constraint::Length(3), // Actions and Status
        ])
        .split(area);

    // 1. Connected Device Header
    let device_lines = vec![
        Line::from(vec![
            Span::styled("Target MCU: ", Style::default().fg(palette.text_muted)),
            Span::styled(&state.target_mcu, Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled("   UUID: ", Style::default().fg(palette.text_muted)),
            Span::styled(&state.device_uuid, Style::default().fg(palette.text)),
            Span::styled("   Link: ", Style::default().fg(palette.text_muted)),
            Span::styled(&state.connection_status, Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Protocol: ", Style::default().fg(palette.text_muted)),
            Span::styled("CanBoot/Katapult Wire Protocol v0.1.0  |  Storage: MCU Internal Flash Config Sector", Style::default().fg(palette.faint)),
        ]),
    ];

    let header_card = Paragraph::new(device_lines).block(
        Block::default()
            .title(" Target Device Identity (Post-Flash Connection) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(header_card, chunks[0]);

    // 2. Settings Grid (Left: Transports & Address, Right: Bus Timings & Boot Flags)
    let [left_col, right_col] = Layout::horizontal([
        Constraint::Percentage(52),
        Constraint::Percentage(48),
    ])
    .areas(chunks[1]);

    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();

    // Helper to format row
    let format_field = |idx: usize, label: &str, value: &str, hint: &str, sel: usize| -> Line<'static> {
        let is_sel = idx == sel;
        let prefix = if is_sel { "▶ " } else { "  " };
        let style_prefix = if is_sel {
            Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text_muted)
        };
        let style_val = if is_sel {
            Style::default().fg(palette.accent).bg(palette.raised).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };

        Line::from(vec![
            Span::styled(prefix, style_prefix),
            Span::styled(format!("{:<20} ", label), Style::default().fg(palette.text_muted)),
            Span::styled(value.to_string(), style_val),
            Span::styled(format!("  ({})", hint), Style::default().fg(palette.faint)),
        ])
    };

    // Field 0: CAN Node ID
    left_lines.push(format_field(0, "CAN Node ID / UUID", &state.node_id, "Enter to edit", state.selected_field));
    // Field 1: I2C Static Address
    left_lines.push(format_field(1, "I2C Static Address", &state.i2c_address, "Enter to edit", state.selected_field));
    // Field 2: CAN Transport
    left_lines.push(format_field(2, "CAN Bus Transport", if state.can_enabled { "[ ENABLED  ]" } else { "[ DISABLED ]" }, "Space toggle", state.selected_field));
    // Field 3: UART Transport
    left_lines.push(format_field(3, "UART Serial Transp", if state.uart_enabled { "[ ENABLED  ]" } else { "[ DISABLED ]" }, "Space toggle", state.selected_field));
    // Field 4: USB CDC Transport
    left_lines.push(format_field(4, "USB CDC Transport", if state.usb_enabled { "[ ENABLED  ]" } else { "[ DISABLED ]" }, "Space toggle", state.selected_field));
    // Field 5: I2C Transport
    left_lines.push(format_field(5, "I2C Bus Transport", if state.i2c_enabled { "[ ENABLED  ]" } else { "[ DISABLED ]" }, "Space toggle", state.selected_field));

    // Field 6: CAN Bitrate
    right_lines.push(format_field(6, "CAN Bitrate", &format!("{} bps", state.can_baud), "c cycle / Enter modal", state.selected_field));
    // Field 7: UART Baud
    right_lines.push(format_field(7, "UART Baud Rate", &format!("{} baud", state.uart_baud), "u cycle / Enter modal", state.selected_field));
    // Field 8: Boot Timeout
    right_lines.push(format_field(8, "Boot Auto-Jump", &format!("{} ms", state.boot_timeout_ms), "Enter to edit", state.selected_field));
    // Field 9: Double Reset Window
    right_lines.push(format_field(9, "Double Reset Entry", if state.double_reset { "[ ENABLED  ]" } else { "[ DISABLED ]" }, "Space toggle", state.selected_field));

    right_lines.push(Line::from(""));
    right_lines.push(Line::from(vec![
        Span::styled("Hold in Bootloader: ", Style::default().fg(palette.text_muted)),
        Span::styled(
            if state.force_stay { "STAY FOREVER (REQUEST_BOOTLOADER)" } else { "AUTO-JUMP TO USER APP" },
            Style::default().fg(if state.force_stay { palette.warn } else { palette.ok }).add_modifier(Modifier::BOLD),
        ),
    ]));

    let left_block = Paragraph::new(left_lines).block(
        Block::default()
            .title(" Transports & Addresses ([↑/↓/Wheel] Select, [Enter] Modal, [Space] Toggle) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(left_block, left_col);

    let right_block = Paragraph::new(right_lines).block(
        Block::default()
            .title(" Bus Frequencies & Boot Parameters ")
            .title_style(Style::default().fg(palette.accent))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(right_block, right_col);

    // 3. Actions & Status Bar
    let status_line = Line::from(vec![
        Span::styled("Actions: ", Style::default().fg(palette.text_muted)),
        Span::styled("[r] Read from Device   ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        Span::styled("[w] Write to MCU Flash   ", Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
        Span::styled("[j] Jump to App   ", Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        Span::styled("[b] Reboot to BL   ", Style::default().fg(palette.warn).add_modifier(Modifier::BOLD)),
        Span::styled("Status: ", Style::default().fg(palette.text_muted)),
        Span::styled(&state.status_message, Style::default().fg(palette.accent)),
    ]);

    let status_block = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(status_block, chunks[2]);
}
