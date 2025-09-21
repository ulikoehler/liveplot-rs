//! Multi-trace oscilloscope UI: plots multiple named series with shared controls.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use eframe::{self, egui};
use egui::Color32;
use egui_plot::{Line, Legend, Plot, PlotPoint, PlotPoints, Points, Text};
use image::{Rgba, RgbaImage};

use crate::controllers::{FftController, WindowController, WindowInfo, UiActionController, RawExportFormat, FftDataRequest, FftRawData};
#[cfg(feature = "fft")]
use crate::controllers::FftPanelInfo;
#[cfg(feature = "fft")]
use crate::fft;
#[cfg(feature = "fft")]
pub use crate::fft::FftWindow;
#[cfg(not(feature = "fft"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FftWindow { Rect, Hann, Hamming, Blackman }
use crate::point_selection::PointSelection;
use crate::export;
use crate::sink::MultiSample;
use crate::config::XDateFormat;
use crate::math::{MathTraceDef, MathRuntimeState, compute_math_trace, MathKind, FilterKind, TraceRef, MinMaxMode};
use crate::thresholds::{ThresholdDef, ThresholdKind, ThresholdEvent, ThresholdController, ThresholdRuntimeState};

/// Internal per-trace state (live buffer, optional snapshot, color, cached FFT).
struct TraceState {
    name: String,
    color: Color32,
    live: VecDeque<[f64;2]>,
    snap: Option<VecDeque<[f64;2]>>,
    // Cached last computed FFT (frequency, magnitude)
    last_fft: Option<Vec<[f64;2]>>,
    // Whether this trace is a derived math trace
    is_math: bool,
}

/// Egui app that displays multiple traces and supports point selection and FFT.
pub struct ScopeAppMulti {
    pub rx: Receiver<MultiSample>,
    traces: HashMap<String, TraceState>,
    pub trace_order: Vec<String>,
    pub max_points: usize,
    pub time_window: f64,
    pub last_prune: std::time::Instant,
    pub reset_view: bool,
    pub paused: bool,
    /// Optional controller to let external code get/set/listen to window info.
    pub window_controller: Option<WindowController>,
    /// Optional controller to get/set/listen to FFT panel info
    pub fft_controller: Option<FftController>,
    /// Optional controller for high-level UI actions (pause/resume/screenshot)
    pub ui_action_controller: Option<UiActionController>,
    // FFT related
    pub show_fft: bool,
    pub fft_size: usize,
    pub fft_window: FftWindow,
    pub fft_last_compute: std::time::Instant,
    pub fft_db: bool,
    pub fft_fit_view: bool,
    pub request_window_shot: bool,
    pub last_viewport_capture: Option<Arc<egui::ColorImage>>,
    // Point & slope selection (multi-trace)
    /// Selected trace for point/slope selection. None => Free placement (no snapping).
    pub selection_trace: Option<String>,
    /// Index-based selection for the active trace (behaves like single-trace mode).
    pub point_selection: PointSelection,
    /// Formatting of X values in point labels
    pub x_date_format: XDateFormat,
    // Math traces
    pub math_defs: Vec<MathTraceDef>,
    math_states: HashMap<String, MathRuntimeState>,
    show_math_dialog: bool,
    math_builder: MathBuilderState,
    math_editing: Option<String>,
    math_error: Option<String>,
    // Thresholds
    pub threshold_controller: Option<ThresholdController>,
    pub threshold_defs: Vec<ThresholdDef>,
    threshold_states: HashMap<String, ThresholdRuntimeState>,
    show_thresholds_dialog: bool,
    thr_builder: ThresholdBuilderState,
    thr_editing: Option<String>,
    thr_error: Option<String>,
    // Threshold events (global)
    /// Total number of threshold exceed events observed since app start (never capped).
    pub threshold_total_count: u64,
    /// Global rolling log of recent threshold events (for the UI table).
    threshold_event_log: VecDeque<ThresholdEvent>,
    /// Maximum number of events to keep in the global UI log (prevents unbounded memory growth).
    threshold_event_log_cap: usize,
    /// Optional filter for the events table (None = all thresholds, Some(name) = only that threshold).
    threshold_events_filter: Option<String>,
}

#[derive(Debug, Clone)]
struct MathBuilderState {
    name: String,
    kind_idx: usize,
    add_inputs: Vec<(usize, f64)>,
    mul_a_idx: usize,
    mul_b_idx: usize,
    single_idx: usize, // for differentiate/integrate/filter/minmax
    integ_y0: f64,
    filter_which: usize, // 0 LP,1 HP,2 BP
    filter_f1: f64,
    filter_f2: f64,
    filter_q: f64,
    minmax_decay: f64,
}

impl Default for MathBuilderState {
    fn default() -> Self {
        Self { name: String::new(), kind_idx: 0, add_inputs: vec![(0, 1.0), (0, 1.0)], mul_a_idx: 0, mul_b_idx: 0, single_idx: 0, integ_y0: 0.0, filter_which: 0, filter_f1: 1.0, filter_f2: 10.0, filter_q: 0.707, minmax_decay: 0.0 }
    }
}

#[derive(Debug, Clone)]
struct ThresholdBuilderState {
    name: String,
    target_idx: usize,
    kind_idx: usize, // 0: >, 1: <, 2: in range
    thr1: f64,
    thr2: f64,
    min_duration_ms: f64,
    max_events: usize,
}

impl Default for ThresholdBuilderState {
    fn default() -> Self {
        Self { name: String::new(), target_idx: 0, kind_idx: 0, thr1: 0.0, thr2: 1.0, min_duration_ms: 2.0, max_events: 100 }
    }
}

impl MathBuilderState {
    fn from_def(def: &MathTraceDef, trace_order: &Vec<String>) -> Self {
        let mut b = Self::default();
        b.name = def.name.clone();
        match &def.kind {
            MathKind::Add { inputs } => {
                b.kind_idx = 0;
                b.add_inputs = inputs.iter().map(|(r, g)| {
                    let idx = trace_order.iter().position(|n| n == &r.0).unwrap_or(0);
                    (idx, *g)
                }).collect();
                if b.add_inputs.is_empty() { b.add_inputs.push((0, 1.0)); }
            }
            MathKind::Multiply { a, b: bb } => {
                b.kind_idx = 1;
                b.mul_a_idx = trace_order.iter().position(|n| n == &a.0).unwrap_or(0);
                b.mul_b_idx = trace_order.iter().position(|n| n == &bb.0).unwrap_or(0);
            }
            MathKind::Divide { a, b: bb } => {
                b.kind_idx = 2;
                b.mul_a_idx = trace_order.iter().position(|n| n == &a.0).unwrap_or(0);
                b.mul_b_idx = trace_order.iter().position(|n| n == &bb.0).unwrap_or(0);
            }
            MathKind::Differentiate { input } => {
                b.kind_idx = 3;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
            }
            MathKind::Integrate { input, y0 } => {
                b.kind_idx = 4;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                b.integ_y0 = *y0;
            }
            MathKind::Filter { input, kind } => {
                b.kind_idx = 5;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                match kind {
                    FilterKind::Lowpass { cutoff_hz } => { b.filter_which = 0; b.filter_f1 = *cutoff_hz; }
                    FilterKind::Highpass { cutoff_hz } => { b.filter_which = 1; b.filter_f1 = *cutoff_hz; }
                    FilterKind::Bandpass { low_cut_hz, high_cut_hz } => { b.filter_which = 2; b.filter_f1 = *low_cut_hz; b.filter_f2 = *high_cut_hz; }
                    FilterKind::BiquadLowpass { cutoff_hz, q } => { b.filter_which = 3; b.filter_f1 = *cutoff_hz; b.filter_q = *q; }
                    FilterKind::BiquadHighpass { cutoff_hz, q } => { b.filter_which = 4; b.filter_f1 = *cutoff_hz; b.filter_q = *q; }
                    FilterKind::BiquadBandpass { center_hz, q } => { b.filter_which = 5; b.filter_f1 = *center_hz; b.filter_q = *q; }
                    FilterKind::Custom { .. } => { b.filter_which = 0; }
                }
            }
            MathKind::MinMax { input, decay_per_sec, mode } => {
                b.kind_idx = if matches!(mode, MinMaxMode::Min) { 6 } else { 7 };
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                b.minmax_decay = decay_per_sec.unwrap_or(0.0);
            }
        }
        b
    }
}

