use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::theme::Palette;
use super::tabs::AppTab;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    palette: &Palette,
    active_tab: AppTab,
) {
    let mut spans = Vec::new();

    let sep = || Span::styled(" │ ", Style::default().fg(palette.border));
    let key = |k: &'static str, desc: &'static str| {
        vec![
            Span::styled(format!("[{k}]"), Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {desc}"), Style::default().fg(palette.text_muted)),
        ]
    };

    match active_tab {
        AppTab::TargetConfig => {
            spans.extend(key("↑/↓", "Nav"));
            spans.push(sep());
            spans.extend(key("Enter", "Modal Edit"));
            spans.push(sep());
            spans.extend(key("Space", "Toggle"));
            spans.push(sep());
            spans.extend(key("r", "Read MCU"));
            spans.push(sep());
            spans.extend(key("w", "Write Flash"));
            spans.push(sep());
            spans.extend(key("j", "Jump App"));
        }
        AppTab::Flasher => {
            spans.extend(key("Space", "Makeflash"));
            spans.push(sep());
            spans.extend(key("Enter", "Modal"));
            spans.push(sep());
            spans.extend(key("P", "Load Profile"));
            spans.push(sep());
            spans.extend(key("S", "Save Profile"));
            spans.push(sep());
            spans.extend(key("t", "Target"));
            spans.push(sep());
            spans.extend(key("d", "Deploy"));
            spans.push(sep());
            spans.extend(key("m", "Mode"));
        }
        AppTab::Console => {
            spans.extend(key("Space", "Pause Stream"));
            spans.push(sep());
            spans.extend(key("s", "Broadcast Scan"));
            spans.push(sep());
            spans.extend(key("c", "Clear Frames"));
            spans.push(sep());
            spans.extend(key("x", "Clear Logs"));
        }
    }

    spans.push(sep());
    spans.extend(key("1-3", "Tabs"));
    spans.push(sep());
    spans.extend(key("T", "Theme"));
    spans.push(sep());
    spans.extend(key(":", "Commands"));
    spans.push(sep());
    spans.extend(key("?", "Help"));
    spans.push(sep());
    spans.extend(key("q", "Quit"));

    let footer_para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(palette.border))
            .style(Style::default().bg(palette.surface)),
    );
    frame.render_widget(footer_para, area);
}
