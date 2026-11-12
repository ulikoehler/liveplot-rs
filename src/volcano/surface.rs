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
        | ((look.marker as u32) << 12)
        | (look.point_size.to_bits() & 0xFFFF) << 16
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

/// Slide-detection state for the ring-append upload: mutation seq of
/// the last uploaded window plus the transform params the GPU copy was
/// baked with — any param/gen change forces a full re-upload.
struct SlideState {
    len: usize,
    uploaded: usize,
    gen: u64,
    pushes: u64,
    pops: u64,
    last: [f64; 2],
    params: u64,
}

fn rgba8(c: egui::Color32) -> u32 {
    let [r, g, b, a] = c.to_array();
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32
}

fn vp_style(look: &TraceLook) -> VolcanoLineStyle {
    match look.style {
        LineStyle::Dashed { .. } => VolcanoLineStyle::Dashed,
        LineStyle::Dotted { .. } => VolcanoLineStyle::Dotted,
        _ => VolcanoLineStyle::Solid,
    }
}

/// egui `MarkerShape` → `plot::MarkerStyle` ordinal; `MARKER_NONE`
/// when `show_points` is off (egui draws points only on request too).

/// Hashable hover-state tag for `trace_sig` — 0 no hover set, 1
/// hovered/emphasized, 2 dimmed.
fn hover_state_bits(names: &[crate::data::traces::TraceRef], traces: &TracesCollection) -> u8 {
    match &traces.hover_trace {
        Some(hov) if names.iter().all(|n| !hov.contains(n)) => 2,
        Some(_) => 1,
        None => 0,
    }
}

/// egui-path hover emphasis (scope_ui.rs): when `traces.hover_trace`
/// is set, non-hovered traces dim to alpha 40 and hovered traces get
/// width x1.6. Returns (rgba, width).
fn hover_look(
    look: &TraceLook,
    names: Option<&[crate::data::traces::TraceRef]>,
    traces: &TracesCollection,
) -> (u32, f32) {
    let mut color = rgba8(look.color);
    let mut width = look.width;
    if let (Some(hov), Some(names)) = (&traces.hover_trace, names) {
        if names.iter().all(|n| !hov.contains(n)) {
            color = (color & 0xFFFF_FF00) | 40;
        } else {
            width = (width * 1.6).max(width + 1.0);
        }
    }
    (color, width)
}

fn vp_marker(look: &TraceLook) -> i32 {
    use egui_plot::MarkerShape;
    if !look.show_points {
        return 40; // MarkerStyle::None
    }
    match look.marker {
        MarkerShape::Circle => 1,
        MarkerShape::Diamond => 3,
        MarkerShape::Square => 2,
        MarkerShape::Cross => 14, // 'x' (stroked)
        MarkerShape::Plus => 13,  // '+' (stroked)
        MarkerShape::Up => 5,     // '^'
        MarkerShape::Down => 6,   // 'v'
        MarkerShape::Left => 7,   // '<'
        MarkerShape::Right => 8,  // '>'
        MarkerShape::Asterisk => 17,
    }
}

/// Long-lived render thread — one channel'd job slot (figure + a
/// recycled pixel scratch) in, one result out. Replaces per-frame
/// `thread::spawn`: thread creation + teardown per rendered frame is
/// real CPU.
struct RenderWorker {
    job_tx: std::sync::mpsc::Sender<(VolcanoFigure, Vec<u8>)>,
    res_rx: std::sync::mpsc::Receiver<(VolcanoFigure, Option<egui::ColorImage>)>,
    handle: std::thread::JoinHandle<()>,
    /// A job is in flight — paint the last texture meanwhile.
    busy: bool,
}

