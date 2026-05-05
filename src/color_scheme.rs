//! Color scheme definitions for LivePlot
//!
//! This module contains the ColorScheme enum, CustomColorScheme struct, and related methods.

use eframe::egui::{Color32, Context, Visuals};
use once_cell::sync::Lazy;
use std::sync::Mutex;

// Global palette used for trace color allocation.  This is updated whenever a
// color scheme is applied.  The value is cloned internally so callers can
// freely mutate the returned vector.
static GLOBAL_PALETTE: Lazy<Mutex<Vec<Color32>>> = Lazy::new(|| {
    // initialize with default dark scheme palette
    Mutex::new(ColorScheme::Dark.trace_colors())
});

/// Get a copy of the current global trace colour palette.
///
/// This is exposed primarily for unit tests; most production code should just
/// rely on [`TraceLook::alloc_color`] which consults the same palette.
pub fn global_palette() -> Vec<Color32> {
    GLOBAL_PALETTE.lock().unwrap().clone()
}

/// Update the global colour palette.  Called automatically when a
/// [`ColorScheme`] is applied, but user code (or tests) may call it directly.
pub(crate) fn set_global_palette(new: Vec<Color32>) {
    let mut guard = GLOBAL_PALETTE.lock().unwrap();
    *guard = new;
}

/// Visual theme for the plot UI, including user-defined custom schemes.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorScheme {
    /// Follow the system / eframe default (typically dark).
    Dark,
    /// Light theme.
    Light,
    /// Solarized Dark.
    SolarizedDark,
    /// Solarized Light.
    SolarizedLight,
    /// ggplot2-inspired: light grey background with muted primary colours.
    GgPlot,
    /// Nord: blue-grey dark theme.
    Nord,
    /// Monokai: vivid colours on a dark background.
    Monokai,
    /// Dracula: dark purple-tinted background with vivid accent colours.
    Dracula,
    /// Gruvbox Dark: retro-warm dark theme.
    GruvboxDark,
    /// High-contrast: pure-black background with maximally-saturated colours.
    HighContrast,
    /// User-defined custom color scheme.
    Custom(CustomColorScheme),
}

/// User-defined custom color scheme.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomColorScheme {
    /// Visuals for egui context (optional, fallback to dark/light).
    pub visuals: Option<Visuals>,
    /// Trace color palette.
    pub palette: Vec<Color32>,
    /// Optional label for UI display.
    pub label: Option<String>,
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme::Dark
    }
}

