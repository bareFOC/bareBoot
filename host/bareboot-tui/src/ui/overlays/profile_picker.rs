use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};
use crate::overlay::ProfilePickerState;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &ProfilePickerState,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
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

    let items: Vec<ListItem> = if state.names.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No profiles saved",
            Style::default().fg(palette.text_muted),
        )))]
    } else {
        state
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let is_sel = i == state.selected;
                let (bg, fg, marker) = if is_sel {
                    (palette.accent, palette.bg, " ▶ ")
                } else {
                    (palette.raised, palette.text, "   ")
                };
                let style = Style::default().bg(bg).fg(fg);
                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(name, style.add_modifier(Modifier::BOLD)),
                ]))
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .title(" Select Flash Profile ([Enter] Load, [Esc] Cancel) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(list, popup_area);
}