/// `Color32` is `repr(C) [u8;4]` in RGBA order — a flat copy is identical
/// to `ColorImage::from_rgba_unmultiplied`'s per-pixel map, but ~4x faster
/// on multi-MB frames.
fn color_image_from_rgba(size: [usize; 2], rgba: &[u8]) -> egui::ColorImage {
    debug_assert_eq!(rgba.len() % 4, 0);
    let mut pixels = Vec::<egui::Color32>::with_capacity(rgba.len() / 4);
    unsafe {
        std::ptr::copy_nonoverlapping(
            rgba.as_ptr(),
            pixels.as_mut_ptr().cast::<u8>(),
            rgba.len(),
        );
        pixels.set_len(rgba.len() / 4);
    }
    egui::ColorImage::new(size, pixels)
}

impl RenderWorker {
    fn spawn() -> Self {
        let (job_tx, job_rx) = std::sync::mpsc::channel::<(VolcanoFigure, Vec<u8>)>();
        let (res_tx, res_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            while let Ok((mut fig, mut px)) = job_rx.recv() {
                // Build the ColorImage here — keeps both the readback
                // and the rgba→Color32 conversion off the UI thread.
                // The view path borrows the C++ staging buffer (one
                // copy total); render_rgba_into is the fallback.
                let res = match fig.render_rgba_view() {
                    Some((rw, rh, view)) => Some(color_image_from_rgba(
                        [rw as usize, rh as usize],
                        view,
                    )),
                    None => fig.render_rgba_into(&mut px).map(|(rw, rh)| {
                        color_image_from_rgba([rw as usize, rh as usize], &px)
                    }),
                };
                if res_tx.send((fig, res)).is_err() {
                    return;
                }
            }
        });
        Self {
            job_tx,
            res_rx,
            handle,
            busy: false,
        }
    }

    /// Hand the figure + recycled pixel scratch to the worker.
    fn dispatch(&mut self, fig: VolcanoFigure, px: Vec<u8>) -> bool {
        if self.job_tx.send((fig, px)).is_ok() {
            self.busy = true;
            true
        } else {
            false
        }
    }
}

