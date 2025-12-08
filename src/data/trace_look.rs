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
        const PALETTE: [Color32; 10] = [
            Color32::from_rgb(31, 119, 180),
            Color32::from_rgb(255, 127, 14),
            Color32::from_rgb(44, 160, 44),
            Color32::from_rgb(214, 39, 40),
            Color32::from_rgb(148, 103, 189),
            Color32::from_rgb(140, 86, 75),
            Color32::from_rgb(227, 119, 194),
            Color32::from_rgb(127, 127, 127),
            Color32::from_rgb(188, 189, 34),
            Color32::from_rgb(23, 190, 207),
        ];
        PALETTE[index % PALETTE.len()]
    }
}
