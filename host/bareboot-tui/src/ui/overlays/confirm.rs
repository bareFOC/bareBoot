use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    message: &str,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(7),
            Constraint::Percentage(35),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(popup_v[1])[1];

    frame.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(message, Style::default().fg(palette.text).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(palette.text_muted)),
            Span::styled("[y] Yes, Confirm", Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
            Span::styled("   or   ", Style::default().fg(palette.text_muted)),
            Span::styled("[n/Esc] Cancel", Style::default().fg(palette.err).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let block = Paragraph::new(content)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Confirmation Required ")
                .title_style(Style::default().fg(palette.warn).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.warn))
                .style(Style::default().bg(palette.raised)),
        );
    frame.render_widget(block, popup_area);
}