impl ScopeAppMulti {
    pub fn new(rx: Receiver<MultiSample>) -> Self {
        Self {
            rx,
            traces: HashMap::new(),
            trace_order: Vec::new(),
            max_points: 10_000,
            time_window: 10.0,
            last_prune: std::time::Instant::now(),
            reset_view: false,
            paused: false,
            show_fft: false,
            fft_size: 1024,
            fft_window: FftWindow::Hann,
            fft_last_compute: std::time::Instant::now(),
            fft_db: false,
            fft_fit_view: false,
            window_controller: None,
            fft_controller: None,
            ui_action_controller: None,
            request_window_shot: false,
            last_viewport_capture: None,
            selection_trace: None,
            point_selection: PointSelection::default(),
            x_date_format: XDateFormat::default(),
            math_defs: Vec::new(),
            math_states: HashMap::new(),
            show_math_dialog: false,
            math_builder: MathBuilderState::default(),
            math_editing: None,
            math_error: None,
            threshold_controller: None,
            threshold_defs: Vec::new(),
            threshold_states: HashMap::new(),
            show_thresholds_dialog: false,
            thr_builder: ThresholdBuilderState::default(),
            thr_editing: None,
            thr_error: None,
            threshold_total_count: 0,
            threshold_event_log: VecDeque::new(),
            threshold_event_log_cap: 10_000,
            threshold_events_filter: None,
        }
    }

    fn add_math_trace_internal(&mut self, def: MathTraceDef) {
        if self.traces.contains_key(&def.name) { return; }
        let idx = self.trace_order.len();
        self.trace_order.push(def.name.clone());
        let color = Self::alloc_color(idx);
        self.traces.insert(def.name.clone(), TraceState { name: def.name.clone(), color, live: VecDeque::new(), snap: None, last_fft: None, is_math: true });
        self.math_states.entry(def.name.clone()).or_insert_with(MathRuntimeState::new);
        self.math_defs.push(def);
    }

    fn remove_math_trace_internal(&mut self, name: &str) {
        self.math_defs.retain(|d| d.name != name);
        self.math_states.remove(name);
        self.traces.remove(name);
        self.trace_order.retain(|n| n != name);
    }

    /// Public API: add a math trace definition (creates a new virtual trace that auto-updates).
    pub fn add_math_trace(&mut self, def: MathTraceDef) { self.add_math_trace_internal(def); }

    /// Public API: remove a previously added math trace by name.
    pub fn remove_math_trace(&mut self, name: &str) { self.remove_math_trace_internal(name); }

    /// Public API: list current math trace definitions.
    pub fn math_traces(&self) -> &[MathTraceDef] { &self.math_defs }

