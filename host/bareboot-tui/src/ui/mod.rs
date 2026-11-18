pub mod views;
pub mod overlays;
pub mod header;
pub mod tabs;
pub mod footer;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Block,
    Frame,
};
use crate::theme::Palette;

#[derive(Clone, Copy, Debug)]
pub struct LayoutAreas {
    pub header: Rect,
    pub tabs: Rect,
    pub main_surface: Rect,
    pub footer: Rect,
}

pub fn compute_layout(area: Rect) -> (Rect, LayoutAreas) {
    // 1. Max-width container (up to 130 cols) centered with painted margins
    let bounded_width = area.width.min(130);
    let app_area = Rect {
        x: area.x + (area.width - bounded_width) / 2,
        y: area.y,
        width: bounded_width,
        height: area.height,
    };

    // 2. Full vertical allocation: Header (2), Tabs (2), Main Body (Min 1), Footer (1)
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Branded header
            Constraint::Length(2), // 3-tab bar
            Constraint::Min(1),    // Main workspace (full width & height)
            Constraint::Length(1), // Key hints footer
        ])
        .split(app_area);

    (
        app_area,
        LayoutAreas {
            header: vert_chunks[0],
            tabs: vert_chunks[1],
            main_surface: vert_chunks[2],
            footer: vert_chunks[3],
        },
    )
}

pub fn render_background(frame: &mut Frame, area: Rect, palette: &Palette) {
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.bg).fg(palette.text)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ultrawide_capping() {
        let wide = Rect { x: 0, y: 0, width: 200, height: 50 };
        let (app_area, _) = compute_layout(wide);
        assert_eq!(app_area.width, 130);
        assert_eq!(app_area.x, 35);
    }

    #[test]
    fn test_full_height_allocation() {
        let area = Rect { x: 0, y: 0, width: 100, height: 40 };
        let (_, layout) = compute_layout(area);
        // Header(2) + Tabs(2) + Footer(1) leaves 35 for body
        assert_eq!(layout.main_surface.height, 35);
        assert_eq!(layout.main_surface.width, 100);
    }
}
