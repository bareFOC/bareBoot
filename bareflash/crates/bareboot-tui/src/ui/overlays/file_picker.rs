use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use crate::overlay::FilePickerState;
use crate::theme::Palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    state: &FilePickerState,
) {
    let popup_v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(popup_v[1])[1];

    frame.render_widget(Clear, popup_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Path indicator
            Constraint::Min(4),    // File list
        ])
        .split(popup_area);

    let path_str = state.current_dir.display().to_string();
    let path_para = Paragraph::new(Line::from(vec![
        Span::styled(" Directory: ", Style::default().fg(palette.text_muted)),
        Span::styled(path_str, Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
    ]))
    .block(
        Block::default()
            .title(" Select Firmware Binary ([j/k] Navigate, [Enter] Select, [Backspace] Up, [Esc] Cancel) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(path_para, inner[0]);

    let items: Vec<ListItem> = if state.entries.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  (No compatible binary files in directory)",
            Style::default().fg(palette.text_muted),
        )))]
    } else {
        state
            .entries
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let is_sel = i == state.selected;
                let is_dir = path.is_dir();
                let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");

                let (bg, fg, marker) = if is_sel {
                    (palette.accent, palette.bg, " ▶ ")
                } else {
                    (palette.raised, palette.text, "   ")
                };

                let icon = if is_dir { "📁 " } else { "📦 " };
                let style = Style::default().bg(bg).fg(fg);

                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(icon, style),
                    Span::styled(file_name, style.add_modifier(Modifier::BOLD)),
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