    fn recompute_math_traces(&mut self) {
        if self.math_defs.is_empty() { return; }
        // Build sources from existing traces (prefer snapshot when paused)
        let mut sources: HashMap<String, Vec<[f64;2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused { if let Some(s) = &tr.snap { Box::new(s.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        // Compute each math def in insertion order; allow math-of-math using updated sources.
        for def in &self.math_defs.clone() {
            let st = self.math_states.entry(def.name.clone()).or_insert_with(MathRuntimeState::new);
            // Provide previous output (from sources) and prune cutoff (based on time window)
            let prev_out = sources.get(&def.name).map(|v| v.as_slice());
            let prune_cut = {
                // Calculate cutoff as oldest time we expect to keep; allow slight cushion
                let latest = self.trace_order.iter().filter_map(|n| sources.get(n).and_then(|v| v.last().map(|p| p[0]))).fold(f64::NEG_INFINITY, f64::max);
                if latest.is_finite() { Some(latest - self.time_window * 1.2) } else { None }
            };
            let pts = compute_math_trace(def, &sources, prev_out, prune_cut, st);
            sources.insert(def.name.clone(), pts.clone());
            // Update backing trace buffers
            if let Some(tr) = self.traces.get_mut(&def.name) {
                tr.live = pts.iter().copied().collect();
                if self.paused { tr.snap = Some(tr.live.clone()); } else { tr.snap = None; }
            } else {
                // Create if missing (def might have been added but no entry created)
                let idx = self.trace_order.len();
                self.trace_order.push(def.name.clone());
                let mut dq: VecDeque<[f64;2]> = VecDeque::new();
                dq.extend(pts.iter().copied());
                self.traces.insert(def.name.clone(), TraceState { name: def.name.clone(), color: Self::alloc_color(idx), live: dq.clone(), snap: if self.paused { Some(dq.clone()) } else { None }, last_fft: None, is_math: true });
            }
        }
    }

    fn process_thresholds(&mut self) {
        if self.threshold_defs.is_empty() { return; }
        // Build sources from existing traces (prefer snapshot when paused)
        let mut sources: HashMap<String, Vec<[f64;2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused { if let Some(s) = &tr.snap { Box::new(s.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        // Process each threshold incrementally
        for def in self.threshold_defs.clone().iter() {
            let state = self.threshold_states.entry(def.name.clone()).or_insert_with(ThresholdRuntimeState::new);
            let data = match sources.get(&def.target.0) { Some(v) => v, None => continue };
            let mut start_idx = 0usize;
            if let Some(t0) = state.prev_in_t { // find first strictly after t0
                start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                    Ok(mut i) => { while i < data.len() && data[i][0] <= t0 { i += 1; } i },
                    Err(i) => i,
                };
            }
            for p in data.iter().skip(start_idx) {
                let t = p[0]; let v = p[1];
                let e = def.kind.excess(v);
                if let Some(t0) = state.last_t {
                    let dt = (t - t0).max(0.0);
                    if state.active || e > 0.0 {
                        // Trapezoid integrate excess
                        state.accum_area += 0.5 * (state.last_excess + e) * dt;
                    }
                }
                // Transition logic
                if !state.active && e > 0.0 {
                    state.active = true;
                    state.start_t = t;
                } else if state.active && e == 0.0 {
                    // Close event
                    let end_t = t;
                    let dur = end_t - state.start_t;
                    if dur >= def.min_duration_s {
                        let evt = ThresholdEvent { threshold: def.name.clone(), trace: def.target.0.clone(), start_t: state.start_t, end_t, duration: dur, area: state.accum_area };
                        state.push_event_capped(evt.clone(), def.max_events);
                        // Update global counters/log (never capped counter)
                        self.threshold_total_count = self.threshold_total_count.saturating_add(1);
                        self.threshold_event_log.push_back(evt.clone());
                        while self.threshold_event_log.len() > self.threshold_event_log_cap { self.threshold_event_log.pop_front(); }
                        if let Some(ctrl) = &self.threshold_controller {
                            let mut inner = ctrl.inner.lock().unwrap();
                            inner.listeners.retain(|s| s.send(evt.clone()).is_ok());
                        }
                    }
                    state.active = false;
                    state.accum_area = 0.0;
                }
                state.last_t = Some(t);
                state.last_excess = e;
                state.prev_in_t = Some(t);
            }
        }
    }

    fn add_threshold_internal(&mut self, def: ThresholdDef) {
        if self.threshold_defs.iter().any(|d| d.name == def.name) { return; }
        self.threshold_states.entry(def.name.clone()).or_insert_with(ThresholdRuntimeState::new);
        self.threshold_defs.push(def);
    }

    fn remove_threshold_internal(&mut self, name: &str) {
        self.threshold_defs.retain(|d| d.name != name);
        self.threshold_states.remove(name);
    }

    /// Public API: add/remove/list thresholds; get events for a threshold (clone)
    pub fn add_threshold(&mut self, def: ThresholdDef) { self.add_threshold_internal(def); }
    pub fn remove_threshold(&mut self, name: &str) { self.remove_threshold_internal(name); }
    pub fn thresholds(&self) -> &[ThresholdDef] { &self.threshold_defs }
    pub fn threshold_events(&self, name: &str) -> Option<Vec<ThresholdEvent>> {
        self.threshold_states.get(name).map(|s| s.events.iter().cloned().collect())
    }

    /// Update an existing math trace definition; supports renaming if the new name is unique.
    pub fn update_math_trace(&mut self, original_name: &str, new_def: MathTraceDef) -> Result<(), &'static str> {
        // Name collision check if renaming
        if new_def.name != original_name && self.traces.contains_key(&new_def.name) {
            return Err("A trace with the new name already exists");
        }
        // Replace def
        if let Some(pos) = self.math_defs.iter().position(|d| d.name == original_name) {
            self.math_defs[pos] = new_def.clone();
        } else { return Err("Original math trace not found"); }

        // Reset runtime state for this math trace (operation may have changed)
        self.math_states.insert(new_def.name.clone(), MathRuntimeState::new());
        if new_def.name != original_name { self.math_states.remove(original_name); }

        // Rename/move underlying TraceState if needed
        if new_def.name != original_name {
            if let Some(mut tr) = self.traces.remove(original_name) {
                tr.name = new_def.name.clone();
                self.traces.insert(new_def.name.clone(), tr);
            }
            // Update order and selection
            for name in &mut self.trace_order { if name == original_name { *name = new_def.name.clone(); break; } }
            if let Some(sel) = &mut self.selection_trace { if sel == original_name { *sel = new_def.name.clone(); } }
        }

        // Trigger recompute on next update cycle immediately
        self.recompute_math_traces();
        Ok(())
    }

    fn apply_add_or_edit(&mut self, def: MathTraceDef) {
        self.math_error = None;
        if let Some(orig) = self.math_editing.clone() {
            match self.update_math_trace(&orig, def) {
                Ok(()) => { self.math_editing = None; self.math_builder = MathBuilderState::default(); }
                Err(e) => { self.math_error = Some(e.to_string()); }
            }
        } else {
            if self.traces.contains_key(&def.name) { self.math_error = Some("A trace with this name already exists".into()); return; }
            self.add_math_trace_internal(def);
            self.math_builder = MathBuilderState::default();
        }
    }

    fn alloc_color(index: usize) -> Color32 {
        // Simple distinct color palette
        const PALETTE: [Color32; 10] = [
            Color32::LIGHT_BLUE,
            Color32::LIGHT_RED,
            Color32::LIGHT_GREEN,
            Color32::GOLD,
            Color32::from_rgb(0xAA, 0x55, 0xFF), // purple
            Color32::from_rgb(0xFF, 0xAA, 0x00), // orange
            Color32::from_rgb(0x00, 0xDD, 0xDD), // cyan
            Color32::from_rgb(0xDD, 0x00, 0xDD), // magenta
            Color32::from_rgb(0x66, 0xCC, 0x66), // green2
            Color32::from_rgb(0xCC, 0x66, 0x66), // red2
        ];
        PALETTE[index % PALETTE.len()]
    }
}

impl eframe::App for ScopeAppMulti {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ingest new multi samples
        while let Ok(s) = self.rx.try_recv() {
            let is_new = !self.traces.contains_key(&s.trace);
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                TraceState { name: s.trace.clone(), color: Self::alloc_color(idx), live: VecDeque::new(), snap: None, last_fft: None, is_math: false }
            });
            if is_new && self.selection_trace.is_none() { self.selection_trace = Some(s.trace.clone()); }
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            if entry.live.len() > self.max_points { entry.live.pop_front(); }
        }

        // Prune per-trace based on rolling time window
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            for (_k, tr) in self.traces.iter_mut() {
                if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                    let cutoff = t_latest - self.time_window * 1.15;
                    while let Some(&[t, _]) = tr.live.front() { if t < cutoff { tr.live.pop_front(); } else { break; } }
                }
            }
            self.last_prune = std::time::Instant::now();
        }

        // Recompute math traces from current sources
        self.recompute_math_traces();

        // Apply threshold controller requests
        if let Some(ctrl) = &self.threshold_controller {
            // Drain requests first, then drop the lock before mutating self
            let (adds, removes) = {
                let mut inner = ctrl.inner.lock().unwrap();
                let adds: Vec<ThresholdDef> = inner.add_requests.drain(..).collect();
                let removes: Vec<String> = inner.remove_requests.drain(..).collect();
                (adds, removes)
            };
            for def in adds { self.add_threshold_internal(def); }
            for name in removes { self.remove_threshold_internal(&name); }
        }

        // Process thresholds based on current buffers (including math traces)
        self.process_thresholds();

        // Controls
        egui::TopBottomPanel::top("controls_multi").show(ctx, |ui| {
            ui.heading("LivePlot (multi)");
            ui.label("Left mouse: pan  |  Right drag: zoom box");
            ui.horizontal(|ui| {
                ui.label("Time window (s):");
                ui.add(egui::Slider::new(&mut self.time_window, 1.0..=60.0));
                ui.label("Points cap:");
                ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));
                // Marker trace selection ("Free" or one trace)
                let mut new_selection = self.selection_trace.clone();
                egui::ComboBox::from_id_salt("marker_trace_select")
                    .selected_text(match &new_selection { Some(s) => format!("Trace: {}", s), None => "Trace: Free".to_owned() })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut new_selection, None, "Free");
                        for name in &self.trace_order { ui.selectable_value(&mut new_selection, Some(name.clone()), name); }
                    });
                if new_selection != self.selection_trace { self.selection_trace = new_selection; }
                if ui.button("Clear Selection").clicked() { self.point_selection.clear(); }
                #[cfg(feature = "fft")]
                if ui.button(if self.show_fft { "Hide FFT" } else { "Show FFT" }).clicked() {
                    self.show_fft = !self.show_fft;
                    if let Some(ctrl) = &self.fft_controller {
                        let mut inner = ctrl.inner.lock().unwrap();
                        inner.show = self.show_fft;
                        let info = FftPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
                        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
                    }
                }
                #[cfg(not(feature = "fft"))]
                {
                    let _ = (FftWindow::Rect,);
                }
                if ui.button(if self.paused { "Resume" } else { "Pause" }).clicked() {
                    if self.paused { // resume
                        self.paused = false;
                        for tr in self.traces.values_mut() { tr.snap = None; }
                    } else { // pause and snapshot
                        for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); }
                        self.paused = true;
                    }
                }
                if ui.button("Reset View").clicked() { self.reset_view = true; }
                if ui.button("Clear").clicked() { for tr in self.traces.values_mut() { tr.live.clear(); if let Some(s) = &mut tr.snap { s.clear(); } } }
                if ui.button("Save PNG").on_hover_text("Take an egui viewport screenshot").clicked() { self.request_window_shot = true; }
                if ui.button("Math…").on_hover_text("Create and manage math traces").clicked() { self.show_math_dialog = true; }
                {
                    let thr_btn_label = if self.threshold_total_count > 0 {
                        format!("Thresholds… ({})", self.threshold_total_count)
                    } else { "Thresholds…".to_string() };
                    if ui.button(thr_btn_label).on_hover_text("Create and manage threshold detectors").clicked() { self.show_thresholds_dialog = true; }
                }
                let hover_text: &str = {
                    #[cfg(feature = "parquet")]
                    { "Export all traces as CSV or Parquet" }
                    #[cfg(not(feature = "parquet"))]
                    { "Export all traces as CSV" }
                };
                if ui.button("Save raw data").on_hover_text(hover_text).clicked() {
                    // Prompt for format; simple dialog via file extension choice.
                    let mut dlg = rfd::FileDialog::new();
                    dlg = dlg.add_filter("CSV", &["csv"]);
                    #[cfg(feature = "parquet")]
                    { dlg = dlg.add_filter("Parquet", &["parquet"]); }
                    if let Some(path) = dlg
                        .set_file_name("liveplot_export.csv")
                        .save_file() {
                        let fmt = {
                            #[cfg(feature = "parquet")]
                            {
                                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                                if ext.eq_ignore_ascii_case("parquet") { RawExportFormat::Parquet } else { RawExportFormat::Csv }
                            }
                            #[cfg(not(feature = "parquet"))]
                            { RawExportFormat::Csv }
                        };
                        if let Err(e) = save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) {
                            eprintln!("Failed to save raw data: {e}");
                        }
                    }
                }
            });
        });

        // Math dialog
        if self.show_math_dialog {
            let mut show_flag = self.show_math_dialog;
            egui::Window::new("Math traces").open(&mut show_flag).show(ctx, |ui| {
                ui.label("Create virtual traces from existing ones.");
                if let Some(err) = &self.math_error { ui.colored_label(Color32::LIGHT_RED, err); }
                ui.separator();
                // Existing math traces list with remove button
                for def in self.math_defs.clone().iter() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}: {:?}", def.name, def.kind));
                        if ui.button("Edit").clicked() {
                            // initialize builder from existing def
                            self.math_builder = MathBuilderState::from_def(def, &self.trace_order);
                            self.math_editing = Some(def.name.clone());
                        }
                        if ui.button("Remove").clicked() {
                            self.remove_math_trace_internal(&def.name);
                        }
                    });
                }
                ui.separator();
                let editing = self.math_editing.clone();
                let is_editing = editing.is_some();
                let header = if is_editing { "Edit" } else { "Add new" };
                ui.collapsing(header, |ui| {
                    // Persistent builder state
                    let kinds = ["Add/Subtract", "Multiply", "Divide", "Differentiate", "Integrate", "Filter", "Min", "Max"];
                    egui::ComboBox::from_label("Operation").selected_text(kinds[self.math_builder.kind_idx]).show_ui(ui, |ui| {
                        for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut self.math_builder.kind_idx, i, *k); }
                    });
                    ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.math_builder.name); });
                    let trace_names: Vec<String> = self.trace_order.clone();
                    match self.math_builder.kind_idx {
                        0 => { // Add/Sub
                            // allow editing up to N inputs with gains
                            for (idx, (sel, gain)) in self.math_builder.add_inputs.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt(format!("add_sel_{}", idx))
                                        .selected_text(trace_names.get(*sel).cloned().unwrap_or_default())
                                        .show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(sel, i, n); } });
                                    ui.label("gain"); ui.add(egui::DragValue::new(gain).speed(0.1));
                                });
                            }
                            ui.horizontal(|ui| {
                                if ui.button("Add input").clicked() { self.math_builder.add_inputs.push((0, 1.0)); }
                                if ui.button("Remove input").clicked() { if self.math_builder.add_inputs.len() > 1 { self.math_builder.add_inputs.pop(); } }
                            });
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                let inputs = self.math_builder.add_inputs.iter().filter_map(|(i, g)| trace_names.get(*i).cloned().map(|n| (TraceRef(n), *g))).collect();
                                if !self.math_builder.name.is_empty() {
                                    let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind: MathKind::Add { inputs } };
                                    self.apply_add_or_edit(def);
                                }
                            }
                        }
                        1 | 2 => { // Multiply/Divide
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_label("A").selected_text(trace_names.get(self.math_builder.mul_a_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.mul_a_idx, i, n); } });
                                egui::ComboBox::from_label("B").selected_text(trace_names.get(self.math_builder.mul_b_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.mul_b_idx, i, n); } });
                            });
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                if let (Some(a), Some(b)) = (trace_names.get(self.math_builder.mul_a_idx), trace_names.get(self.math_builder.mul_b_idx)) {
                                    let kind = if self.math_builder.kind_idx == 1 { MathKind::Multiply { a: TraceRef(a.clone()), b: TraceRef(b.clone()) } } else { MathKind::Divide { a: TraceRef(a.clone()), b: TraceRef(b.clone()) } };
                                    if !self.math_builder.name.is_empty() { let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind }; self.apply_add_or_edit(def); }
                                }
                            }
                        }
                        3 => { // Differentiate
                            egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.single_idx, i, n); } });
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                if let Some(nm) = trace_names.get(self.math_builder.single_idx) { if !self.math_builder.name.is_empty() { let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind: MathKind::Differentiate { input: TraceRef(nm.clone()) } }; self.apply_add_or_edit(def); } }
                            }
                        }
                        4 => { // Integrate
                            egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.single_idx, i, n); } });
                            ui.horizontal(|ui| { ui.label("y0"); ui.add(egui::DragValue::new(&mut self.math_builder.integ_y0).speed(0.1)); });
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                if let Some(nm) = trace_names.get(self.math_builder.single_idx) { if !self.math_builder.name.is_empty() { let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind: MathKind::Integrate { input: TraceRef(nm.clone()), y0: self.math_builder.integ_y0 } }; self.apply_add_or_edit(def); } }
                            }
                        }
                        5 => { // Filter
                            egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.single_idx, i, n); } });
                            let fk = ["Lowpass (1st)", "Highpass (1st)", "Bandpass (1st)", "Biquad LP", "Biquad HP", "Biquad BP"];
                            egui::ComboBox::from_label("Filter").selected_text(fk[self.math_builder.filter_which]).show_ui(ui, |ui| { for (i, n) in fk.iter().enumerate() { ui.selectable_value(&mut self.math_builder.filter_which, i, *n); } });
                            match self.math_builder.filter_which {
                                0 | 1 => { ui.horizontal(|ui| { ui.label("Cutoff Hz"); ui.add(egui::DragValue::new(&mut self.math_builder.filter_f1).speed(0.1)); }); },
                                2 => { ui.horizontal(|ui| { ui.label("Low cut Hz"); ui.add(egui::DragValue::new(&mut self.math_builder.filter_f1).speed(0.1)); }); ui.horizontal(|ui| { ui.label("High cut Hz"); ui.add(egui::DragValue::new(&mut self.math_builder.filter_f2).speed(0.1)); }); },
                                3 | 4 | 5 => {
                                    let label = match self.math_builder.filter_which { 3 | 4 => "Cutoff Hz", _ => "Center Hz" };
                                    ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(&mut self.math_builder.filter_f1).speed(0.1)); });
                                    ui.horizontal(|ui| { ui.label("Q"); ui.add(egui::DragValue::new(&mut self.math_builder.filter_q).speed(0.01)); });
                                }
                                _ => {}
                            }
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                if let Some(nm) = trace_names.get(self.math_builder.single_idx) { if !self.math_builder.name.is_empty() {
                                    let kind = match self.math_builder.filter_which {
                                        0 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Lowpass { cutoff_hz: self.math_builder.filter_f1 } },
                                        1 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Highpass { cutoff_hz: self.math_builder.filter_f1 } },
                                        2 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Bandpass { low_cut_hz: self.math_builder.filter_f1, high_cut_hz: self.math_builder.filter_f2 } },
                                        3 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadLowpass { cutoff_hz: self.math_builder.filter_f1, q: self.math_builder.filter_q } },
                                        4 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadHighpass { cutoff_hz: self.math_builder.filter_f1, q: self.math_builder.filter_q } },
                                        _ => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadBandpass { center_hz: self.math_builder.filter_f1, q: self.math_builder.filter_q } },
                                    };
                                    let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind }; self.apply_add_or_edit(def);
                                } }
                            }
                        }
                        6 | 7 => { // Min/Max
                            egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.math_builder.single_idx, i, n); } });
                            ui.horizontal(|ui| { ui.label("Decay (1/s, 0=none)"); ui.add(egui::DragValue::new(&mut self.math_builder.minmax_decay).speed(0.1)); });
                            if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                                if let Some(nm) = trace_names.get(self.math_builder.single_idx) { if !self.math_builder.name.is_empty() { let mode = if self.math_builder.kind_idx == 6 { MinMaxMode::Min } else { MinMaxMode::Max }; let decay_opt = if self.math_builder.minmax_decay > 0.0 { Some(self.math_builder.minmax_decay) } else { None }; let def = MathTraceDef { name: self.math_builder.name.clone(), color_hint: None, kind: MathKind::MinMax { input: TraceRef(nm.clone()), decay_per_sec: decay_opt, mode } }; self.apply_add_or_edit(def); } }
                            }
                        }
                        _ => {}
                    }
                    if is_editing {
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() { self.math_editing = None; self.math_builder = MathBuilderState::default(); self.math_error = None; }
                        });
                    }
                });
            });
            self.show_math_dialog = show_flag;
        }

        // FFT bottom panel for multi-traces
        #[cfg(feature = "fft")]
        if self.show_fft {
            egui::TopBottomPanel::bottom("fft_panel_multi")
                .resizable(true)
                .min_height(120.0)
                .default_height(300.0)
                .show(ctx, |ui| {
                    if let Some(ctrl) = &self.fft_controller {
                        let size_pts = ui.available_size();
                        let ppp = ctx.pixels_per_point();
                        let size_px = [size_pts.x * ppp, size_pts.y * ppp];
                        let mut inner = ctrl.inner.lock().unwrap();
                        inner.current_size = Some(size_px);
                        let info = FftPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
                        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
                    }
                    egui::CollapsingHeader::new("FFT Settings").default_open(true).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("FFT size:");
                            let mut size_log2 = (self.fft_size as f32).log2() as u32;
                            let mut changed = false;
                            let resp = egui::Slider::new(&mut size_log2, 8..=15).text("2^N");
                            if ui.add(resp).changed() { changed = true; }
                            if changed { self.fft_size = 1usize << size_log2; }
                            ui.separator();
                            ui.label("Window:");
                            egui::ComboBox::from_id_salt("fft_window_multi")
                                .selected_text(self.fft_window.label())
                                .show_ui(ui, |ui| { for w in FftWindow::ALL { ui.selectable_value(&mut self.fft_window, *w, w.label()); } });
                            ui.separator();
                            if ui.button(if self.fft_db { "Linear" } else { "dB" }).on_hover_text("Toggle FFT magnitude scale").clicked() { self.fft_db = !self.fft_db; }
                            ui.separator();
                            if ui.button("Fit into view").on_hover_text("Auto scale FFT axes").clicked() { self.fft_fit_view = true; }
                        });
                    });

                    // Compute all FFTs (throttled)
                    if self.fft_last_compute.elapsed() > Duration::from_millis(100) {
                        for name in self.trace_order.clone().into_iter() {
                            if let Some(tr) = self.traces.get_mut(&name) {
                                tr.last_fft = fft::compute_fft(
                                    &tr.live,
                                    self.paused,
                                    &tr.snap,
                                    self.fft_size,
                                    self.fft_window,
                                );
                            }
                        }
                        self.fft_last_compute = std::time::Instant::now();
                    }

                    // Determine overall bounds for optional fit
                    let mut any_spec = false;
                    let mut min_x = f64::INFINITY;
                    let mut max_x = f64::NEG_INFINITY;
                    let mut min_y = f64::INFINITY;
                    let mut max_y = f64::NEG_INFINITY;
                    for name in self.trace_order.clone().into_iter() {
                        if let Some(tr) = self.traces.get(&name) {
                            if let Some(spec) = &tr.last_fft {
                                any_spec = true;
                                if self.fft_db {
                                    for p in spec.iter() {
                                        let y = 20.0 * p[1].max(1e-12).log10();
                                        if p[0] < min_x { min_x = p[0]; }
                                        if p[0] > max_x { max_x = p[0]; }
                                        if y < min_y { min_y = y; }
                                        if y > max_y { max_y = y; }
                                    }
                                } else {
                                    for p in spec.iter() {
                                        if p[0] < min_x { min_x = p[0]; }
                                        if p[0] > max_x { max_x = p[0]; }
                                        if p[1] < min_y { min_y = p[1]; }
                                        if p[1] > max_y { max_y = p[1]; }
                                    }
                                }
                            }
                        }
                    }

                    // Build plot and optionally include bounds
                    let mut plot = Plot::new("fft_plot_multi")
                        .legend(Legend::default())
                        .allow_zoom(true)
                        .allow_scroll(false)
                        .allow_boxed_zoom(true)
                        .y_axis_label(if self.fft_db { "Magnitude (dB)" } else { "Magnitude" })
                        .x_axis_label("Hz");
                    if self.fft_fit_view {
                        if min_x.is_finite() { plot = plot.include_x(min_x).include_x(max_x); }
                        if min_y.is_finite() { plot = plot.include_y(min_y).include_y(max_y); }
                        self.fft_fit_view = false; // consume request
                    }

                    let _ = plot.show(ui, |plot_ui| {
                        for name in self.trace_order.clone().into_iter() {
                            if let Some(tr) = self.traces.get(&name) {
                                if let Some(spec) = &tr.last_fft {
                                    let pts: PlotPoints = if self.fft_db {
                                        spec.iter().map(|p| { let mag = p[1].max(1e-12); let y = 20.0 * mag.log10(); [p[0], y] }).collect()
                                    } else {
                                        spec.iter().map(|p| [p[0], p[1]]).collect()
                                    };
                                    let line = Line::new(&tr.name, pts).color(tr.color);
                                    plot_ui.line(line);
                                }
                            }
                        }
                    });
                    if !any_spec { ui.label("FFT: not enough data yet"); }
                });
        }

        // Thresholds dialog
        if self.show_thresholds_dialog {
            let mut show_flag = self.show_thresholds_dialog;
            egui::Window::new("Thresholds").open(&mut show_flag).show(ctx, |ui| {
                ui.label("Detect and log when a trace exceeds a condition.");
                if let Some(err) = &self.thr_error { ui.colored_label(Color32::LIGHT_RED, err); }
                ui.separator();
                // List existing thresholds
                for def in self.threshold_defs.clone().iter() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} on {}: {:?}, min_dur={:.3} ms, cap={} events", def.name, def.target.0, def.kind, def.min_duration_s*1000.0, def.max_events));
                        if ui.button("Edit").clicked() {
                            self.thr_builder = ThresholdBuilderState::default();
                            self.thr_builder.name = def.name.clone();
                            self.thr_builder.target_idx = self.trace_order.iter().position(|n| n == &def.target.0).unwrap_or(0);
                            match &def.kind {
                                ThresholdKind::GreaterThan { value } => { self.thr_builder.kind_idx = 0; self.thr_builder.thr1 = *value; },
                                ThresholdKind::LessThan { value } => { self.thr_builder.kind_idx = 1; self.thr_builder.thr1 = *value; },
                                ThresholdKind::InRange { low, high } => { self.thr_builder.kind_idx = 2; self.thr_builder.thr1 = *low; self.thr_builder.thr2 = *high; },
                            }
                            self.thr_builder.min_duration_ms = def.min_duration_s * 1000.0;
                            self.thr_builder.max_events = def.max_events;
                            self.thr_editing = Some(def.name.clone());
                        }
                        if ui.button("Remove").clicked() { self.remove_threshold_internal(&def.name); }
                    });
                    // Show a short summary of recent events
                    if let Some(st) = self.threshold_states.get(&def.name) {
                        let cnt = st.events.len();
                        if let Some(last) = st.events.back() {
                            ui.label(format!("Events: {}, last: start={:.3}s, dur={:.3}ms, area={:.4}", cnt, last.start_t, last.duration*1000.0, last.area));
                        } else { ui.label("Events: 0"); }
                    }
                }
                ui.separator();
                let editing = self.thr_editing.clone();
                let is_editing = editing.is_some();
                let header = if is_editing { "Edit" } else { "Add new" };
                ui.collapsing(header, |ui| {
                    let kinds = [">", "<", "in range"];
                    egui::ComboBox::from_label("Condition").selected_text(kinds[self.thr_builder.kind_idx]).show_ui(ui, |ui| { for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut self.thr_builder.kind_idx, i, *k); } });
                    ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.thr_builder.name); });
                    let trace_names: Vec<String> = self.trace_order.clone();
                    egui::ComboBox::from_label("Trace").selected_text(trace_names.get(self.thr_builder.target_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.thr_builder.target_idx, i, n); } });
                    match self.thr_builder.kind_idx {
                        0 | 1 => { ui.horizontal(|ui| { ui.label("Value"); ui.add(egui::DragValue::new(&mut self.thr_builder.thr1).speed(0.01)); }); },
                        _ => {
                            ui.horizontal(|ui| { ui.label("Low"); ui.add(egui::DragValue::new(&mut self.thr_builder.thr1).speed(0.01)); });
                            ui.horizontal(|ui| { ui.label("High"); ui.add(egui::DragValue::new(&mut self.thr_builder.thr2).speed(0.01)); });
                        }
                    }
                    ui.horizontal(|ui| { ui.label("Min duration (ms)"); ui.add(egui::DragValue::new(&mut self.thr_builder.min_duration_ms).speed(0.1)); });
                    ui.horizontal(|ui| { ui.label("Max events"); ui.add(egui::DragValue::new(&mut self.thr_builder.max_events).speed(1)); });
                    if ui.button(if is_editing { "Save" } else { "Add threshold" }).clicked() {
                        if let Some(nm) = trace_names.get(self.thr_builder.target_idx) { if !self.thr_builder.name.is_empty() {
                            let kind = match self.thr_builder.kind_idx { 0 => ThresholdKind::GreaterThan { value: self.thr_builder.thr1 }, 1 => ThresholdKind::LessThan { value: self.thr_builder.thr1 }, _ => ThresholdKind::InRange { low: self.thr_builder.thr1.min(self.thr_builder.thr2), high: self.thr_builder.thr1.max(self.thr_builder.thr2) } };
                            let def = ThresholdDef { name: self.thr_builder.name.clone(), target: TraceRef(nm.clone()), kind, min_duration_s: (self.thr_builder.min_duration_ms / 1000.0).max(0.0), max_events: self.thr_builder.max_events };
                            if is_editing {
                                // replace existing by name
                                self.remove_threshold_internal(&editing.unwrap());
                                self.add_threshold_internal(def);
                            } else {
                                if self.threshold_defs.iter().any(|d| d.name == def.name) { self.thr_error = Some("A threshold with this name already exists".into()); } else { self.add_threshold_internal(def); self.thr_builder = ThresholdBuilderState::default(); }
                            }
                        } }
                    }
                    if is_editing { if ui.button("Cancel").clicked() { self.thr_editing = None; self.thr_builder = ThresholdBuilderState::default(); self.thr_error = None; } }
                });

                ui.separator();
                ui.heading("Threshold events");
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    // Build list of names from current thresholds and from the log
                    let mut names: Vec<String> = self.threshold_defs.iter().map(|d| d.name.clone()).collect();
                    for e in self.threshold_event_log.iter() { if !names.iter().any(|n| n == &e.threshold) { names.push(e.threshold.clone()); } }
                    names.sort(); names.dedup();
                    let mut sel = self.threshold_events_filter.clone();
                    egui::ComboBox::from_id_salt("thr_events_filter")
                        .selected_text(match &sel { Some(s) => format!("{}", s), None => "All".to_string() })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(sel.is_none(), "All").clicked() { sel = None; }
                            for n in &names { if ui.selectable_label(sel.as_ref() == Some(n), n).clicked() { sel = Some(n.clone()); } }
                        });
                    if sel != self.threshold_events_filter { self.threshold_events_filter = sel; }
                    if ui.button("Export to CSV").clicked() {
                        // Collect filtered events (newest first as shown)
                        let mut evts: Vec<&ThresholdEvent> = self.threshold_event_log.iter().rev()
                            .filter(|e| self.threshold_events_filter.as_ref().map_or(true, |f| &e.threshold == f))
                            .collect();
                        if !evts.is_empty() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("threshold_events.csv")
                                .add_filter("CSV", &["csv"]).save_file() {
                                if let Err(e) = save_threshold_events_csv(&path, &evts) { eprintln!("Failed to export events CSV: {e}"); }
                            }
                        }
                    }
                });
                // Table header
                egui::Grid::new("thr_events_grid_header").num_columns(6).striped(true).show(ui, |ui| {
                    ui.strong("End time (s)");
                    ui.strong("Threshold");
                    ui.strong("Trace");
                    ui.strong("Start (s)");
                    ui.strong("Duration (ms)");
                    ui.strong("Area");
                    ui.end_row();
                });
                // Rows in a scroll area (most recent on top)
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    egui::Grid::new("thr_events_grid_rows").num_columns(6).striped(true).show(ui, |ui| {
                        for e in self.threshold_event_log.iter().rev() {
                            if self.threshold_events_filter.as_ref().map_or(false, |f| &e.threshold != f) { continue; }
                            ui.label(format!("{:.6}", e.end_t));
                            ui.label(&e.threshold);
                            ui.label(&e.trace);
                            ui.label(format!("{:.6}", e.start_t));
                            ui.label(format!("{:.3}", e.duration * 1000.0));
                            ui.label(format!("{:.6}", e.area));
                            ui.end_row();
                        }
                    });
                });
            });
            self.show_thresholds_dialog = show_flag;
        }
        #[cfg(not(feature = "fft"))]
        {
            let _ = ctx; // suppress unused warnings
        }

        // Prepare selection data for currently selected trace (if any)
        let selected_trace_name = self.selection_trace.clone();
        let sel_data_points: Option<Vec<[f64;2]>> = if let Some(name) = &selected_trace_name {
            self.traces.get(name).map(|tr| {
                let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused {
                    if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                } else { Box::new(tr.live.iter()) };
                iter.cloned().collect()
            })
        } else { None };

    // Plot all traces
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut plot = Plot::new("scope_plot_multi")
                .allow_scroll(false)
                .allow_zoom(true)
                .allow_boxed_zoom(true)
                .x_axis_formatter(|x, _range| {
                    let val = x.value; let secs = val as i64; let nsecs = ((val - secs as f64) * 1e9) as u32;
                    let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                    dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
                });
            if self.reset_view { plot = plot.reset(); self.reset_view = false; }
            // Constrain X axis to the configured rolling time window across all traces
            let mut t_latest_overall = f64::NEG_INFINITY;
            for name in self.trace_order.clone().into_iter() {
                if let Some(tr) = self.traces.get(&name) {
                    let last_t = if self.paused { tr.snap.as_ref().and_then(|s| s.back()).map(|p| p[0]) } else { tr.live.back().map(|p| p[0]) };
                    if let Some(t) = last_t { if t > t_latest_overall { t_latest_overall = t; } }
                }
            }
            if t_latest_overall.is_finite() {
                let t_min = t_latest_overall - self.time_window;
                plot = plot.include_x(t_min).include_x(t_latest_overall);
            }
            if self.traces.len() > 1 { plot = plot.legend(Legend::default()); }
            let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
            let marker_font_size = base_body * 1.5;
            let plot_response = plot.show(ui, |plot_ui| {
                for name in self.trace_order.clone().into_iter() {
                    if let Some(tr) = self.traces.get(&name) {
                        let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                        } else { Box::new(tr.live.iter()) };
                        let pts: PlotPoints = iter.cloned().collect();
                        let mut line = Line::new(&tr.name, pts).color(tr.color);
                        if self.traces.len() > 1 { line = line.name(&tr.name); }
                        plot_ui.line(line);
                    }
                }
                // Draw shared selection markers/overlays (same in all modes)
                if let Some(p) = self.point_selection.selected_p1 {
                    plot_ui.points(Points::new("", vec![p]).radius(5.0).color(Color32::YELLOW));
                    let txt = format!("P1\nx={}\ny={:.4}", self.x_date_format.format_value(p[0]), p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::YELLOW);
                    plot_ui.text(Text::new("p1_lbl", PlotPoint::new(p[0], p[1]), rich));
                }
                if let Some(p) = self.point_selection.selected_p2 {
                    plot_ui.points(Points::new("", vec![p]).radius(5.0).color(Color32::LIGHT_BLUE));
                    let txt = format!("P2\nx={}\ny={:.4}", self.x_date_format.format_value(p[0]), p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::LIGHT_BLUE);
                    plot_ui.text(Text::new("p2_lbl", PlotPoint::new(p[0], p[1]), rich));
                }
                if let (Some(p1), Some(p2)) = (self.point_selection.selected_p1, self.point_selection.selected_p2) {
                    plot_ui.line(Line::new("delta", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let slope = if dx.abs() > 1e-12 { dy / dx } else { f64::INFINITY };
                    let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                    let overlay = if slope.is_finite() { format!("Δx={:.4}\nΔy={:.4}\nslope={:.4}", dx, dy, slope) } else { format!("Δx=0\nΔy={:.4}\nslope=∞", dy) };
                    let rich = egui::RichText::new(overlay).size(marker_font_size).color(Color32::LIGHT_GREEN);
                    plot_ui.text(Text::new("delta_lbl", PlotPoint::new(mid[0], mid[1]), rich));
                }
            });
            // Handle click for selection in multi mode
            if plot_response.response.clicked() {
                if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                    let transform = plot_response.transform;
                    let plot_pos = transform.value_from_position(screen_pos);
                    match (&selected_trace_name, &sel_data_points) {
                        (Some(_), Some(data_points)) if !data_points.is_empty() => {
                            let mut best_i = 0usize;
                            let mut best_d2 = f64::INFINITY;
                            for (i, p) in data_points.iter().enumerate() {
                                let dx = p[0] - plot_pos.x;
                                let dy = p[1] - plot_pos.y;
                                let d2 = dx*dx + dy*dy;
                                if d2 < best_d2 { best_d2 = d2; best_i = i; }
                            }
                            let p = data_points[best_i];
                            self.point_selection.handle_click_point(p);
                        },
                        _ => {
                            self.point_selection.handle_click_point([plot_pos.x, plot_pos.y]);
                        }
                    }
                }
            }
        });

        // Repaint
        ctx.request_repaint_after(Duration::from_millis(16));

        // Apply any external UI action requests (pause/resume/screenshot)
        if let Some(ctrl) = &self.ui_action_controller {
            let mut inner = ctrl.inner.lock().unwrap();
            if let Some(want_pause) = inner.request_pause.take() {
                if want_pause && !self.paused {
                    for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); }
                    self.paused = true;
                } else if !want_pause && self.paused {
                    self.paused = false;
                    for tr in self.traces.values_mut() { tr.snap = None; }
                }
            }
            if inner.request_screenshot {
                inner.request_screenshot = false;
                self.request_window_shot = true;
            }
            if let Some(path) = inner.request_screenshot_to.take() {
                // Request a screenshot, then save to given path when event arrives
                self.request_window_shot = true;
                drop(inner);
                // Poll for the next screenshot event shortly after
                // We hook into the same event processing below; saving to explicit path is handled there.
                // Store target path for one-shot save by temporarily stashing in last_viewport_capture path via env.
                std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path);
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(fmt) = inner.request_save_raw.take() {
                drop(inner); // avoid holding the lock during file dialog/IO
                let mut dlg = rfd::FileDialog::new();
                dlg = dlg.add_filter("CSV", &["csv"]);
                #[cfg(feature = "parquet")]
                { dlg = dlg.add_filter("Parquet", &["parquet"]); }
                if let Some(path) = dlg.save_file() {
                    if let Err(e) = save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) { eprintln!("Failed to save raw data: {e}"); }
                }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some((fmt, path)) = inner.request_save_raw_to.take() {
                drop(inner);
                if let Err(e) = save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) { eprintln!("Failed to save raw data: {e}"); }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(req) = inner.fft_request.take() {
                // Gather the requested trace's time-domain data and notify listeners
                let name_opt = match req { FftDataRequest::CurrentTrace => self.selection_trace.clone(), FftDataRequest::NamedTrace(s) => Some(s), };
                if let Some(name) = name_opt {
                    if let Some(tr) = self.traces.get(&name) {
                        let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
                        let data: Vec<[f64;2]> = iter.cloned().collect();
                        let msg = FftRawData { trace: name.clone(), data };
                        inner.fft_listeners.retain(|s| s.send(msg.clone()).is_ok());
                    }
                }
            }
        }

        // Window controller: publish current window info and apply any pending requests.
        if let Some(ctrl) = &self.window_controller {
            let rect = ctx.input(|i| i.screen_rect);
            let ppp = ctx.pixels_per_point();
            let mut inner = ctrl.inner.lock().unwrap();
            // Read current size/pos (best-effort)
            let size_pts = rect.size();
            inner.current_size = Some([size_pts.x * ppp, size_pts.y * ppp]);
            inner.current_pos = Some([rect.min.x * ppp, rect.min.y * ppp]);

            // Apply size/pos requests (physical px -> egui points)
            if let Some(size_px) = inner.request_set_size.take() {
                let size_pts = [size_px[0] / ppp, size_px[1] / ppp];
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size_pts.into()));
            }
            if let Some(pos_px) = inner.request_set_pos.take() {
                let pos_pts = [pos_px[0] / ppp, pos_px[1] / ppp];
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos_pts.into()));
            }

            let info = WindowInfo { current_size: inner.current_size, current_pos: inner.current_pos, requested_size: inner.request_set_size, requested_pos: inner.request_set_pos };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // Screenshot request
        if self.request_window_shot { self.request_window_shot = false; ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default())); }
        if let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        }) {
            self.last_viewport_capture = Some(image_arc.clone());
            // Save to explicit path if requested via env hook; else prompt user
            if let Ok(path_str) = std::env::var("LIVEPLOT_SAVE_SCREENSHOT_TO") {
                std::env::remove_var("LIVEPLOT_SAVE_SCREENSHOT_TO");
                let path = std::path::PathBuf::from(path_str);
                let egui::ColorImage { size: [w, h], pixels, .. } = &*image_arc;
                let mut out = RgbaImage::new(*w as u32, *h as u32);
                for y in 0..*h { for x in 0..*w {
                    let p = pixels[y * *w + x];
                    out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()]));
                }}
                if let Err(e) = out.save(&path) { eprintln!("Failed to save viewport screenshot: {e}"); } else { eprintln!("Saved viewport screenshot to {:?}", path); }
            } else {
                let default_name = format!("viewport_{:.0}.png", chrono::Local::now().timestamp_millis());
                if let Some(path) = rfd::FileDialog::new().set_file_name(&default_name).save_file() {
                    let egui::ColorImage { size: [w, h], pixels, .. } = &*image_arc;
                    let mut out = RgbaImage::new(*w as u32, *h as u32);
                    for y in 0..*h { for x in 0..*w {
                        let p = pixels[y * *w + x];
                        out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()]));
                    }}
                    if let Err(e) = out.save(&path) { eprintln!("Failed to save viewport screenshot: {e}"); } else { eprintln!("Saved viewport screenshot to {:?}", path); }
                }
            }
        }
    }
}

