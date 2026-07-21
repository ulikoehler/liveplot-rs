//! TraceLook: visual styling for plot traces.

use eframe::egui::Color32;
use egui_plot::{LineStyle, MarkerShape};

/// The visual presentation of a trace (color, visibility, line style, markers).
#[derive(Debug, Clone)]
pub struct TraceLook {
    pub color: Color32,
    pub visible: bool,
    pub width: f32,
    pub show_points: bool,
    pub highlight_newest_point: bool,
    pub style: LineStyle,
    pub point_size: f32,
    pub marker: MarkerShape,
}

impl Default for TraceLook {
    fn default() -> Self {
        Self {
            color: Color32::GRAY,
            visible: true,
            width: 1.5,
            show_points: false,
            highlight_newest_point: false,
            style: LineStyle::Solid,
            point_size: 4.0,
            marker: MarkerShape::Circle,
        }
    }
}

impl TraceLook {
    /// Create a new TraceLook with a color allocated based on the trace index.
    pub fn new(index: usize) -> Self {
        Self {
            color: Self::alloc_color(index),
            ..Default::default()
        }
    }

    /// Allocate a distinct color for the given trace index.
    pub fn alloc_color(index: usize) -> Color32 {
        // Consult the global colour palette, which is kept in sync with the
        // currently-applied `ColorScheme`.  Fallback to gray if something went
        // wrong (empty palette or poisoned lock).
        let palette = crate::color_scheme::global_palette();
        if palette.is_empty() {
            Color32::GRAY
        } else {
            palette[index % palette.len()]
        }
    }
}

