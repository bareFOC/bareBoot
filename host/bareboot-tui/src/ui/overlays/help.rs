use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use crate::theme::Palette;

pub fn render(frame: &mut Frame, area: Rect, palette: &Palette) {
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
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup_v[1])[1];

    frame.render_widget(Clear, popup_area);

    let key_row = |k: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {:<16}", k), Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(desc, Style::default().fg(palette.text)),
        ])
    };

    let section_header = |title: &'static str| {
        Line::from(vec![
            Span::styled(format!("\n── {} ──", title), Style::default().fg(palette.accent_dim).add_modifier(Modifier::BOLD)),
        ])
    };

    let content = vec![
        section_header("Navigation & Three Tabs"),
        key_row("1 - 3", "Directly switch tabs (1: Target Config, 2: Flasher, 3: Console)"),
        key_row("Left / Right", "Cycle tabs left and right"),
        key_row("Tab / Shift-Tab", "Cycle form fields (in Config) or cycle tabs"),

        section_header("Flasher Tab (Build, Flash & Katapult Makeflash)"),
        key_row("Space", "Run KATAPULT MAKEFLASH (Compile & Flash in one touch)"),
        key_row("b", "Run Cargo Build only (without flashing)"),
        key_row("f", "Flash discovered/selected binary"),
        key_row("t", "Cycle target project (blink-test, bareboot, ch32v203-usb)"),
        key_row("d", "Cycle deploy method (Probe-RS, Katapult CAN, Serial, Memory)"),
        key_row("m", "Toggle Release (--release) vs Debug mode"),
        key_row("e", "Toggle Full Chip Erase [ON/OFF]"),
        key_row("v", "Toggle Post-Flash Verify [ON/OFF]"),
        key_row("p", "Query attached hardware probes (WCH-Link, CMSIS-DAP)"),
        key_row("r", "Hardware reset target MCU"),

        section_header("Console & Bus Traffic Tab"),
        key_row("Space", "Pause / Resume incoming protocol packet stream"),
        key_row("s", "Broadcast bus scan to ping bootloader nodes"),
        key_row("c", "Clear protocol bus frame table"),
        key_row("x", "Clear system diagnostic logs"),

        section_header("Global & Overlays"),
        key_row(":", "Open global command palette"),
        key_row("T / t", "Cycle visual theme (Ember, Nord, Gruvbox, Tokyo, Synth)"),
        key_row("?", "Toggle this help dialog"),
        key_row("Esc", "Dismiss active overlay / cancel operation"),
        key_row("q / Ctrl+C", "Quit bareflash"),
    ];

    let block = Paragraph::new(content).block(
        Block::default()
            .title(" Keyboard Shortcut Reference (Press [Esc] or [?] to close) ")
            .title_style(Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.raised)),
    );
    frame.render_widget(block, popup_area);
}