/// One per scope. Owns the FFI figure + the egui texture it produces.
pub struct VolcanoSurface {
    fig: Option<VolcanoFigure>,
    tex: Option<egui::TextureHandle>,
    trace_ids: HashMap<String, u64>,
    /// Last pushed data signature per trace key — skips re-uploads.
    trace_sigs: HashMap<String, u64>,
    /// Scratch upload buffers — reused across frames so the per-frame
    /// point collect doesn't allocate O(N) fresh Vecs (alloc churn is
    /// a real UI-thread stutter source on live data).
    upload_scratch: HashMap<String, Vec<[f64; 2]>>,
    slide_state: HashMap<String, SlideState>,
    /// Signature of the whole render input — skips the Vulkan render
    /// + readback + texture upload when nothing changed.
    last_sig: u64,
    /// Persistent render+readback worker — keeps the UI at full rate
    /// while Vulkan renders (~2-7ms/frame) in the background.
    worker: Option<RenderWorker>,
    /// Recycled w*h*4 pixel buffer — travels to the worker and back,
    /// so rendered frames don't allocate/copy a fresh Vec each time.
    pixel_scratch: Vec<u8>,
    /// Current data-space viewport (x, y) — the PADDED view bounds
    /// (data bounds + 5 % per side, matching the egui transform),
    /// written back from the caller each frame.
    pub bounds: ([f64; 2], [f64; 2]),
    /// True when the caller owns `bounds` — fit_bounds defers to it
    /// (without this the first-frame fit clobbered the real viewport,
    /// which is what made early frames render the full data range).
    pub bounds_external: bool,
    /// Data→viewport translate offsets — VolcanoPlot's viewport is f32,
    /// so huge data magnitudes (e.g. epoch timestamps ~1.79e9) lose all
    /// sub-window precision. Shift data and bounds by the same offset
    /// so the plotted range stays in representable f32 space.
    shift: (f64, f64),
    /// Persistent shift reference — set ONCE when the bounds first
    /// become huge, so the label space is `x - ref` = elapsed values
    /// that ADVANCE with the data (recomputing to xb0 every frame
    /// froze the axis at 0..window — looked like no autoscroll).
    shift_ref: Option<(f64, f64)>,
    /// Frames rendered+uploaded so far — parity-test observability.
    pub frames_rendered: u64,
    /// FFI frames dropped on size-mismatch readback — parity-test
    /// observability (`ScopeData::volcano_dropped_frames` mirrors).
    pub dropped_frames: u32,
    /// Perf observability: dispatches sent to the worker, frames that
    /// reached signature computation, frames skipped while the worker
    /// was busy, traces re-uploaded.
    pub dbg_dispatches: u64,
    pub dbg_sig_frames: u64,
    pub dbg_busy_skips: u64,
    pub dbg_uploads: u64,
    pub dbg_slides: u64,
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
            upload_scratch: HashMap::new(),
            slide_state: HashMap::new(),
            last_sig: 0,
            worker: None,
            pixel_scratch: Vec::new(),
            bounds: ([0.0, 1.0], [0.0, 1.0]),
            bounds_external: false,
            shift: (0.0, 0.0),
            shift_ref: None,
            frames_rendered: 0,
            dropped_frames: 0,
            dbg_dispatches: 0,
            dbg_sig_frames: 0,
            dbg_busy_skips: 0,
            dbg_uploads: 0,
            dbg_slides: 0,
            initialized: false,
        }
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// Sync + render + upload into the egui texture. Painting the
    /// texture is the caller's job (see `texture_id()`). `margins` is
    /// the pixel margin table `(left, right, bottom, top)` the caller
    /// also uses for its overlay axes-rect — one source of truth keeps
    /// the figure's axes aligned with the overlay transform. Returns
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
        margins: (f32, f32, f32, f32),
    ) -> bool {
        let w = w.max(8);
        let h = h.max(8);

        // Collect a finished render: upload the frame + reclaim fig.
        // First render blocks once (~2-7ms) so callers waiting for real
        // output aren't left with a blank texture; afterwards renders
        // pipeline behind — the UI paints the last completed texture.
        enum Poll {
            Done(VolcanoFigure, Option<egui::ColorImage>),
            Busy,
            Dead,
        }
        let mut worker_dead = false;
        if let Some(w) = &mut self.worker {
            if w.busy {
                let poll = if !self.initialized {
                    match w.res_rx.recv() {
                        Ok((f, px)) => Poll::Done(f, px),
                        Err(_) => Poll::Dead,
                    }
                } else {
                    match w.res_rx.try_recv() {
                        Ok((f, px)) => Poll::Done(f, px),
                        Err(std::sync::mpsc::TryRecvError::Empty) => Poll::Busy,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => Poll::Dead,
                    }
                };
                match poll {
                    Poll::Done(fig, image) => {
                        w.busy = false;
                        self.fig = Some(fig);
                        if let Some(image) = image {
                            // The image is the RENDERED size — may lag
                            // the current w/h if a resize raced this
                            // frame; tex.set handles a size change.
                            let tex = self.tex.get_or_insert_with(|| {
                                ctx.load_texture(
                                    "volcano_scope",
                                    image.clone(),
                                    egui::TextureOptions::LINEAR,
                                )
                            });
                            tex.set(image, egui::TextureOptions::LINEAR);
                            self.initialized = true;
                            self.frames_rendered += 1;
                        } else {
                            self.dropped_frames += 1;
                        }
                    }
                    // Worker still busy — paint last frame, skip this one.
                    Poll::Busy => {
                        self.dbg_busy_skips += 1;
                        return true;
                    }
                    Poll::Dead => worker_dead = true,
                }
            }
        }
        if worker_dead {
            self.worker = None;
        }

        let fig = match &mut self.fig {
            Some(f) => f,
            None => match VolcanoFigure::new(w, h) {
                Some(f) => self.fig.insert(f),
                None => return false,
            },
        };
        self.dbg_sig_frames += 1;
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
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else {
                continue;
            };
            if !tr.look.visible {
                continue;
            }
            let sig = traces
                .get_points_ref(name, scope.paused)
                .map(|p| trace_sig(p, &tr.look, tr.offset, log_xy))
                .unwrap_or(0);
            name.0.hash(&mut sh);
            sig.hash(&mut sh);
        }
        if scope.scope_type == ScopeType::XYScope {
            for (xn, yn, pl) in scope.xy_pairs.iter() {
                let (Some(xn), Some(yn)) = (xn, yn) else {
                    continue;
                };
                if !pl.visible {
                    continue;
                }
                let (Some(xt), Some(yt)) = (traces.get_trace(xn), traces.get_trace(yn)) else {
                    continue;
                };
                let a = traces
                    .get_points_ref(xn, scope.paused)
                    .map(|p| trace_sig(p, pl, xt.offset, log_xy))
                    .unwrap_or(0);
                let b = traces
                    .get_points_ref(yn, scope.paused)
                    .map(|p| trace_sig(p, pl, yt.offset, log_xy))
                    .unwrap_or(0);
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

        // f32-precision-safe translate: when the bounds magnitude is
        // huge relative to its span (epoch timestamps ~1.79e9), shift
        // data+viewport by the same offset so the plotted range sits
        // in representable f32 space. MUST be computed BEFORE the
        // trace uploads below — the shifted data lands off-screen
        // otherwise (empty scope, axes only).
        let (xb, yb) = self.bounds;
        let x_span = (xb[1] - xb[0]).abs().max(1e-12);
        let y_span = (yb[1] - yb[0]).abs().max(1e-12);
        // Persistent ref: first time the bounds look huge, anchor the
        // shift there — the viewport then shows `x - ref` (elapsed
        // values advancing with the stream, like the egui axis labels).
        // Re-anchor if the data drifts so far the ref stops helping.
        let needs_x = xb[0].abs() > x_span * 1e4 || xb[1].abs() > x_span * 1e4;
        let needs_y = yb[0].abs() > y_span * 1e4 || yb[1].abs() > y_span * 1e4;
        if needs_x && self.shift_ref.is_none() {
            self.shift_ref = Some((xb[0], 0.0));
        }
        if needs_y {
            self.shift_ref = Some((self.shift_ref.map_or(0.0, |r| r.0), yb[0]));
        }
        if let Some(r) = self.shift_ref {
            // Re-anchor when the bounds move too far from the ref —
            // the shifted range must stay f32-safe.
            if !needs_x && !needs_y {
                self.shift = (0.0, 0.0);
            } else {
                self.shift = (
                    if needs_x && (xb[0] - r.0).abs() > x_span * 1e7 {
                        xb[0]
                    } else {
                        r.0
                    },
                    if needs_y && (yb[0] - r.1).abs() > y_span * 1e7 {
                        yb[0]
                    } else {
                        r.1
                    },
                );
            }
        }
        let (sx, sy) = self.shift;
        // A changed shift must force re-upload — fold it into the
        // per-trace sig so the stale unshifted data gets replaced.
        let shift_bits = (self.shift.0.to_bits() ^ self.shift.1.to_bits()) as u64;

        // Sync trace set: add missing, update data+style.
        let mut seen = std::collections::HashSet::new();
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else {
                continue;
            };
            if !tr.look.visible {
                continue;
            }
            // Skip before add_trace — an empty LinePlot breaks
            // VolcanoPlot's renderFrame (and not marking it `seen`
            // lets the dead-trace cleanup remove a stale plot).
            let Some(pts) = traces.get_points_ref(name, scope.paused) else {
                continue;
            };
            if pts.is_empty() {
                continue;
            }
            seen.insert(name.0.clone());
            let id = *self
                .trace_ids
                .entry(name.0.clone())
                .or_insert_with(|| fig.add_trace());
            let (x_log, y_log) = log_xy;
            let new_sig = traces
                .get_points_ref(name, scope.paused)
                .map(|p| trace_sig(p, &tr.look, tr.offset, log_xy))
                .unwrap_or(0)
                ^ shift_bits
                // hover emphasis changes effective color/width — must
                // re-upload even when data/style are unchanged.
                ^ hover_state_bits(std::slice::from_ref(name), traces) as u64;
            if self.trace_sigs.get(&name.0) == Some(&new_sig) {
                continue;
            }
            // Params the uploaded data is baked with — a change
            // invalidates the GPU copy even for a pure slide.
            let mut params_h = DefaultHasher::new();
            x_log.hash(&mut params_h);
            y_log.hash(&mut params_h);
            tr.offset.to_bits().hash(&mut params_h);
            sx.to_bits().hash(&mut params_h);
            sy.to_bits().hash(&mut params_h);
            let params = params_h.finish();
            // Slide detection via the store's mutation counters:
            // pushes/pops since the last upload give drop + tail
            // exactly, O(1). The kept-tail anchor (new[kept-1] ==
            // old.last) double-checks that the retained suffix really
            // is the old window's tail — anything else falls back to
            // a full upload.
            let seq = traces.get_points_seq(name, scope.paused);
            let slide = seq.and_then(|(_, gen, pushes, pops)| {
                let s = self.slide_state.get(&name.0)?;
                if s.params != params
                    || s.gen != gen
                    || s.uploaded != s.len
                    || pts.len() > u32::MAX as usize
                {
                    return None;
                }
                let d = usize::try_from(pops.checked_sub(s.pops)?).ok()?;
                let tail_n = usize::try_from(pushes.checked_sub(s.pushes)?).ok()?;
                let kept = s.len.checked_sub(d)?;
                (tail_n > 0
                    && kept <= pts.len()
                    && pts.len() - kept == tail_n
                    && (kept == 0 || pts.get(kept.wrapping_sub(1)) == Some(&s.last)))
                    .then_some((d.min(u32::MAX as usize) as u32, kept))
            });
            let xform = |p: &[f64; 2]| {
                let x = if x_log && p[0] > 0.0 {
                    p[0].log10()
                } else {
                    p[0] - sx
                };
                let y = if y_log {
                    (p[1] + tr.offset).log10()
                } else {
                    p[1] - sy + tr.offset
                };
                (x.is_finite() && y.is_finite()).then_some([x, y])
            };
            let xy = self.upload_scratch.entry(name.0.clone()).or_default();
            xy.clear();
            let (color, width) = hover_look(&tr.look, Some(std::slice::from_ref(name)), traces);
            let mut uploaded = 0usize;
            let slid = slide.is_some_and(|(d, kept)| {
                xy.extend(pts.iter().skip(kept).filter_map(xform));
                if xy.len() != pts.len() - kept {
                    return false; // non-finite tail points — full upload
                }
                self.dbg_uploads += 1;
                self.dbg_slides += 1;
                fig.set_trace_slide(id, d, &xy, color, width, vp_style(&tr.look));
                uploaded = pts.len();
                true
            });
            if !slid {
                xy.clear();
                xy.extend(pts.iter().filter_map(xform));
                uploaded = xy.len();
                self.dbg_uploads += 1;
                fig.set_trace(id, &xy, color, width, vp_style(&tr.look));
            }
            fig.set_marker(id, vp_marker(&tr.look), tr.look.point_size * 2.0);
            if let Some((_, gen, pushes, pops)) = seq {
                self.slide_state.insert(
                    name.0.clone(),
                    SlideState {
                        len: pts.len(),
                        uploaded,
                        gen,
                        pushes,
                        pops,
                        last: *pts.back().unwrap_or(&[0.0; 2]),
                        params,
                    },
                );
            }
            self.trace_sigs.insert(name.0.clone(), new_sig);
        }
        // XY-scope pairs: join x/y traces on timestamp within 1e-9 —
        // same merge as the egui draw loop (`join_xy_points`), then
        // the same per-axis transform: offset first, log10 of the
        // (offset) value when the axis is log, otherwise the f32-safe
        // shift. The egui path drops non-positive log values to NaN.
        if scope.scope_type == ScopeType::XYScope {
            const TOL: f64 = 1e-9;
            let (x_log, y_log) = log_xy;
            for (x_name, y_name, pair_look) in scope.xy_pairs.iter() {
                let (Some(x_name), Some(y_name)) = (x_name, y_name) else {
                    continue;
                };
                if !pair_look.visible {
                    continue;
                }
                let (Some(x_tr), Some(y_tr)) = (traces.get_trace(x_name), traces.get_trace(y_name))
                else {
                    continue;
                };
                let (Some(x_pts), Some(y_pts)) = (
                    traces.get_points_ref(x_name, scope.paused),
                    traces.get_points_ref(y_name, scope.paused),
                ) else {
                    continue;
                };
                let joined = TracesCollection::join_xy_points(x_pts, y_pts, TOL);
                let key = format!("xy:{}:{}", x_name.0, y_name.0);
                if joined.is_empty() {
                    // egui draws nothing for an empty join — drop the
                    // stale vp trace rather than leave the last frame
                    // lingering.
                    if let Some(id) = self.trace_ids.remove(&key) {
                        fig.remove_trace(id);
                    }
                    self.trace_sigs.remove(&key);
                    self.slide_state.remove(&key);
                    continue;
                }
                let derived = self.upload_scratch.entry(key.clone()).or_default();
                derived.clear();
                derived.extend(joined.iter().filter_map(|p| {
                    let x_lin = p[0] + x_tr.offset;
                    let y_lin = p[1] + y_tr.offset;
                    let x = if x_log {
                        if x_lin > 0.0 {
                            x_lin.log10()
                        } else {
                            f64::NAN
                        }
                    } else {
                        x_lin - sx
                    };
                    let y = if y_log {
                        if y_lin > 0.0 {
                            y_lin.log10()
                        } else {
                            f64::NAN
                        }
                    } else {
                        y_lin - sy
                    };
                    (x.is_finite() && y.is_finite()).then_some([x, y])
                }));
                seen.insert(key.clone());
                let id = *self.trace_ids.entry(key).or_insert_with(|| fig.add_trace());
                let (color, width) =
                    hover_look(pair_look, Some(&[x_name.clone(), y_name.clone()]), traces);
                fig.set_trace(id, &derived, color, width, vp_style(pair_look));
                fig.set_marker(id, vp_marker(pair_look), pair_look.point_size * 2.0);
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
            self.slide_state.remove(&k);
        }

        // Until the caller writes back real plot bounds, auto-fit data
        // (or honor the trailing time window). bounds_external means
        // the widget owns the viewport — never clobber it. `bounds` is
        // the PADDED view — the fit produces data (inner) bounds, so
        // pad here exactly like the caller does for `data.*_bounds`.
        if !self.initialized && !self.bounds_external {
            let inner = Self::fit_bounds(scope, traces, self.bounds);
            self.bounds = (
                crate::panels::plot_backend::pad_axis_bounds(inner.0),
                crate::panels::plot_backend::pad_axis_bounds(inner.1),
            );
            // fit may have changed the bounds the shift was computed
            // from — recompute so data+viewport stay consistent.
            let x_span = (self.bounds.0[1] - self.bounds.0[0]).abs().max(1e-12);
            let y_span = (self.bounds.1[1] - self.bounds.1[0]).abs().max(1e-12);
            self.shift.0 = if self.bounds.0[0].abs() > x_span * 1e4 {
                self.bounds.0[0]
            } else {
                0.0
            };
            self.shift.1 = if self.bounds.1[0].abs() > y_span * 1e4 {
                self.bounds.1[0]
            } else {
                0.0
            };
        }
        let (xb, yb) = self.bounds;

        fig.set_viewport(
            xb[0] - self.shift.0,
            xb[1] - self.shift.0,
            yb[0] - self.shift.1,
            yb[1] - self.shift.1,
        );
        fig.set_background(!dark);
        // Pixel margins as canvas fractions — the caller's
        // `volcano_margins_px` table, identical to the values used for
        // the overlay axes-rect (single source of truth).
        let (ml, mr, mb, mt) = margins;
        let (wf, hf) = (w as f32, h as f32);
        fig.set_margins(ml / wf, 1.0 - mr / wf, mb / hf, 1.0 - mt / hf);
        fig.set_background_rgba(
            panel_bg.r() as f32 / 255.0,
            panel_bg.g() as f32 / 255.0,
            panel_bg.b() as f32 / 255.0,
        );
        // Same label text the egui path computes (explicit name,
        // auto-generated trace names, or None when show_label is off).
        let x_label = scope.axis_label_text(traces, true);
        let y_label = scope.axis_label_text(traces, false);
        fig.set_axis_labels(x_label.as_deref(), y_label.as_deref());
        fig.set_grid(scope.show_grid);

        // Render + readback on the persistent worker — the expensive
        // part (prepare/renderFrame/readback ~2-7ms) never blocks the
        // UI. A dead worker (send fails) is dropped and respawned on
        // the next sync; the figure is recreated below if lost.
        let owned = self.fig.take().unwrap();
        let px = std::mem::take(&mut self.pixel_scratch);
        self.dbg_dispatches += 1;
        if !self
            .worker
            .get_or_insert_with(RenderWorker::spawn)
            .dispatch(owned, px)
        {
            self.worker = None;
        }
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
        if scope.scope_type == ScopeType::XYScope {
            // XY scopes keep no entries in `trace_order` — fit over
            // the joined pairs like `ScopeData::fit_*_bounds` does.
            const TOL: f64 = 1e-9;
            for (x_name, y_name, pl) in scope.xy_pairs.iter() {
                let (Some(x_name), Some(y_name)) = (x_name, y_name) else {
                    continue;
                };
                if !pl.visible {
                    continue;
                }
                let (Some(x_tr), Some(y_tr)) = (traces.get_trace(x_name), traces.get_trace(y_name))
                else {
                    continue;
                };
                if !x_tr.look.visible || !y_tr.look.visible {
                    continue;
                }
                let (Some(x_pts), Some(y_pts)) = (
                    traces.get_points_ref(x_name, scope.paused),
                    traces.get_points_ref(y_name, scope.paused),
                ) else {
                    continue;
                };
                for p in TracesCollection::join_xy_points(x_pts, y_pts, TOL) {
                    xb[0] = xb[0].min(p[0] + x_tr.offset);
                    xb[1] = xb[1].max(p[0] + x_tr.offset);
                    yb[0] = yb[0].min(p[1] + y_tr.offset);
                    yb[1] = yb[1].max(p[1] + y_tr.offset);
                }
            }
            if !xb[0].is_finite() {
                xb = bounds.0;
            }
            if !yb[0].is_finite() {
                yb = bounds.1;
            }
            return (xb, yb);
        }
        for name in scope.trace_order.iter() {
            let Some(tr) = traces.get_trace(name) else {
                continue;
            };
            let Some(pts) = traces.get_points_ref(name, scope.paused) else {
                continue;
            };
            for p in pts.iter() {
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

impl Drop for VolcanoSurface {
    /// Close the job channel and join the worker — a detached render
    /// thread could still be inside `vp_render_rgba`/`vp_figure_destroy`
    /// while the process (and the Vulkan instance) tears down.
    fn drop(&mut self) {
        if let Some(w) = self.worker.take() {
            drop(w.job_tx);
            let _ = w.handle.join();
        }
    }
}
