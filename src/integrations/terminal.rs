use amane::Color;

use crate::theme::{self, Theme};

// the seeds for the colors the shell's theme has no name for
const YELLOW: Color = Color::rgb(0xf9, 0xe2, 0xaf);
const MAGENTA: Color = Color::rgb(0xf5, 0xc2, 0xe7);
const CYAN: Color = Color::rgb(0x94, 0xe2, 0xd5);
const BLUE: Color = Color::rgb(0x89, 0xb4, 0xfa);

// the 16 ansi colors, and the few around them
pub struct Colors {
    pub background: Color,
    pub foreground: Color,
    pub selection: Color,
    pub cursor: Color,
    pub ansi: [Color; 16],
}

pub fn colors(theme: &Theme) -> Colors {
    let light = theme.light;

    // pulled toward the accent, so they sit with the rest of the theme
    let harmonize = |seed: Color, amount: f32| {
        let lightness = if light { 0.42 } else { 0.70 };

        theme::retone(theme::mix(seed, theme.accent, amount), lightness, 0.48)
    };

    // the dimmer first eight
    let regular = |color: Color| {
        let lightness = if light { 0.36 } else { 0.58 };

        theme::retone(color, lightness, 0.44)
    };

    let yellow = harmonize(YELLOW, 0.22);
    let magenta = harmonize(MAGENTA, 0.28);
    let cyan = harmonize(CYAN, 0.24);
    let blue = harmonize(BLUE, 0.30);

    Colors {
        background: theme.background,
        foreground: theme.text,
        selection: theme.selected_surface,
        cursor: theme.accent,
        ansi: [
            theme.surface,
            regular(theme.danger),
            regular(theme.success),
            regular(yellow),
            regular(blue),
            regular(magenta),
            regular(cyan),
            theme.secondary_text,
            theme.muted_text,
            theme.danger,
            theme.success,
            yellow,
            blue,
            magenta,
            cyan,
            theme.text,
        ],
    }
}