/// Save all traces to path in the chosen format. If paused and snapshots exist, export snapshots; otherwise export live buffers.
fn save_raw_data_to_path(
    fmt: RawExportFormat,
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match fmt {
        RawExportFormat::Csv => save_as_csv(path, paused, traces, trace_order),
        RawExportFormat::Parquet => save_as_parquet(path, paused, traces, trace_order),
    }
}

fn save_as_csv(
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build series map of the currently exported buffers (paused => snapshot if present)
    let mut series: HashMap<String, Vec<[f64;2]>> = HashMap::new();
    for name in trace_order.iter() {
        if let Some(tr) = traces.get(name) {
            let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            let vec: Vec<[f64;2]> = iter.cloned().collect();
            series.insert(name.clone(), vec);
        }
    }
    // Tolerance fixed to 1e-9 seconds
    export::write_csv_aligned_path(path, trace_order, &series, 1e-9)?;
    Ok(())
}

fn save_as_parquet(
    path: &std::path::Path,
    paused: bool,
    traces: &std::collections::HashMap<String, TraceState>,
    trace_order: &Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "parquet")]
    {
        // Build series map of the currently exported buffers (paused => snapshot if present)
        let mut series: HashMap<String, Vec<[f64;2]>> = HashMap::new();
        for name in trace_order.iter() {
            if let Some(tr) = traces.get(name) {
                let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
                let vec: Vec<[f64;2]> = iter.cloned().collect();
                series.insert(name.clone(), vec);
            }
        }
        export::write_parquet_aligned_path(path, trace_order, &series, 1e-9)?;
        return Ok(());
    }
    #[cfg(not(feature = "parquet"))]
    {
        let _ = (path, paused, traces, trace_order);
        Err("Parquet export not available: build with feature `parquet`".into())
    }
}

