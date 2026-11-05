use ratatui::style::Color;

pub type Rgb = (u8, u8, u8);

const BLACK: Rgb = (0, 0, 0);
const WHITE: Rgb = (255, 255, 255);

fn mix(a: Rgb, b: Rgb, ratio: f32) -> Rgb {
    let t = ratio.clamp(0.0, 1.0);
    (
        ((1.0 - t) * a.0 as f32 + t * b.0 as f32).round() as u8,
        ((1.0 - t) * a.1 as f32 + t * b.1 as f32).round() as u8,
        ((1.0 - t) * a.2 as f32 + t * b.2 as f32).round() as u8,
    )
}

fn to_color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color,
    pub surface: Color,
    pub raised: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub ok: Color,
    pub err: Color,
    pub warn: Color,

    // Legacy compatibility fields
    pub fg: Color,
    pub active: Color,
    pub muted: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeDefinition {
    pub name: &'static str,
    pub accent: Rgb,
    pub secondary: Option<Rgb>,
    pub base: Option<Rgb>,
}

pub const THEMES: &[ThemeDefinition] = &[
    ThemeDefinition {
        name: "ember",
        accent: (255, 140, 50), // Warm hardware amber/orange
        secondary: Some((255, 195, 110)),
        base: None,
    },
    ThemeDefinition {
        name: "nord",
        accent: (136, 192, 208), // Frost blue
        secondary: Some((129, 161, 193)),
        base: Some((46, 52, 64)),
    },
    ThemeDefinition {
        name: "gruvbox",
        accent: (250, 189, 47), // Warm yellow
        secondary: Some((254, 128, 25)),
        base: Some((40, 40, 40)),
    },
    ThemeDefinition {
        name: "tokyo-night",
        accent: (122, 162, 247), // Cyber blue
        secondary: Some((187, 154, 247)),
        base: Some((26, 27, 38)),
    },
    ThemeDefinition {
        name: "synthwave",
        accent: (255, 113, 206), // Neon pink
        secondary: Some((1, 205, 254)),
        base: Some((38, 26, 58)),
    },
];

impl ThemeDefinition {
    pub fn palette(&self) -> Palette {
        let (bg, surface, raised, border, text_muted, faint) = match self.base {
            Some(base) => (
                base,
                mix(base, WHITE, 0.07),
                mix(base, WHITE, 0.15),
                mix(base, self.accent, 0.35),
                mix(base, self.accent, 0.65),
                mix(base, self.accent, 0.25),
            ),
            None => (
                mix(BLACK, self.accent, 0.08),
                mix(BLACK, self.accent, 0.14),
                mix(BLACK, self.accent, 0.22),
                mix(BLACK, self.accent, 0.35),
                mix(BLACK, self.accent, 0.65),
                mix(BLACK, self.accent, 0.22),
            ),
        };

        let text = mix(WHITE, self.accent, 0.15);
        let accent_dim = mix(self.accent, bg, 0.45);
        let ok = (90, 220, 120);     // Permanent green
        let err = (240, 90, 90);     // Permanent red
        let warn = (250, 189, 47);   // Permanent amber/yellow

        Palette {
            bg: to_color(bg),
            surface: to_color(surface),
            raised: to_color(raised),
            border: to_color(border),
            text: to_color(text),
            text_muted: to_color(text_muted),
            faint: to_color(faint),
            accent: to_color(self.accent),
            accent_dim: to_color(accent_dim),
            ok: to_color(ok),
            err: to_color(err),
            warn: to_color(warn),

            // Legacy mappings
            fg: to_color(text),
            active: to_color(self.accent),
            muted: to_color(text_muted),
            success: to_color(ok),
            warning: to_color(warn),
            error: to_color(err),
        }
    }
}

pub fn get_palette(index: usize) -> Palette {
    let theme = THEMES.get(index % THEMES.len()).unwrap_or(&THEMES[0]);
    theme.palette()
}

pub fn get_theme_name(index: usize) -> &'static str {
    THEMES.get(index % THEMES.len()).unwrap_or(&THEMES[0]).name
}

// Global default Ember palette for convenience
pub static THEME: std::sync::LazyLock<Palette> = std::sync::LazyLock::new(|| THEMES[0].palette());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_count_and_cycling() {
        assert_eq!(THEMES.len(), 5);
        assert_eq!(get_theme_name(0), "ember");
        assert_eq!(get_theme_name(1), "nord");
        assert_eq!(get_theme_name(5), "ember"); // cycles
    }

    #[test]
    fn test_constant_status_colors() {
        for theme in THEMES {
            let p = theme.palette();
            assert_eq!(p.ok, Color::Rgb(90, 220, 120));
            assert_eq!(p.err, Color::Rgb(240, 90, 90));
            assert_eq!(p.warn, Color::Rgb(250, 189, 47));
        }
    }

    #[test]
    fn test_ember_palette_contrast() {
        let p = THEMES[0].palette();
        assert_ne!(p.bg, p.surface);
        assert_ne!(p.surface, p.raised);
        assert_ne!(p.text, p.text_muted);
    }
}
