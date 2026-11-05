use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    FlashStudio,
    BusAnalyzer,
    TargetConfig,
    BuildProbe,
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    active_view: View,
    profiles: &[String],
    active_profile_idx: usize,
    probe_name: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Views
            Constraint::Min(6),    // Profiles
            Constraint::Length(5), // Hardware / Probe
        ])
        .split(area);

    // Views List
    let view_items = vec![
        (View::FlashStudio, "1. Flash Studio"),
        (View::BusAnalyzer, "2. Protocol Bus"),
        (View::TargetConfig, "3. Target Config"),
        (View::BuildProbe, "4. Build & Probe"),
    ];

    let items: Vec<ListItem> = view_items
        .into_iter()
        .map(|(v, label)| {
            let is_active = v == active_view;
            let (style, prefix) = if is_active {
                (
                    Style::default()
                        .fg(palette.bg)
                        .bg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                    "▶ ",
                )
            } else {
                (Style::default().fg(palette.text), "  ")
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(label, style),
            ]))
        })
        .collect();

    let views_list = List::new(items).block(
        Block::default()
            .title(" Views (1-4) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(views_list, chunks[0]);

    // Profiles List
    let profile_items: Vec<ListItem> = if profiles.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  (No profiles saved)",
            Style::default().fg(palette.text_muted),
        )))]
    } else {
        profiles
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let is_sel = i == active_profile_idx;
                let (style, marker) = if is_sel {
                    (
                        Style::default()
                            .fg(palette.accent)
                            .add_modifier(Modifier::BOLD),
                        "• ",
                    )
                } else {
                    (Style::default().fg(palette.text_muted), "  ")
                };
                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(name, style),
                ]))
            })
            .collect()
    };

    let profiles_block = List::new(profile_items).block(
        Block::default()
            .title(" Profiles (p) ")
            .title_style(Style::default().fg(palette.accent))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(profiles_block, chunks[1]);

    // Hardware / Probe status
    let probe_info = vec![
        Line::from(vec![
            Span::styled("Probe: ", Style::default().fg(palette.text_muted)),
            Span::styled(probe_name, Style::default().fg(palette.text)),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(palette.text_muted)),
            Span::styled("Ready", Style::default().fg(palette.ok)),
        ]),
    ];

    let hw_block = Paragraph::new(probe_info).block(
        Block::default()
            .title(" Hardware ")
            .title_style(Style::default().fg(palette.accent))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(hw_block, chunks[2]);
}
