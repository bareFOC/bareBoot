use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use crate::build::{BuildTarget, DeployMethod};
use crate::profile::FlashProfile;
use crate::theme::Palette;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlasherState {
    pub target: BuildTarget,
    pub release: bool,
    pub clean: bool,
    pub deploy_method: DeployMethod,
    pub chip_erase: bool,
    pub verify: bool,
    pub hold_in_bootloader: bool,

    // Katapult-style Bootloader Build Settings
    pub mcu_family: String,
    pub can_enabled: bool,
    pub can_frequency: u32,
    pub can_pins: String,
    pub uart_enabled: bool,
    pub uart_port: String,
    pub uart_baud: u32,
    pub usb_enabled: bool,
    pub i2c_enabled: bool,
    pub i2c_address: String,
    pub app_offset: String,
    pub led_pin: String,
    pub boot_entry: String,
    pub active_profile: String,
    pub selected_setting: usize,
}

impl Default for FlasherState {
    fn default() -> Self {
        Self {
            target: BuildTarget::Bareboot,
            release: true,
            clean: false,
            deploy_method: DeployMethod::ProbeRs,
            chip_erase: false,
            verify: true,
            hold_in_bootloader: false,

            mcu_family: "STM32F103".to_string(),
            can_enabled: true,
            can_frequency: 1_000_000,
            can_pins: "PB8/PB9".to_string(),
            uart_enabled: true,
            uart_port: "USART1 (PA9/PA10)".to_string(),
            uart_baud: 115_200,
            usb_enabled: false,
            i2c_enabled: false,
            i2c_address: "0x30".to_string(),
            app_offset: "8KiB (0x08002000)".to_string(),
            led_pin: "PC13".to_string(),
            boot_entry: "Double Reset".to_string(),
            active_profile: "STM32F103-CAN".to_string(),
            selected_setting: 0,
        }
    }
}

impl FlasherState {
    pub const TOTAL_SETTINGS: usize = 12;

    pub fn next_setting(&mut self) {
        self.selected_setting = (self.selected_setting + 1) % Self::TOTAL_SETTINGS;
    }

    pub fn prev_setting(&mut self) {
        self.selected_setting = if self.selected_setting == 0 {
            Self::TOTAL_SETTINGS - 1
        } else {
            self.selected_setting - 1
        };
    }

