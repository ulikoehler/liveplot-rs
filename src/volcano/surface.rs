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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn style_bits(look: &TraceLook) -> u32 {
    let s = match look.style {
        LineStyle::Dashed { .. } => 1,
        LineStyle::Dotted { .. } => 2,
        _ => 0,
    };
    s | ((look.show_points as u32) << 8)
}

/// Signature of everything that feeds one vp trace — data head/tail
/// (eviction + appends), look, offset, log transform.
fn trace_sig(
    pts: &std::collections::VecDeque<[f64; 2]>,
    look: &TraceLook,
    offset: f64,
    log_xy: (bool, bool),
) -> u64 {
    let mut h = DefaultHasher::new();
    pts.len().hash(&mut h);
    if let Some(p) = pts.front() {
        p[0].to_bits().hash(&mut h);
        p[1].to_bits().hash(&mut h);
    }
    if let Some(p) = pts.back() {
        p[0].to_bits().hash(&mut h);
        p[1].to_bits().hash(&mut h);
    }
    offset.to_bits().hash(&mut h);
    look.color.to_array().hash(&mut h);
    look.width.to_bits().hash(&mut h);
    style_bits(look).hash(&mut h);
    look.visible.hash(&mut h);
    log_xy.hash(&mut h);
    h.finish()
}

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
    /// Last pushed data signature per trace key — skips re-uploads.
    trace_sigs: HashMap<String, u64>,
    /// Signature of the whole render input — skips the Vulkan render
    /// + readback + texture upload when nothing changed.
    last_sig: u64,
    /// In-flight render+readback on a worker thread — keeps the UI at
    /// 60fps while Vulkan renders (~7ms/frame) in the background.
    pending: Option<
        std::thread::JoinHandle<
            (Option<VolcanoFigure>, Option<(u32, u32, Vec<u8>)>),
        >,
    >,
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
            trace_sigs: HashMap::new(),
            last_sig: 0,
            pending: None,
            bounds: ([0.0, 1.0], [0.0, 1.0]),
            initialized: false,
        }
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// Sync + render + upload into the egui texture. Painting the
    /// texture is the caller's job (see `texture_id()`). Returns
    /// false when Vulkan is unavailable → caller falls back.
    pub fn sync_and_render(
        &mut self,
        ctx: &egui::Context,
        scope: &ScopeData,
        traces: &TracesCollection,
        w: u32,
        h: u32,
        dark: bool,
        panel_bg: egui::Color32,
    ) -> bool {
        let w = w.max(8);
        let h = h.max(8);

        // Collect a finished render: upload the frame + reclaim fig.
        // First render blocks once (~7ms) so callers waiting for real
        // output aren't left with a blank texture; afterwards renders
        // pipeline 1-frame behind — the UI stays at 60fps.
        if let Some(handle) = self.pending.take() {
            if !self.initialized || handle.is_finished() {
                if let Ok((fig, px)) = handle.join() {
                    self.fig = fig;
                    if let Some((rw, rh, px)) = px {
                        // rw/rh are the RENDERED size — may lag the
                        // current w/h if a resize raced this frame.
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [rw as usize, rh as usize],
                            &px,
                        );
                        let tex = self.tex.get_or_insert_with(|| {
                            ctx.load_texture(
                                "volcano_scope",
                                image.clone(),
                                egui::TextureOptions::LINEAR,
                            )
                        });
                        tex.set(image, egui::TextureOptions::LINEAR);
                        self.initialized = true;
                    }
                }
            } else {
                // Worker still busy — paint last frame, skip this one.
                self.pending = Some(handle);
                return true;
            }
        }

        let fig = match &mut self.fig {
            Some(f) => f,
            None => match VolcanoFigure::new(w, h) {
                Some(f) => self.fig.insert(f),
                None => return false,
            },
        };
        // Whole-render signature — if identical to last frame the
        // texture is still current: skip the Vulkan render + readback
        // + upload entirely.
        let log_xy = (scope.x_axis.log_scale, scope.y_axis.log_scale);
        let mut sh = DefaultHasher::new();
        w.hash(&mut sh);
        h.hash(&mut sh);
        self.bounds.0[0].to_bits().hash(&mut sh);
        self.bounds.0[1].to_bits().hash(&mut sh);
        self.bounds.1[0].to_bits().hash(&mut sh);
        self.bounds.1[1].to_bits().hash(&mut sh);
        dark.hash(&mut sh);
        log_xy.hash(&mut sh);
        scope.paused.hash(&mut sh);
        std::mem::discriminant(&scope.scope_type).hash(&mut sh);
        let mut frame_sigs: Vec<(String, u64)> = Vec::new();
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else { continue };
            if !tr.look.visible {
                continue;
            }
            let sig = trace_sig(&tr.live, &tr.look, tr.offset, log_xy);
            name.0.hash(&mut sh);
            sig.hash(&mut sh);
            frame_sigs.push((name.0.clone(), sig));
        }
        if scope.scope_type == ScopeType::XYScope {
            for (xn, yn, pl) in scope.xy_pairs.iter() {
                let (Some(xn), Some(yn)) = (xn, yn) else { continue };
                if !pl.visible {
                    continue;
                }
                let (Some(xt), Some(yt)) =
                    (traces.get_trace(xn), traces.get_trace(yn))
                else {
                    continue;
                };
                let a = trace_sig(&xt.live, pl, xt.offset, (false, false));
                let b = trace_sig(&yt.live, pl, yt.offset, (false, false));
                format!("xy:{}:{}", xn.0, yn.0).hash(&mut sh);
                a.hash(&mut sh);
                b.hash(&mut sh);
            }
        }
        let sig = sh.finish();
        if self.initialized && sig == self.last_sig {
            return true;
        }
        self.last_sig = sig;

        let (fw, fh) = fig.size();
        // Debounce: resizing rebuilds the whole Vulkan backend —
        // don't do it for small layout jitter (a few % stretch is
        // invisible); only when the figure is clearly wrong-sized.
        let dw = (w as i32 - fw as i32).abs();
        let dh = (h as i32 - fh as i32).abs();
        if dw > 32 || dh > 32 {
            fig.resize(w, h);
        }

        // Sync trace set: add missing, update data+style.
        let mut seen = std::collections::HashSet::new();
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else { continue };
            if !tr.look.visible {
                continue;
            }
            // Skip before add_trace — an empty LinePlot breaks
            // VolcanoPlot's renderFrame (and not marking it `seen`
            // lets the dead-trace cleanup remove a stale plot).
            if tr.live.is_empty() {
                continue;
            }
            seen.insert(name.0.clone());
            let id = *self
                .trace_ids
                .entry(name.0.clone())
                .or_insert_with(|| fig.add_trace());
            let (x_log, y_log) = log_xy;
            let new_sig = frame_sigs
                .iter()
                .find(|(k, _)| k == &name.0)
                .map(|(_, s)| *s)
                .unwrap_or(0);
            if self.trace_sigs.get(&name.0) == Some(&new_sig) {
                continue;
            }
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
            self.trace_sigs.insert(name.0.clone(), new_sig);
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
            self.trace_sigs.remove(&k);
        }

        // Until the caller writes back real plot bounds, auto-fit data
        // (or honor the trailing time window).
        if !self.initialized {
            self.bounds = Self::fit_bounds(scope, traces, self.bounds);
        }
        let (xb, yb) = self.bounds;
        fig.set_viewport(xb[0], xb[1], yb[0], yb[1]);
        fig.set_background(!dark);
        fig.set_background_rgba(
            panel_bg.r() as f32 / 255.0,
            panel_bg.g() as f32 / 255.0,
            panel_bg.b() as f32 / 255.0,
        );
        fig.set_axis_labels(scope.x_axis.name.as_deref(), scope.y_axis.name.as_deref());

        // Render + readback on a worker thread — the expensive part
        // (prepare/renderFrame/readback ~7ms) never blocks the UI.
        let mut owned = self.fig.take().unwrap();
        self.pending = Some(std::thread::spawn(move || {
            let (rw, rh) = owned.size();
            let px = owned.render_rgba().map(|b| (rw, rh, b.to_vec()));
            (Some(owned), px)
        }));
        true
    }

    /// Texture with the last rendered frame, for the caller to paint.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.tex.as_ref().map(|t| t.id())
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
