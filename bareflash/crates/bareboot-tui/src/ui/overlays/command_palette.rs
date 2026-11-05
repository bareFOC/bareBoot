use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use crate::overlay::CommandPaletteState;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &CommandPaletteState,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup_v[1])[1];

    frame.render_widget(Clear, popup_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(4),    // Filtered command list
        ])
        .split(popup_area);

    // Search bar
    let search_line = Line::from(vec![
        Span::styled(" > ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
        Span::styled(&state.query, Style::default().fg(palette.text)),
        Span::styled("█", Style::default().fg(palette.accent)),
    ]);

    let search_box = Paragraph::new(search_line).block(
        Block::default()
            .title(" Command Palette (Type to search, [Enter] Run, [Esc] Close) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(search_box, inner[0]);

    // Filtered items
    let filtered = state.filtered_items();
    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No matching commands found",
            Style::default().fg(palette.text_muted),
        )))]
    } else {
        filtered
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_sel = i == state.selected;
                let (bg, fg, prefix) = if is_sel {
                    (palette.accent, palette.bg, " ▶ ")
                } else {
                    (palette.raised, palette.text, "   ")
                };

                let style = Style::default().bg(bg).fg(fg);
                let shortcut_style = if is_sel {
                    Style::default().bg(bg).fg(palette.bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().bg(bg).fg(palette.text_muted)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(format!("{:<30}", item.title), style.add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" [{}]", item.category), Style::default().bg(bg).fg(palette.accent_dim)),
                    Span::styled(format!("   [{}]", item.shortcut), shortcut_style),
                ]))
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(list, inner[1]);
}
