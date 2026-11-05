use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    logs: &[String],
    is_building: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Probe & Debugger Status
            Constraint::Min(8),    // Streaming Output
        ])
        .split(area);

    // 1. Probe & Build Action Card
    let status_str = if is_building {
        "COMPILING FIRMWARE..."
    } else {
        "IDLE - READY"
    };

    let probe_lines = vec![
        Line::from(vec![
            Span::styled("Attached Debug Probes: ", Style::default().fg(palette.text_muted)),
            Span::styled("WCH-LinkE (CMSIS-DAP v2.9) on USB 1-2.4", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Actions: ", Style::default().fg(palette.text_muted)),
            Span::styled("[b] Cargo Build Firmware  ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled("[p] Probe-rs Query Target  ", Style::default().fg(palette.accent)),
            Span::styled(format!("Status: [{status_str}]"), Style::default().fg(if is_building { palette.warn } else { palette.ok })),
        ]),
    ];

    let probe_block = Paragraph::new(probe_lines).block(
        Block::default()
            .title(" Hardware Debugger & Compiler ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(probe_block, chunks[0]);

    // 2. Build & Probe Output Stream
    let log_lines: Vec<Line> = logs
        .iter()
        .rev()
        .take(area.height.saturating_sub(7) as usize)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| {
            let color = if l.contains("error") || l.contains("ERROR") {
                palette.err
            } else if l.contains("warning") {
                palette.warn
            } else if l.contains("Finished") || l.contains("DONE") {
                palette.ok
            } else if l.contains("Compiling") {
                palette.accent
            } else {
                palette.text
            };
            Line::from(Span::styled(l.clone(), Style::default().fg(color)))
        })
        .collect();

    let output_block = Paragraph::new(log_lines)
        .block(
            Block::default()
                .title(" Compiler / Probe Output Stream ")
                .title_style(Style::default().fg(palette.accent))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(output_block, chunks[1]);
}
