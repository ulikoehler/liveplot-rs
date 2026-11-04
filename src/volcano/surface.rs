//! `VolcanoSurface` — renders a scope's traces through the VolcanoPlot
//! FFI into an egui texture, with pointer→viewport forwarding.

use std::collections::HashMap;

use crate::data::traces::TracesCollection;
use crate::data::scope::ScopeData;
use crate::data::trace_look::TraceLook;
use egui_plot::LineStyle;
use super::{VolcanoFigure, VolcanoLineStyle};

fn rgba8(c: egui::Color32) -> u32 {
    let [r, g, b, a] = c.to_array();
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32
}

fn vp_style(look: &TraceLook) -> VolcanoLineStyle {
    match look.style {
        LineStyle::Dashed { .. } => VolcanoLineStyle::Dashed,
        LineStyle::Dotted { .. } => VolcanoLineStyle::Dotted,
        _ if look.show_points => VolcanoLineStyle::PointsOnly,
        _ => VolcanoLineStyle::Solid,
    }
}

/// One per scope. Owns the FFI figure + the egui texture it produces.
pub struct VolcanoSurface {
    fig: Option<VolcanoFigure>,
    tex: Option<egui::TextureHandle>,
    trace_ids: HashMap<String, u64>,
    /// Current data-space viewport (x, y).
    pub bounds: ([f64; 2], [f64; 2]),
    initialized: bool,
}

impl Default for VolcanoSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl VolcanoSurface {
    pub fn new() -> Self {
        Self {
            fig: None,
            tex: None,
            trace_ids: HashMap::new(),
            bounds: ([0.0, 1.0], [0.0, 1.0]),
            initialized: false,
        }
    }

    /// Render the scope's traces into `rect`. Returns whether the scope
    /// actually rendered a frame (false when Vulkan init failed — caller
    /// falls back to egui_plot).
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        scope: &ScopeData,
        traces: &TracesCollection,
    ) -> bool {
        let w = rect.width().max(8.0) as u32;
        let h = rect.height().max(8.0) as u32;
        let fig = match &mut self.fig {
            Some(f) => f,
            None => match VolcanoFigure::new(w, h) {
                Some(f) => self.fig.insert(f),
                None => return false,
            },
        };
        let (fw, fh) = fig.size();
        if fw != w || fh != h {
            fig.resize(w, h);
        }

        // Sync trace set: add missing, update data+style.
        let mut seen = std::collections::HashSet::new();
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else { continue };
            if !tr.look.visible {
                continue;
            }
            seen.insert(name.0.clone());
            let id = *self
                .trace_ids
                .entry(name.0.clone())
                .or_insert_with(|| fig.add_trace());
            let xy: Vec<[f64; 2]> = tr
                .live
                .iter()
                .map(|p| [p[0], p[1] + tr.offset])
                .collect();
            fig.set_trace(id, &xy, rgba8(tr.look.color), tr.look.width, vp_style(&tr.look));
        }
        // Drop vanished traces.
        let dead: Vec<String> = self
            .trace_ids
            .keys()
            .filter(|k| !seen.contains(*k))
            .cloned()
            .collect();
        for k in dead {
            if let Some(id) = self.trace_ids.remove(&k) {
                fig.remove_trace(id);
            }
        }

        // Viewport: auto-fit → data extents; else stored bounds/time_window.
        let (xb, yb) = Self::viewport_for(scope, traces, self.bounds);
        self.bounds = (xb, yb);
        fig.set_viewport(xb[0], xb[1], yb[0], yb[1]);

        fig.set_axis_labels(
            scope.x_axis.name.as_deref(),
            scope.y_axis.name.as_deref(),
        );

        let Some(px) = fig.render_rgba() else {
            return false;
        };
        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], px);
        let tex = self.tex.get_or_insert_with(|| {
            ui.ctx()
                .load_texture("volcano_scope", image.clone(), egui::TextureOptions::LINEAR)
        });
        tex.set(image, egui::TextureOptions::LINEAR);
        ui.painter().image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        self.initialized = true;
        true
    }

    fn viewport_for(
        scope: &ScopeData,
        traces: &TracesCollection,
        bounds: ([f64; 2], [f64; 2]),
    ) -> ([f64; 2], [f64; 2]) {
        if !scope.x_axis.auto_fit || !scope.y_axis.auto_fit {
            return bounds;
        }
        let mut xb = [f64::INFINITY, f64::NEG_INFINITY];
        let mut yb = [f64::INFINITY, f64::NEG_INFINITY];
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else { continue };
            for p in tr.live.iter() {
                xb[0] = xb[0].min(p[0]);
                xb[1] = xb[1].max(p[0]);
                yb[0] = yb[0].min(p[1] + tr.offset);
                yb[1] = yb[1].max(p[1] + tr.offset);
            }
        }
        if !xb[0].is_finite() {
            xb = [0.0, 1.0];
        }
        if !yb[0].is_finite() {
            yb = [-1.0, 1.0];
        }
        // Time window keeps a trailing x range.
        if scope.time_window > 0.0 && scope.x_axis.auto_fit {
            xb[0] = (xb[1] - scope.time_window).max(xb[0]);
        }
        (xb, yb)
    }

    /// Forward pointer input: scroll = zoom, primary drag = pan,
    /// secondary drag = box zoom. `resp` is the interact response for
    /// the plot rect.
    pub fn handle_input(&mut self, resp: &egui::Response, rect: egui::Rect) {
        let (xb, yb) = self.bounds;
        if let Some(hover) = resp.hover_pos() {
            let scroll = resp.ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let fx = (hover.x - rect.left()) / rect.width().max(1.0);
                let fy = (hover.y - rect.top()) / rect.height().max(1.0);
                let cx = xb[0] + f64::from(fx) * (xb[1] - xb[0]);
                let cy = yb[1] - f64::from(fy) * (yb[1] - yb[0]);
                let z = (1.0 - f64::from(scroll) * 0.002).clamp(0.5, 2.0);
                self.bounds = (
                    [cx - (cx - xb[0]) * z, cx + (xb[1] - cx) * z],
                    [cy - (cy - yb[0]) * z, cy + (yb[1] - cy) * z],
                );
            }
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            let dx = -f64::from(d.x) / f64::from(rect.width()) * (xb[1] - xb[0]);
            let dy = f64::from(d.y) / f64::from(rect.height()) * (yb[1] - yb[0]);
            self.bounds = ([xb[0] + dx, xb[1] + dx], [yb[0] + dy, yb[1] + dy]);
        }
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }
}

impl std::fmt::Debug for VolcanoSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolcanoSurface")
            .field("initialized", &self.initialized)
            .finish()
    }
}