impl ColorScheme {
    /// All built-in schemes (useful for combo-box UIs).
    pub fn all() -> &'static [ColorScheme] {
        &[
            ColorScheme::Dark,
            ColorScheme::Light,
            ColorScheme::SolarizedDark,
            ColorScheme::SolarizedLight,
            ColorScheme::GgPlot,
            ColorScheme::Nord,
            ColorScheme::Monokai,
            ColorScheme::Dracula,
            ColorScheme::GruvboxDark,
            ColorScheme::HighContrast,
        ]
    }

    /// Human-readable label.
    pub fn label(&self) -> String {
        match self {
            ColorScheme::Dark => "Dark".to_string(),
            ColorScheme::Light => "Light".to_string(),
            ColorScheme::SolarizedDark => "Solarized Dark".to_string(),
            ColorScheme::SolarizedLight => "Solarized Light".to_string(),
            ColorScheme::GgPlot => "ggplot2".to_string(),
            ColorScheme::Nord => "Nord".to_string(),
            ColorScheme::Monokai => "Monokai".to_string(),
            ColorScheme::Dracula => "Dracula".to_string(),
            ColorScheme::GruvboxDark => "Gruvbox Dark".to_string(),
            ColorScheme::HighContrast => "High Contrast".to_string(),
            ColorScheme::Custom(custom) => {
                custom.label.clone().unwrap_or_else(|| "Custom".to_string())
            }
        }
    }

    /// Apply this scheme's visuals to an egui context.
    pub fn apply(&self, ctx: &Context) {
        match self {
            ColorScheme::Dark => ctx.set_visuals(Visuals::dark()),
            ColorScheme::Light => ctx.set_visuals(Visuals::light()),
            ColorScheme::SolarizedDark => {
                let mut v = Visuals::dark();
                let base03 = Color32::from_rgb(0, 43, 54);
                let base02 = Color32::from_rgb(7, 54, 66);
                let base01 = Color32::from_rgb(88, 110, 117);
                let base0 = Color32::from_rgb(131, 148, 150);
                v.panel_fill = base03;
                v.window_fill = base02;
                v.extreme_bg_color = base03;
                v.faint_bg_color = base02;
                v.override_text_color = Some(base0);
                v.widgets.noninteractive.bg_fill = base02;
                v.widgets.noninteractive.fg_stroke.color = base0;
                v.widgets.inactive.bg_fill = base02;
                v.widgets.inactive.fg_stroke.color = base01;
                v.widgets.hovered.bg_fill = base01;
                v.widgets.active.bg_fill = base01;
                ctx.set_visuals(v);
            }
            ColorScheme::SolarizedLight => {
                let mut v = Visuals::light();
                let base3 = Color32::from_rgb(253, 246, 227);
                let base2 = Color32::from_rgb(238, 232, 213);
                let base00 = Color32::from_rgb(101, 123, 131);
                v.panel_fill = base3;
                v.window_fill = base2;
                v.extreme_bg_color = base3;
                v.faint_bg_color = base2;
                v.override_text_color = Some(base00);
                v.widgets.noninteractive.bg_fill = base2;
                v.widgets.noninteractive.fg_stroke.color = base00;
                v.widgets.inactive.bg_fill = base2;
                v.widgets.inactive.fg_stroke.color = base00;
                ctx.set_visuals(v);
            }
            ColorScheme::GgPlot => {
                let mut v = Visuals::light();
                let bg = Color32::from_rgb(229, 229, 229);
                let fg = Color32::from_rgb(51, 51, 51);
                v.panel_fill = bg;
                v.window_fill = Color32::WHITE;
                v.extreme_bg_color = bg;
                v.faint_bg_color = Color32::from_rgb(240, 240, 240);
                v.override_text_color = Some(fg);
                v.widgets.noninteractive.bg_fill = Color32::from_rgb(240, 240, 240);
                v.widgets.noninteractive.fg_stroke.color = fg;
                ctx.set_visuals(v);
            }
            ColorScheme::Nord => {
                let mut v = Visuals::dark();
                let polar0 = Color32::from_rgb(46, 52, 64);
                let polar1 = Color32::from_rgb(59, 66, 82);
                let snow0 = Color32::from_rgb(216, 222, 233);
                let snow1 = Color32::from_rgb(229, 233, 240);
                v.panel_fill = polar0;
                v.window_fill = polar1;
                v.extreme_bg_color = polar0;
                v.faint_bg_color = polar1;
                v.override_text_color = Some(snow0);
                v.widgets.noninteractive.bg_fill = polar1;
                v.widgets.noninteractive.fg_stroke.color = snow0;
                v.widgets.inactive.fg_stroke.color = snow1;
                v.widgets.hovered.bg_fill = Color32::from_rgb(76, 86, 106);
                ctx.set_visuals(v);
            }
            ColorScheme::Monokai => {
                let mut v = Visuals::dark();
                let bg = Color32::from_rgb(39, 40, 34);
                let fg = Color32::from_rgb(248, 248, 242);
                v.panel_fill = bg;
                v.window_fill = Color32::from_rgb(49, 50, 44);
                v.extreme_bg_color = bg;
                v.faint_bg_color = Color32::from_rgb(49, 50, 44);
                v.override_text_color = Some(fg);
                v.widgets.noninteractive.bg_fill = Color32::from_rgb(49, 50, 44);
                v.widgets.noninteractive.fg_stroke.color = fg;
                ctx.set_visuals(v);
            }
            ColorScheme::Dracula => {
                let mut v = Visuals::dark();
                let bg = Color32::from_rgb(40, 42, 54);
                let current = Color32::from_rgb(68, 71, 90);
                let fg = Color32::from_rgb(248, 248, 242);
                v.panel_fill = bg;
                v.window_fill = current;
                v.extreme_bg_color = bg;
                v.faint_bg_color = current;
                v.override_text_color = Some(fg);
                v.widgets.noninteractive.bg_fill = current;
                v.widgets.noninteractive.fg_stroke.color = fg;
                v.widgets.inactive.fg_stroke.color = Color32::from_rgb(98, 114, 164);
                v.widgets.hovered.bg_fill = Color32::from_rgb(98, 114, 164);
                ctx.set_visuals(v);
            }
            ColorScheme::GruvboxDark => {
                let mut v = Visuals::dark();
                let bg = Color32::from_rgb(40, 40, 40);
                let bg1 = Color32::from_rgb(60, 56, 54);
                let fg = Color32::from_rgb(235, 219, 178);
                v.panel_fill = bg;
                v.window_fill = bg1;
                v.extreme_bg_color = bg;
                v.faint_bg_color = bg1;
                v.override_text_color = Some(fg);
                v.widgets.noninteractive.bg_fill = bg1;
                v.widgets.noninteractive.fg_stroke.color = fg;
                ctx.set_visuals(v);
            }
            ColorScheme::HighContrast => {
                let mut v = Visuals::dark();
                let bg = Color32::BLACK;
                let fg = Color32::WHITE;
                v.panel_fill = bg;
                v.window_fill = Color32::from_rgb(10, 10, 10);
                v.extreme_bg_color = bg;
                v.faint_bg_color = Color32::from_rgb(20, 20, 20);
                v.override_text_color = Some(fg);
                v.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 20, 20);
                v.widgets.noninteractive.fg_stroke.color = fg;
                ctx.set_visuals(v);
            }
            ColorScheme::Custom(custom) => {
                if let Some(visuals) = &custom.visuals {
                    ctx.set_visuals(visuals.clone());
                } else {
                    ctx.set_visuals(Visuals::dark());
                }
            }
        }

        // Always refresh the global palette so that any future calls to
        // [`TraceLook::alloc_color`] (or manual look allocation) uses the
        // colours appropriate for the newly-applied scheme.  This is also
        // important for unit tests and for programmes that query the palette
        // directly.
        set_global_palette(self.trace_colors());
    }

    /// Default trace colour palette for this scheme (up to 8 colours).
    pub fn trace_colors(&self) -> Vec<Color32> {
        match self {
            ColorScheme::Dark | ColorScheme::HighContrast => vec![
                Color32::from_rgb(31, 119, 180),
                Color32::from_rgb(255, 127, 14),
                Color32::from_rgb(44, 160, 44),
                Color32::from_rgb(214, 39, 40),
                Color32::from_rgb(148, 103, 189),
                Color32::from_rgb(140, 86, 75),
                Color32::from_rgb(227, 119, 194),
                Color32::from_rgb(127, 127, 127),
            ],
            ColorScheme::Light | ColorScheme::GgPlot => vec![
                Color32::from_rgb(228, 26, 28),
                Color32::from_rgb(55, 126, 184),
                Color32::from_rgb(77, 175, 74),
                Color32::from_rgb(152, 78, 163),
                Color32::from_rgb(255, 127, 0),
                Color32::from_rgb(166, 86, 40),
                Color32::from_rgb(247, 129, 191),
                Color32::from_rgb(153, 153, 153),
            ],
            ColorScheme::SolarizedDark | ColorScheme::SolarizedLight => vec![
                Color32::from_rgb(181, 137, 0),
                Color32::from_rgb(203, 75, 22),
                Color32::from_rgb(220, 50, 47),
                Color32::from_rgb(211, 54, 130),
                Color32::from_rgb(108, 113, 196),
                Color32::from_rgb(38, 139, 210),
                Color32::from_rgb(42, 161, 152),
                Color32::from_rgb(133, 153, 0),
            ],
            ColorScheme::Nord => vec![
                Color32::from_rgb(136, 192, 208),
                Color32::from_rgb(129, 161, 193),
                Color32::from_rgb(94, 129, 172),
                Color32::from_rgb(191, 97, 106),
                Color32::from_rgb(208, 135, 112),
                Color32::from_rgb(235, 203, 139),
                Color32::from_rgb(163, 190, 140),
                Color32::from_rgb(180, 142, 173),
            ],
            ColorScheme::Monokai => vec![
                Color32::from_rgb(249, 38, 114),
                Color32::from_rgb(166, 226, 46),
                Color32::from_rgb(253, 151, 31),
                Color32::from_rgb(102, 217, 239),
                Color32::from_rgb(174, 129, 255),
                Color32::from_rgb(230, 219, 116),
                Color32::from_rgb(248, 248, 242),
                Color32::from_rgb(117, 113, 94),
            ],
            ColorScheme::Dracula => vec![
                Color32::from_rgb(139, 233, 253),
                Color32::from_rgb(80, 250, 123),
                Color32::from_rgb(255, 184, 108),
                Color32::from_rgb(255, 121, 198),
                Color32::from_rgb(189, 147, 249),
                Color32::from_rgb(255, 85, 85),
                Color32::from_rgb(241, 250, 140),
                Color32::from_rgb(248, 248, 242),
            ],
            ColorScheme::GruvboxDark => vec![
                Color32::from_rgb(251, 73, 52),
                Color32::from_rgb(184, 187, 38),
                Color32::from_rgb(250, 189, 47),
                Color32::from_rgb(131, 165, 152),
                Color32::from_rgb(211, 134, 155),
                Color32::from_rgb(142, 192, 124),
                Color32::from_rgb(254, 128, 25),
                Color32::from_rgb(168, 153, 132),
            ],
            ColorScheme::Custom(custom) => custom.palette.clone(),
        }
    }
}
