use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};
use crate::protocol::{DecodedFrame, Direction as FrameDir};
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    frames: &[DecodedFrame],
    logs: &[String],
    is_paused: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // Traffic summary strip
            Constraint::Percentage(55),// Protocol Bus Table
            Constraint::Percentage(45),// System & Compiler Logs
        ])
        .split(area);

    // 1. Bus Traffic Summary Strip
    let (status_str, status_col) = if is_paused {
        ("PAUSED", palette.warn)
    } else {
        ("STREAMING", palette.ok)
    };

    let summary_line = Line::from(vec![
        Span::styled("Bus Monitor: ", Style::default().fg(palette.text_muted)),
        Span::styled(format!(" [{status_str}] "), Style::default().fg(palette.bg).bg(status_col).add_modifier(Modifier::BOLD)),
        Span::styled("   Captured: ", Style::default().fg(palette.text_muted)),
        Span::styled(format!("{} frames", frames.len()), Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        Span::styled("   Controls: [Space] Pause/Resume   [s] Broadcast Scan   [c] Clear Frames   [x] Clear Logs", Style::default().fg(palette.text_muted)),
    ]);

    let summary_block = Paragraph::new(summary_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(summary_block, chunks[0]);

    // 2. Real-Time Protocol Frame Table
    let header_row = Row::new(vec![
        "TIME", "DIR", "BUS", "NODE", "OPCODE", "HEX (UP TO 8B)", "DETAIL",
    ])
    .style(
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD)
            .bg(palette.surface),
    )
    .bottom_margin(1);

    let rows: Vec<Row> = frames
        .iter()
        .rev()
        .take(chunks[1].height.saturating_sub(4) as usize)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|f| {
            let dir_span = match f.direction {
                FrameDir::Tx => Span::styled("TX ▶", Style::default().fg(palette.accent)),
                FrameDir::Rx => Span::styled("◀ RX", Style::default().fg(palette.ok)),
            };

            let op_style = if f.is_ack {
                Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)
            } else if f.is_error {
                Style::default().fg(palette.err).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.text).add_modifier(Modifier::BOLD)
            };

            Row::new(vec![
                Line::from(Span::styled(&f.timestamp, Style::default().fg(palette.text_muted))),
                Line::from(dir_span),
                Line::from(Span::styled(format!("{:?}", f.bus_type), Style::default().fg(palette.text_muted))),
                Line::from(Span::styled(&f.node_id, Style::default().fg(palette.accent))),
                Line::from(Span::styled(&f.opcode, op_style)),
                Line::from(Span::styled(&f.hex, Style::default().fg(palette.text_muted))),
                Line::from(Span::styled(&f.detail, Style::default().fg(palette.text))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12), // Time
        Constraint::Length(6),  // Dir
        Constraint::Length(8),  // Bus
        Constraint::Length(8),  // Node
        Constraint::Length(12), // Opcode
        Constraint::Length(24), // Hex
        Constraint::Min(20),    // Detail
    ];

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(
            Block::default()
                .title(" Protocol Bus Traffic (CAN, I2C, Serial Bootloader Frames) ")
                .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        );
    frame.render_widget(table, chunks[1]);

    // 3. System Diagnostic & Compiler Logs
    let log_lines: Vec<Line> = logs
        .iter()
        .rev()
        .take(chunks[2].height.saturating_sub(2) as usize)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| {
            let color = if l.contains("error") || l.contains("ERROR") {
                palette.err
            } else if l.contains("warning") {
                palette.warn
            } else if l.contains("DONE") || l.contains("complete") {
                palette.ok
            } else if l.contains("Makeflash") || l.contains("Compiling") {
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
                .title(" System & Console Diagnostic Log ")
                .title_style(Style::default().fg(palette.accent))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(logs_block, chunks[2]);
}
