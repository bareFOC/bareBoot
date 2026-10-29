use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub active: Color,
    pub muted: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            active: Color::Yellow,
            muted: Color::Gray,
            accent: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            border: Color::DarkGray,
        }
    }
}

pub const THEME: Theme = Theme {
    bg: Color::Reset,
    fg: Color::White,
    active: Color::Yellow,
    muted: Color::Gray,
    accent: Color::Cyan,
    success: Color::Green,
    warning: Color::Yellow,
    error: Color::Red,
    border: Color::DarkGray,
};