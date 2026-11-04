//! `VolcanoSurface` — renders a scope's traces through the VolcanoPlot
//! FFI into an egui texture painted *underneath* the (transparent)
//! egui_plot layer, which keeps owning interaction, overlays and
//! `PlotMemory`. Viewport sync: the app writes `bounds` back from the
//! plot transform each frame; the surface only auto-fits until the
//! first painted frame.

use std::collections::HashMap;

use crate::data::scope::{ScopeData, ScopeType};
use crate::data::trace_look::TraceLook;
use crate::data::traces::TracesCollection;
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
    /// Current data-space viewport (x, y) — written back from the
    /// plot transform by the caller after each frame.
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

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// Render the scope's traces into `rect` (paint only — the
    /// transparent egui plot layered on top owns all interaction).
    /// Returns false when Vulkan is unavailable → caller falls back.
    pub fn paint(
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
            let x_log = scope.x_axis.log_scale;
            let y_log = scope.y_axis.log_scale;
            let xy: Vec<[f64; 2]> = tr
                .live
                .iter()
                .map(|p| {
                    let x = if x_log && p[0] > 0.0 { p[0].log10() } else { p[0] };
                    let y = if y_log { (p[1] + tr.offset).log10() } else { p[1] + tr.offset };
                    [x, if y.is_finite() { y } else { f64::NAN }]
                })
                .collect();
            fig.set_trace(id, &xy, rgba8(tr.look.color), tr.look.width, vp_style(&tr.look));
        }
        // XY-scope pairs: join x/y traces on timestamp within 1e-9.
        if scope.scope_type == ScopeType::XYScope {
            const TOL: f64 = 1e-9;
            for (x_name, y_name, pair_look) in scope.xy_pairs.iter() {
                let (Some(x_name), Some(y_name)) = (x_name, y_name) else {
                    continue;
                };
                if !pair_look.visible {
                    continue;
                }
                let (Some(x_tr), Some(y_tr)) =
                    (traces.get_trace(x_name), traces.get_trace(y_name))
                else {
                    continue;
                };
                let (Some(x_pts), Some(y_pts)) = (
                    traces.get_points_ref(x_name, scope.paused),
                    traces.get_points_ref(y_name, scope.paused),
                ) else {
                    continue;
                };
                let mut derived: Vec<[f64; 2]> = Vec::new();
                let (mut i, mut j) = (0usize, 0usize);
                while i < x_pts.len() && j < y_pts.len() {
                    let tx = x_pts[i][0];
                    let ty = y_pts[j][0];
                    let dt = tx - ty;
                    if dt.abs() <= TOL {
                        derived.push([x_pts[i][1] + x_tr.offset,
                                      y_pts[j][1] + y_tr.offset]);
                        i += 1;
                        j += 1;
                    } else if dt < 0.0 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
                let key = format!("xy:{}:{}", x_name.0, y_name.0);
                seen.insert(key.clone());
                let id = *self
                    .trace_ids
                    .entry(key)
                    .or_insert_with(|| fig.add_trace());
                fig.set_trace(id, &derived, rgba8(pair_look.color),
                              pair_look.width, vp_style(pair_look));
            }
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

        // Until the caller writes back real plot bounds, auto-fit data
        // (or honor the trailing time window).
        if !self.initialized {
            self.bounds = Self::fit_bounds(scope, traces, self.bounds);
        }
        let (xb, yb) = self.bounds;
        fig.set_viewport(xb[0], xb[1], yb[0], yb[1]);
        fig.set_background(!ui.visuals().dark_mode);
        fig.set_axis_labels(scope.x_axis.name.as_deref(), scope.y_axis.name.as_deref());

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

    fn fit_bounds(
        scope: &ScopeData,
        traces: &TracesCollection,
        bounds: ([f64; 2], [f64; 2]),
    ) -> ([f64; 2], [f64; 2]) {
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
            xb = bounds.0;
        }
        if !yb[0].is_finite() {
            yb = bounds.1;
        }
        if scope.time_window > 0.0 && scope.x_axis.auto_fit {
            xb[0] = (xb[1] - scope.time_window).max(xb[0]);
        }
        (xb, yb)
    }
}

impl std::fmt::Debug for VolcanoSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolcanoSurface")
            .field("initialized", &self.initialized)
            .finish()
    }
}