/// Run the multi-trace plotting UI with default window title and size.
/// Unified entry point to run the LivePlot multi-trace UI.
pub fn run_liveplot(rx: Receiver<MultiSample>, cfg: crate::config::LivePlotConfig) -> eframe::Result<()> {
    let mut options = cfg.native_options.unwrap_or_else(eframe::NativeOptions::default);
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    let title = cfg.title.clone().unwrap_or_else(|| "LivePlot (multi)".to_string());
    eframe::run_native(&title, options, Box::new(move |_cc| {
        Ok(Box::new({
            let mut app = ScopeAppMulti::new(rx);
            // Set config-derived values
            app.time_window = cfg.time_window_secs;
            app.max_points = cfg.max_points;
            app.x_date_format = cfg.x_date_format;
            // Attach optional controllers
            app.window_controller = cfg.window_controller.clone();
            app.fft_controller = cfg.fft_controller.clone();
            app.ui_action_controller = cfg.ui_action_controller.clone();
            app.threshold_controller = cfg.threshold_controller.clone();
            app
        }))
    }))
}

/// Save a list of threshold events to a CSV file with columns:
/// end_time_seconds,threshold,trace,start_time_seconds,duration_seconds,area
fn save_threshold_events_csv(path: &std::path::Path, events: &[&ThresholdEvent]) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "end_time_seconds,threshold,trace,start_time_seconds,duration_seconds,area")?;
    for e in events {
        writeln!(f, "{:.9},{},{},{:.9},{:.9},{:.9}", e.end_t, e.threshold, e.trace, e.start_t, e.duration, e.area)?;
    }
    Ok(())
}
