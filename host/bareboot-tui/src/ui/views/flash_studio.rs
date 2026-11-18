use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use crate::profile::FlashProfile;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    profile: &FlashProfile,
    progress: u16,
    logs: &[String],
    is_flashing: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Target & Firmware info
            Constraint::Length(5), // Progress & Telemetry Gauge
            Constraint::Length(6), // Backend & Parameters Card
            Constraint::Min(4),    // Flash Log output
        ])
        .split(area);

    // 1. Target & Firmware Card
    let fw_display = if profile.firmware.is_empty() {
        "(No firmware binary selected - press 'f' to browse)".to_string()
    } else {
        profile.firmware.clone()
    };

    let fw_lines = vec![
        Line::from(vec![
            Span::styled("Firmware: ", Style::default().fg(palette.text_muted)),
            Span::styled(fw_display, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("App Base: ", Style::default().fg(palette.text_muted)),
            Span::styled("0x08002000  ", Style::default().fg(palette.accent)),
            Span::styled("Verify: ", Style::default().fg(palette.text_muted)),
            Span::styled(
                if profile.verify { "ENABLED" } else { "DISABLED" },
                Style::default().fg(if profile.verify { palette.ok } else { palette.warn }),
            ),
            Span::styled("  Max Retries: ", Style::default().fg(palette.text_muted)),
            Span::styled(profile.tries.to_string(), Style::default().fg(palette.text)),
        ]),
    ];

    let fw_block = Paragraph::new(fw_lines).block(
        Block::default()
            .title(" Firmware & Target ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(fw_block, chunks[0]);

    // 2. Progress & Telemetry Gauge
    let gauge_title = if is_flashing {
        format!(" Flashing in progress... {progress}% ")
    } else if progress >= 100 {
        " Flash Complete (100% Verified) ".to_string()
    } else {
        " Ready to Flash - Press [Space] to start ".to_string()
    };

    let gauge_color = if progress >= 100 {
        palette.ok
    } else if is_flashing {
        palette.accent
    } else {
        palette.text_muted
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(gauge_title)
                .title_style(Style::default().fg(gauge_color).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        )
        .gauge_style(Style::default().fg(gauge_color).bg(palette.bg))
        .percent(progress);
    frame.render_widget(gauge, chunks[1]);

    // 3. Backend & Parameters Card
    let backend_detail = match profile.backend.as_str() {
        "can" => format!(
            "SocketCAN │ Interface: {} │ Node ID: {} │ Bitrate: 1 Mbps",
            if profile.can_interface.is_empty() { "can0" } else { &profile.can_interface },
            profile.node_id
        ),
        "serial" => format!(
            "UART/Serial │ Device: {} │ Baud: {}",
            if profile.serial_device.is_empty() { "/dev/ttyUSB0" } else { &profile.serial_device },
            profile.baud
        ),
        "i2c" => format!(
            "I2C Bus │ Device: {} │ Address: {}",
            if profile.i2c_device.is_empty() { "/dev/i2c-1" } else { &profile.i2c_device },
            profile.i2c_address
        ),
        _ => "InMemory Bus (Hardware Simulation Mode)".to_string(),
    };

    let backend_lines = vec![
        Line::from(vec![
            Span::styled("Active Backend: ", Style::default().fg(palette.text_muted)),
            Span::styled(profile.backend.to_uppercase(), Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ({backend_detail})"), Style::default().fg(palette.text)),
        ]),
        Line::from(vec![
            Span::styled("Switch Backend: ", Style::default().fg(palette.text_muted)),
            Span::styled("[c] CAN  ", Style::default().fg(palette.accent)),
            Span::styled("[e] Serial  ", Style::default().fg(palette.accent)),
            Span::styled("[i] I2C  ", Style::default().fg(palette.accent)),
            Span::styled("[m] Memory Sim", Style::default().fg(palette.accent)),
        ]),
    ];

    let backend_block = Paragraph::new(backend_lines).block(
        Block::default()
            .title(" Transport Backend ")
            .title_style(Style::default().fg(palette.accent))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(backend_block, chunks[2]);

    // 4. Output Logs
    let log_lines: Vec<Line> = logs
        .iter()
        .rev()
        .take(15)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| {
            let color = if l.contains("ERROR") {
                palette.err
            } else if l.contains("DONE") || l.contains("complete") {
                palette.ok
            } else if l.contains("progress") {
                palette.accent
            } else {
                palette.text
            };
            Line::from(Span::styled(l.clone(), Style::default().fg(color)))
        })
        .collect();

    let logs_block = Paragraph::new(log_lines)
        .block(
            Block::default()
                .title(" Session Log ")
                .title_style(Style::default().fg(palette.accent))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(logs_block, chunks[3]);
}
