use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use crate::overlay::SelectModalState;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &SelectModalState,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(18),
            Constraint::Percentage(64),
            Constraint::Percentage(18),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(18),
            Constraint::Percentage(64),
            Constraint::Percentage(18),
        ])
        .split(popup_v[1])[1];

    frame.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // List of items
            Constraint::Length(2), // Help / Instructions
        ])
        .split(popup_area);

    let items: Vec<ListItem> = state
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_sel = i == state.selected;
            let (bg, fg_title, fg_desc, marker) = if is_sel {
                (palette.accent, palette.bg, palette.bg, " (•) ")
            } else {
                (palette.raised, palette.text, palette.text_muted, " ( ) ")
            };
            let style = Style::default().bg(bg);
            ListItem::new(Line::from(vec![
                Span::styled(marker, style.fg(if is_sel { palette.bg } else { palette.accent })),
                Span::styled(&item.title, style.fg(fg_title).add_modifier(Modifier::BOLD)),
                Span::styled("  ", style),
                Span::styled(&item.description, style.fg(fg_desc)),
            ]))
        })
        .collect();

    let title = format!(" [ Settings: {} ] ", state.title);
    let list_widget = List::new(items).block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(list_widget, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [↑/↓/Wheel] Navigate   ", Style::default().fg(palette.text_muted)),
        Span::styled("[Enter/Click] Select   ", Style::default().fg(palette.ok).add_modifier(Modifier::BOLD)),
        Span::styled("[Esc] Cancel", Style::default().fg(palette.text_muted)),
    ])).style(Style::default().bg(palette.raised));
    frame.render_widget(footer, chunks[1]);
}
