//! Marker: a single-point annotation drawn on scope plots.
//!
//! A marker is like a [`Measurement`](crate::data::measurement::Measurement) but
//! holds exactly one point instead of two.  It can be rendered as a point (dot),
//! a horizontal line, a vertical line, or any combination of those, and can
//! optionally snap to a trace ("catch trace") when placed.

use crate::TraceRef;
use egui::Color32;
use egui_plot::MarkerShape;

/// A single-point marker shown on all scopes of a [`crate::LivePlotPanel`].
#[derive(Debug, Clone)]
pub struct Marker {
    /// Display name (shown in the panel list and in the plot legend).
    pub name: String,
    /// The marker position in plot coordinates (log-transformed like
    /// measurement points), or `None` when not placed yet.
    pub point: Option<[f64; 2]>,
    /// Display color.
    pub color: Color32,
    /// Whether the marker is drawn at all.
    pub visible: bool,
    /// Draw the dot at the marker point.
    pub show_point: bool,
    /// Draw a horizontal line through the marker's y value.
    pub show_hline: bool,
    /// Draw a vertical line through the marker's x value.
    pub show_vline: bool,
    /// Shape used for the dot.
    pub shape: MarkerShape,
    /// Optional trace the marker snaps to when placed by clicking.
    pub catch_trace: Option<TraceRef>,
    /// Id of the scope where the marker point was placed (used for
    /// axis-aware value formatting).
    pub scope_id: Option<usize>,
}

impl Marker {
    /// Create a new marker with the given name and a palette color derived
    /// from `index` (same allocation as traces).
    pub fn new(name: &str, index: usize) -> Self {
        Self {
            name: name.to_string(),
            point: None,
            color: crate::data::trace_look::TraceLook::alloc_color(index),
            visible: true,
            show_point: true,
            show_hline: false,
            show_vline: true,
            shape: MarkerShape::Circle,
            catch_trace: None,
            scope_id: None,
        }
    }

    /// Clear the marker position (keeps name/color/style).
    pub fn clear(&mut self) {
        self.point = None;
        self.scope_id = None;
    }

    /// Whether the marker currently has a position.
    pub fn is_placed(&self) -> bool {
        self.point.is_some()
    }
}
