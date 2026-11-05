use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppTab {
    TargetConfig,
    Flasher,
    Console,
}

impl AppTab {
    pub fn all() -> &'static [AppTab] {
        &[AppTab::TargetConfig, AppTab::Flasher, AppTab::Console]
    }

    pub fn index(&self) -> usize {
        match self {
            AppTab::TargetConfig => 0,
            AppTab::Flasher => 1,
            AppTab::Console => 2,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            AppTab::TargetConfig => " 1. Target Config (Post-Flash) ",
            AppTab::Flasher => " 2. Flasher (Katapult Makeflash) ",
            AppTab::Console => " 3. Console & Traffic ",
        }
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    active_tab: AppTab,
) {
    let mut spans = Vec::new();

    for tab in AppTab::all() {
        let is_active = *tab == active_tab;
        let style = if is_active {
            Style::default()
                .fg(palette.bg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(palette.text_muted)
                .bg(palette.surface)
        };

        spans.push(Span::styled(tab.title(), style));
        spans.push(Span::styled("  ", Style::default().bg(palette.surface)));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(p, area);
}
