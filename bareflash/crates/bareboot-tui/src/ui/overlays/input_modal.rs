use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use crate::overlay::InputModalState;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &InputModalState,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(32),
            Constraint::Length(8),
            Constraint::Percentage(32),
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

    let text_lines = vec![
        Line::from(Span::styled(&state.prompt, Style::default().fg(palette.text_muted))),
        Line::from(""),
        Line::from(vec![
            Span::styled(" > ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(&state.value, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(palette.accent)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter] Confirm   ", Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
            Span::styled("[Esc] Cancel", Style::default().fg(palette.text_muted)),
        ]),
    ];

    let title = format!(" [ {} ] ", state.title);
    let block = Paragraph::new(text_lines).block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(block, popup_area);
}