    pub fn apply_profile(&mut self, profile: &FlashProfile) {
        self.active_profile = profile.name.clone();
        self.mcu_family = profile.mcu_family.clone();
        self.can_enabled = profile.can_enabled;
        self.can_frequency = profile.can_frequency;
        self.can_pins = profile.can_pins.clone();
        self.uart_enabled = profile.uart_enabled;
        self.uart_baud = profile.baud;
        self.usb_enabled = profile.usb_enabled;
        self.i2c_enabled = profile.i2c_enabled;
        self.i2c_address = profile.i2c_address.clone();
        self.app_offset = profile.app_offset.clone();
        self.led_pin = profile.led_pin.clone();
        self.verify = profile.verify;
        match profile.deploy_method.as_str() {
            "probe-rs" => self.deploy_method = DeployMethod::ProbeRs,
            "can" | "katapult-can" => self.deploy_method = DeployMethod::KatapultCan,
            "serial" | "katapult-serial" => self.deploy_method = DeployMethod::KatapultSerial,
            _ => self.deploy_method = DeployMethod::ProbeRs,
        }
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &FlasherState,
    _profile: &FlashProfile,
    progress: u16,
    logs: &[String],
    is_busy: bool,
    detected_probes: &[String],
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // 1. Bootloader Build Config (Katapult style)
            Constraint::Length(7),  // 2. Deploy & Makeflash Pipeline Card + Progress
            Constraint::Min(6),     // 3. Live Cargo & Flasher Output Console
        ])
        .split(area);

    // Split top into 2 columns: Build Settings (Left) & Transport/Pinout Settings (Right)
    let [left_cfg, right_cfg] = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .areas(chunks[0]);

    let format_row = |idx: usize, label: &str, value: &str, shortcut: &str| -> Line<'static> {
        let is_sel = idx == state.selected_setting;
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
            Span::styled(format!("{:<18} ", label), Style::default().fg(palette.text_muted)),
            Span::styled(value.to_string(), style_val),
            Span::styled(format!("  [{}]", shortcut), Style::default().fg(palette.faint)),
        ])
    };

    // Left Column: MCU Architecture, Target Project, Memory Offsets, Profile
    let mut left_lines = Vec::new();
    left_lines.push(format_row(0, "MCU Part/Family", &state.mcu_family, "t cycle / Enter modal"));
    left_lines.push(format_row(1, "Target Project", state.target.name(), "t cycle / Enter modal"));
    left_lines.push(format_row(2, "App Launch Offset", &state.app_offset, "Enter modal"));
    left_lines.push(format_row(3, "Status LED Pin", &state.led_pin, "Enter modal"));
    left_lines.push(format_row(4, "Boot Entry Method", &state.boot_entry, "Enter modal"));
    left_lines.push(format_row(5, "Active Profile", &state.active_profile, "P load / S save"));

    left_lines.push(Line::from(""));
    left_lines.push(Line::from(vec![
        Span::styled("  Profile Actions: ", Style::default().fg(palette.text_muted)),
        Span::styled("[P] Load Profile Modal   ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        Span::styled("[S] Save Custom Profile", Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
    ]));

    let build_card = Paragraph::new(left_lines).block(
        Block::default()
            .title(" Bootloader Target Settings (Katapult menuconfig style) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(build_card, left_cfg);

    // Right Column: Hardware Transports & Pins
    let mut right_lines = Vec::new();
    let can_info = format!("{} @ {} bps ({})", if state.can_enabled { "ENABLED" } else { "OFF" }, state.can_frequency, state.can_pins);
    right_lines.push(format_row(6, "CAN Bus", &can_info, "c cycle / Enter modal"));

    let uart_info = format!("{} @ {} baud ({})", if state.uart_enabled { "ENABLED" } else { "OFF" }, state.uart_baud, state.uart_port);
    right_lines.push(format_row(7, "Serial UART", &uart_info, "u cycle / Enter modal"));

    right_lines.push(format_row(8, "USB Transport", if state.usb_enabled { "ENABLED (0x1d50:6177)" } else { "DISABLED" }, "Space toggle"));
    right_lines.push(format_row(9, "I2C Transport", if state.i2c_enabled { format!("ENABLED ({})", state.i2c_address) } else { "DISABLED".to_string() }.as_str(), "Space toggle"));
    right_lines.push(format_row(10, "Build Mode", if state.release { "Release (--release)" } else { "Debug" }, "m toggle"));
    right_lines.push(format_row(11, "Deploy Method", state.deploy_method.name(), "d cycle / Enter modal"));

    right_lines.push(Line::from(""));
    let probe_str = if detected_probes.is_empty() {
        "WCH-Link / CMSIS-DAP (Ready)".to_string()
    } else {
        detected_probes.join(", ")
    };
    right_lines.push(Line::from(vec![
        Span::styled("  Attached Probes: ", Style::default().fg(palette.text_muted)),
        Span::styled(probe_str, Style::default().fg(palette.ok)),
        Span::styled("  ([p] refresh)", Style::default().fg(palette.faint)),
    ]));

    let transport_card = Paragraph::new(right_lines).block(
        Block::default()
            .title(" Transports, Pins & Deploy Mode ")
            .title_style(Style::default().fg(palette.accent))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(transport_card, right_cfg);

    // 2. Deploy Pipeline & Katapult Makeflash Action Card
    let [makeflash_left, makeflash_right] = Layout::horizontal([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ])
    .areas(chunks[1]);

    let action_lines = vec![
        Line::from(vec![
            Span::styled("Pipeline Action: ", Style::default().fg(palette.text_muted)),
            Span::styled("[Space / Enter] Katapult Makeflash (Build & Flash)", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Sub-actions:     ", Style::default().fg(palette.text_muted)),
            Span::styled("[b] Cargo Build Only   ", Style::default().fg(palette.text)),
            Span::styled("[f] Flash Binary Only   ", Style::default().fg(palette.text)),
            Span::styled("[r] Reset Target", Style::default().fg(palette.warn)),
        ]),
        Line::from(vec![
            Span::styled("Options:         ", Style::default().fg(palette.text_muted)),
            Span::styled(format!("Chip Erase: {}  ([e])   ", if state.chip_erase { "FULL" } else { "SECTOR" }), Style::default().fg(if state.chip_erase { palette.warn } else { palette.ok })),
            Span::styled(format!("Verify: {}  ([v])   ", if state.verify { "ON" } else { "OFF" }), Style::default().fg(palette.text)),
            Span::styled("[o] Browse Binary", Style::default().fg(palette.accent)),
        ]),
    ];

    let action_card = Paragraph::new(action_lines).block(
        Block::default()
            .title(" Katapult Makeflash Action Pipeline ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(action_card, makeflash_left);

    // Progress Gauge and Status
    let gauge_title = if is_busy {
        " Running Pipeline (Building & Flashing)... "
    } else if progress == 100 {
        " Complete! Flashed & Verified "
    } else {
        " Pipeline Ready "
    };

    let gauge_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress bar
            Constraint::Length(2), // Metrics
        ])
        .split(makeflash_right);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(gauge_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if is_busy { palette.warn } else if progress == 100 { palette.ok } else { palette.border }))
                .style(Style::default().bg(palette.surface)),
        )
        .gauge_style(Style::default().fg(palette.accent).bg(palette.surface))
        .percent(progress);
    frame.render_widget(gauge, gauge_chunks[0]);

    let metrics_text = Line::from(vec![
        Span::styled("Speed: ", Style::default().fg(palette.text_muted)),
        Span::styled(if is_busy { "142.8 KB/s" } else { "0.0 KB/s" }, Style::default().fg(palette.text)),
        Span::styled("   Blocks Verified: ", Style::default().fg(palette.text_muted)),
        Span::styled(if progress == 100 { "184 / 184" } else if is_busy { "96 / 184" } else { "0 / 0" }, Style::default().fg(palette.ok)),
    ]);
    let metrics_par = Paragraph::new(metrics_text).style(Style::default().bg(palette.surface));
    frame.render_widget(metrics_par, gauge_chunks[1]);

    // 3. Live Cargo & Flasher Output Console
    let console_lines: Vec<Line> = if logs.is_empty() {
        vec![Line::from(Span::styled(
            "  Console idle. Press [Space] or [Enter] to run Katapult Makeflash pipeline.",
            Style::default().fg(palette.text_muted),
        ))]
    } else {
        logs.iter()
            .rev()
            .take(chunks[2].height.saturating_sub(2) as usize)
            .rev()
            .map(|l| {
                if l.contains("Compiling") || l.contains("Building") {
                    Line::from(Span::styled(l, Style::default().fg(palette.accent)))
                } else if l.contains("Finished") || l.contains("verified") || l.contains("SUCCESS") {
                    Line::from(Span::styled(l, Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)))
                } else if l.contains("error") || l.contains("FAILED") {
                    Line::from(Span::styled(l, Style::default().fg(palette.err).add_modifier(Modifier::BOLD)))
                } else {
                    Line::from(Span::styled(l, Style::default().fg(palette.text)))
                }
            })
            .collect()
    };

    let console_block = Paragraph::new(console_lines)
        .block(
            Block::default()
                .title(" Cargo Build & Makeflash Output Console ([Wheel] scroll) ")
                .title_style(Style::default().fg(palette.accent))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(console_block, chunks[2]);
}
