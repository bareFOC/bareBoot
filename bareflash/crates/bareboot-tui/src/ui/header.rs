use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    target: &str,
    transport: &str,
    node_id: &str,
    is_online: bool,
    theme_name: &str,
) {
    let [left, right] = Layout::horizontal([
        Constraint::Min(40),
        Constraint::Length(28),
    ])
    .areas(area);

    let sep = Span::styled(" │ ", Style::default().fg(palette.border));

    let header_line = Line::from(vec![
        Span::styled(
            " BAREFLASH",
            Style::default().fg(palette.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" v0.2.0", Style::default().fg(palette.text_muted)),
        sep.clone(),
        Span::styled("target: ", Style::default().fg(palette.text_muted)),
        Span::styled(target, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        sep.clone(),
        Span::styled("bus: ", Style::default().fg(palette.text_muted)),
        Span::styled(transport, Style::default().fg(palette.text)),
        sep,
        Span::styled("node: ", Style::default().fg(palette.text_muted)),
        Span::styled(node_id, Style::default().fg(palette.text)),
    ]);

    let left_para = Paragraph::new(header_line)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        );
    frame.render_widget(left_para, left);

    let (status_text, status_color) = if is_online {
        (" [ONLINE] ", palette.ok)
    } else {
        (" [DISCONNECTED] ", palette.err)
    };

    let right_line = Line::from(vec![
        Span::styled(format!("[{theme_name}] "), Style::default().fg(palette.text_muted)),
        Span::styled(
            status_text,
            Style::default().fg(color_black_or_dark(palette)).bg(status_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);

    let right_para = Paragraph::new(right_line)
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface)),
        );
    frame.render_widget(right_para, right);
}

fn color_black_or_dark(palette: &Palette) -> ratatui::style::Color {
    palette.bg
}
