//! Core multi-trace oscilloscope app wiring.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use eframe::{self, egui};
use egui::Color32;
use egui_plot::{Legend, Line, Plot, PlotPoint, PlotPoints, Points, Text};
use image::{Rgba, RgbaImage};

#[cfg(feature = "fft")]
use crate::controllers::FftPanelInfo;
use crate::controllers::{
    FftController, FftDataRequest, FftRawData, RawExportFormat, TraceInfo, TracesController,
    TracesInfo, UiActionController, WindowController, WindowInfo,
};
#[cfg(feature = "fft")]
use crate::fft;
#[cfg(feature = "fft")]
pub use crate::fft::FftWindow;
#[cfg(not(feature = "fft"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FftWindow {
    Rect,
    Hann,
    Hamming,
    Blackman,
}
use crate::config::XDateFormat;
use crate::math::{compute_math_trace, MathRuntimeState, MathTraceDef};
use crate::point_selection::PointSelection;
use crate::sink::MultiSample;
use crate::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdRuntimeState};

use super::types::{MathBuilderState, ThresholdBuilderState, TraceState};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ControlsMode {
    Embedded,
    Main,
}

/// Egui app that displays multiple traces and supports point selection and FFT.
pub struct ScopeAppMulti {
    pub rx: Receiver<MultiSample>,
    pub(super) traces: HashMap<String, TraceState>,
    pub trace_order: Vec<String>,
    pub max_points: usize,
    pub time_window: f64,
    // Dynamic slider bounds for time window (seconds)
    pub time_window_min: f64,
    pub time_window_max: f64,
    // Text/Drag numeric entry next to slider
    pub time_window_input: f64,
    // Internal: track drag state for time slider to detect release
    pub time_slider_dragging: bool,
    pub last_prune: std::time::Instant,
    pub reset_view: bool,
    pub paused: bool,
    /// Optional controller to let external code get/set/listen to window info.
    pub window_controller: Option<WindowController>,
    /// Optional controller to get/set/listen to FFT panel info
    pub fft_controller: Option<FftController>,
    /// Optional controller for high-level UI actions (pause/resume/screenshot)
    pub ui_action_controller: Option<UiActionController>,
    /// Optional controller to observe and modify trace colors/visibility/marker selection
    pub traces_controller: Option<TracesController>,
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
    /// Optional unit label for Y axis and value readouts
    pub y_unit: Option<String>,
    /// Whether to display Y axis in log10 scale (applied after per-trace offset)
    pub y_log: bool,
    // Manual Y-axis limits; when set, Y is locked to [y_min, y_max]
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    // Inputs for manual Y bounds
    pub y_min_input: f64,
    pub y_max_input: f64,
    // One-shot flag to compute Y-auto from current view
    pub pending_auto_y: bool,
    // Math traces
    pub math_defs: Vec<MathTraceDef>,
    pub(super) math_states: HashMap<String, MathRuntimeState>,
    pub(super) show_math_dialog: bool,
    pub(super) show_traces_dialog: bool,
    pub(super) math_builder: MathBuilderState,
    pub(super) math_editing: Option<String>,
    pub(super) math_error: Option<String>,
    // Thresholds
    pub threshold_controller: Option<ThresholdController>,
    pub threshold_defs: Vec<ThresholdDef>,
    pub(super) threshold_states: HashMap<String, ThresholdRuntimeState>,
    pub(super) show_thresholds_dialog: bool,
    pub(super) thr_builder: ThresholdBuilderState,
    pub(super) thr_editing: Option<String>,
    pub(super) thr_error: Option<String>,
    // Threshold events (global)
    /// Total number of threshold exceed events observed since app start (never capped).
    pub threshold_total_count: u64,
    /// Global rolling log of recent threshold events (for the UI table).
    pub(super) threshold_event_log: VecDeque<ThresholdEvent>,
    /// Maximum number of events to keep in the global UI log (prevents unbounded memory growth).
    pub(super) threshold_event_log_cap: usize,
    /// Optional filter for the events table (None = all thresholds, Some(name) = only that threshold).
    pub(super) threshold_events_filter: Option<String>,
    /// Currently hovered trace name for UI highlighting
    pub(super) hover_trace: Option<String>,
}

impl ScopeAppMulti {
    /// Drain incoming samples and append to per-trace buffers. Create traces on first sighting.
    fn drain_rx_and_update_traces(&mut self) {
        while let Ok(s) = self.rx.try_recv() {
            let is_new = !self.traces.contains_key(&s.trace);
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                TraceState {
                    name: s.trace.clone(),
                    color: Self::alloc_color(idx),
                    visible: true,
                    offset: 0.0,
                    live: VecDeque::new(),
                    snap: None,
                    last_fft: None,
                    is_math: false,
                }
            });
            if is_new && self.selection_trace.is_none() {
                self.selection_trace = Some(s.trace.clone());
            }
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            if entry.live.len() > self.max_points {
                entry.live.pop_front();
            }
        }
    }

    /// Prune each live buffer by a margin beyond the visible window to cap memory.
    fn prune_by_time_window(&mut self) {
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            for (_k, tr) in self.traces.iter_mut() {
                if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                    let cutoff = t_latest - self.time_window * 1.15;
                    while let Some(&[t, _]) = tr.live.front() {
                        if t < cutoff {
                            tr.live.pop_front();
                        } else {
                            break;
                        }
                    }
                }
            }
            self.last_prune = std::time::Instant::now();
        }
    }

    /// Apply threshold controller add/remove requests.
    fn apply_threshold_controller_requests(&mut self) {
        if let Some(ctrl) = &self.threshold_controller {
            let (adds, removes) = {
                let mut inner = ctrl.inner.lock().unwrap();
                let adds: Vec<ThresholdDef> = inner.add_requests.drain(..).collect();
                let removes: Vec<String> = inner.remove_requests.drain(..).collect();
                (adds, removes)
            };
            for def in adds {
                self.add_threshold_internal(def);
            }
            for name in removes {
                self.remove_threshold_internal(&name);
            }
        }
    }

    /// Apply trace controller requests and publish snapshot to listeners.
    fn apply_traces_controller_requests_and_publish(&mut self) {
        if let Some(ctrl) = &self.traces_controller {
            // Apply incoming requests first
            {
                let mut inner = ctrl.inner.lock().unwrap();
                for (name, rgb) in inner.color_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    }
                }
                for (name, vis) in inner.visible_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.visible = vis;
                    }
                }
                for (name, off) in inner.offset_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.offset = off;
                    }
                }
                if let Some(sel) = inner.selection_request.take() {
                    self.selection_trace = sel;
                }
                if let Some(unit_opt) = inner.y_unit_request.take() {
                    self.y_unit = unit_opt;
                }
                if let Some(ylog) = inner.y_log_request.take() {
                    self.y_log = ylog;
                }
            }
            // Publish snapshot
            let traces: Vec<TraceInfo> = self
                .trace_order
                .iter()
                .filter_map(|n| {
                    self.traces.get(n).map(|tr| TraceInfo {
                        name: tr.name.clone(),
                        color_rgb: [tr.color.r(), tr.color.g(), tr.color.b()],
                        visible: tr.visible,
                        is_math: tr.is_math,
                        offset: tr.offset,
                    })
                })
                .collect();
            let info = TracesInfo {
                traces,
                marker_selection: self.selection_trace.clone(),
                y_unit: self.y_unit.clone(),
                y_log: self.y_log,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }

    /// One-shot shared update: ingest -> prune -> recompute math -> thresholds -> traces publish
    fn tick_non_ui(&mut self) {
        self.drain_rx_and_update_traces();
        self.prune_by_time_window();
        self.recompute_math_traces();
        self.apply_threshold_controller_requests();
        self.process_thresholds();
        self.apply_traces_controller_requests_and_publish();
    }

    /// Show any open dialogs in a shared way.
    fn show_dialogs_shared(&mut self, ctx: &egui::Context) {
        if self.show_math_dialog {
            super::math_ui::show_math_dialog(self, ctx);
        }
        if self.show_traces_dialog {
            super::traces_ui::show_traces_dialog(self, ctx);
        }
        if self.show_thresholds_dialog {
            super::thresholds_ui::show_thresholds_dialog(self, ctx);
        }
    }

    /// Compute latest overall time across traces respecting paused state.
    fn latest_time_overall(&self) -> Option<f64> {
        let mut t_latest_overall = f64::NEG_INFINITY;
        for name in self.trace_order.iter() {
            if let Some(tr) = self.traces.get(name) {
                let last_t = if self.paused {
                    tr.snap.as_ref().and_then(|s| s.back()).map(|p| p[0])
                } else {
                    tr.live.back().map(|p| p[0])
                };
                if let Some(t) = last_t {
                    if t > t_latest_overall {
                        t_latest_overall = t;
                    }
                }
            }
        }
        if t_latest_overall.is_finite() { Some(t_latest_overall) } else { None }
    }

    /// Shared plot for both embedded and main variants. Returns (x_width, zoomed) and full response.
    fn plot_traces_common(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        plot_id: &str,
    ) -> egui_plot::PlotResponse<(f64, bool)> {
        let mut plot = Plot::new(plot_id)
            .allow_scroll(false)
            .allow_zoom(true)
            .allow_boxed_zoom(true)
            .x_axis_formatter(|x, _range| {
                let val = x.value;
                let secs = val as i64;
                let nsecs = ((val - secs as f64) * 1e9) as u32;
                let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                dt_utc
                    .with_timezone(&Local)
                    .format("%H:%M:%S")
                    .to_string()
            })
            .y_axis_formatter(|y, _range| {
                let v = y.value;
                let label_val = if self.y_log { 10f64.powf(v) } else { v };
                if let Some(unit) = &self.y_unit {
                    format!("{:.3} {}", label_val, unit)
                } else {
                    format!("{:.3}", label_val)
                }
            });
        if self.reset_view {
            plot = plot.reset();
            self.reset_view = false;
        }
        // Determine desired x-bounds for follow
        let x_bounds = self
            .latest_time_overall()
            .map(|t_latest| (t_latest - self.time_window, t_latest));
        if self.traces.len() > 1 {
            plot = plot.legend(Legend::default());
        }
        let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
        let marker_font_size = base_body * 1.5;
        plot.show(ui, |plot_ui| {
            let resp = plot_ui.response();
            // Detect wheel zoom over the plot
            let mut is_zooming = false;
            plot_ui.ctx().input(|i| {
                let d = i.raw_scroll_delta;
                if (d.x != 0.0 || d.y != 0.0) && resp.hovered() {
                    is_zooming = true;
                }
            });
            let interacting = resp.dragged() || resp.is_pointer_button_down_on();
            let suppress_follow = interacting || is_zooming;
            if !self.paused && !suppress_follow {
                if let Some((xmin, xmax)) = x_bounds {
                    if xmin < xmax {
                        plot_ui.set_plot_bounds_x(xmin..=xmax);
                    }
                }
            }

            // Apply manual Y or pending auto Y
            if let (Some(y0), Some(y1)) = (self.y_min, self.y_max) {
                if y0.is_finite() && y1.is_finite() && y0 < y1 {
                    plot_ui.set_plot_bounds_y(y0..=y1);
                }
            } else if self.pending_auto_y {
                let mut ymin = f64::INFINITY;
                let mut ymax = f64::NEG_INFINITY;
                for tr in self.traces.values() {
                    if !tr.visible {
                        continue;
                    }
                    for p in tr.live.iter() {
                        let y_lin = p[1] + tr.offset;
                        let y = if self.y_log {
                            if y_lin > 0.0 {
                                y_lin.log10()
                            } else {
                                continue;
                            }
                        } else {
                            y_lin
                        };
                        if y < ymin {
                            ymin = y;
                        }
                        if y > ymax {
                            ymax = y;
                        }
                    }
                }
                if ymin.is_finite() && ymax.is_finite() && ymin < ymax {
                    plot_ui.set_plot_bounds_y(ymin..=ymax);
                }
                self.pending_auto_y = false;
            }

            // Lines
            for name in self.trace_order.clone().into_iter() {
                if let Some(tr) = self.traces.get(&name) {
                    if !tr.visible {
                        continue;
                    }
                    let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                        if let Some(snap) = &tr.snap {
                            Box::new(snap.iter())
                        } else {
                            Box::new(tr.live.iter())
                        }
                    } else {
                        Box::new(tr.live.iter())
                    };
                    let pts: PlotPoints = iter
                        .map(|p| {
                            let y_lin = p[1] + tr.offset;
                            let y = if self.y_log {
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                y_lin
                            };
                            [p[0], y]
                        })
                        .collect();
                    let mut color = tr.color;
                    if let Some(hov) = &self.hover_trace {
                        if &tr.name != hov {
                            color = Color32::from_rgba_unmultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                80,
                            );
                        }
                    }
                    let mut line = Line::new(&tr.name, pts).color(color);
                    if self.traces.len() > 1 {
                        line = line.name(&tr.name);
                    }
                    plot_ui.line(line);
                }
            }

            // Shared selection overlays
            if let Some(p) = self.point_selection.selected_p1 {
                plot_ui.points(Points::new("", vec![p]).radius(5.0).color(Color32::YELLOW));
                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit {
                    format!("{:.4} {}", y_lin, u)
                } else {
                    format!("{:.4}", y_lin)
                };
                let txt = format!(
                    "P1\nx={}\ny={}",
                    self.x_date_format.format_value(p[0]),
                    ytxt
                );
                let rich = egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::YELLOW);
                plot_ui.text(Text::new("p1_lbl", PlotPoint::new(p[0], p[1]), rich));
            }
            if let Some(p) = self.point_selection.selected_p2 {
                plot_ui.points(
                    Points::new("", vec![p]).radius(5.0).color(Color32::LIGHT_BLUE),
                );
                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit {
                    format!("{:.4} {}", y_lin, u)
                } else {
                    format!("{:.4}", y_lin)
                };
                let txt = format!(
                    "P2\nx={}\ny={}",
                    self.x_date_format.format_value(p[0]),
                    ytxt
                );
                let rich = egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_BLUE);
                plot_ui.text(Text::new("p2_lbl", PlotPoint::new(p[0], p[1]), rich));
            }
            if let (Some(p1), Some(p2)) = (self.point_selection.selected_p1, self.point_selection.selected_p2) {
                plot_ui.line(Line::new("delta", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                let dx = p2[0] - p1[0];
                let y1 = if self.y_log { 10f64.powf(p1[1]) } else { p1[1] };
                let y2 = if self.y_log { 10f64.powf(p2[1]) } else { p2[1] };
                let dy = y2 - y1;
                let slope = if dx.abs() > 1e-12 { dy / dx } else { f64::INFINITY };
                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                let dy_txt = if let Some(u) = &self.y_unit {
                    format!("{:.4} {}", dy, u)
                } else {
                    format!("{:.4}", dy)
                };
                let overlay = if slope.is_finite() {
                    format!("Δx={:.4}\nΔy={}\nslope={:.4}", dx, dy_txt, slope)
                } else {
                    format!("Δx=0\nΔy={}\nslope=∞", dy_txt)
                };
                let rich = egui::RichText::new(overlay)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_GREEN);
                plot_ui.text(Text::new("delta_lbl", PlotPoint::new(mid[0], mid[1]), rich));
            }

            let w = {
                let b = plot_ui.plot_bounds();
                let r = b.range_x();
                let (a, b) = (*r.start(), *r.end());
                (b - a).abs()
            };
            (w, is_zooming)
        })
    }

    /// Sync time window with zoom when following is active (not paused)
    fn sync_time_window_with_plot(&mut self, plot_response: &egui_plot::PlotResponse<(f64, bool)>) {
        if !self.paused && !self.time_slider_dragging {
            let (w, zoomed) = plot_response.inner;
            if zoomed
                && w.is_finite()
                && w > 0.0
                && (w - self.time_window).abs() / self.time_window.max(1e-6) > 0.02
            {
                self.time_window = w;
            }
        }
    }

    /// Handle click selection on the plot using nearest point logic.
    fn handle_plot_click(&mut self, plot_response: &egui_plot::PlotResponse<(f64, bool)>) {
        if plot_response.response.clicked() {
            if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                let transform = plot_response.transform;
                let plot_pos = transform.value_from_position(screen_pos);
                let selected_trace_name = self.selection_trace.clone();
                let sel_data_points: Option<Vec<[f64; 2]>> = if let Some(name) = &selected_trace_name {
                    self.traces.get(name).map(|tr| {
                        let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap {
                                Box::new(snap.iter())
                            } else {
                                Box::new(tr.live.iter())
                            }
                        } else {
                            Box::new(tr.live.iter())
                        };
                        iter.cloned().collect()
                    })
                } else {
                    None
                };
                match (&selected_trace_name, &sel_data_points) {
                    (Some(name), Some(data_points)) if !data_points.is_empty() => {
                        let off = self.traces.get(name).map(|t| t.offset).unwrap_or(0.0);
                        let mut best_i = None;
                        let mut best_d2 = f64::INFINITY;
                        for (i, p) in data_points.iter().enumerate() {
                            let x = p[0];
                            let y_lin = p[1] + off;
                            let y_plot = if self.y_log {
                                if y_lin > 0.0 { y_lin.log10() } else { continue; }
                            } else { y_lin };
                            let dx = x - plot_pos.x;
                            let dy = y_plot - plot_pos.y;
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_d2 {
                                best_d2 = d2;
                                best_i = Some(i);
                            }
                        }
                        if let Some(i) = best_i {
                            let p = data_points[i];
                            let y_lin = p[1] + off;
                            let y_plot = if self.y_log { y_lin.log10() } else { y_lin };
                            self.point_selection.handle_click_point([p[0], y_plot]);
                        }
                    }
                    _ => {
                        self
                            .point_selection
                            .handle_click_point([plot_pos.x, plot_pos.y]);
                    }
                }
            }
        }
    }

    fn controls_ui(&mut self, ui: &mut egui::Ui, mode: ControlsMode) {
        // Heading/help lines differ slightly by mode
        match mode {
            ControlsMode::Main => {
                ui.heading("LivePlot (multi)");
                ui.label("Left mouse: pan  |  Right drag: zoom box");
            }
            ControlsMode::Embedded => {
                ui.label("LivePlot");
                ui.label("Left mouse: pan  |  Right drag: zoom box");
            }
        }

        ui.horizontal(|ui| {
            // Time window slider (shared)
            ui.label("Time window (s):");
            let mut tw = self.time_window;
            let slider = egui::Slider::new(&mut tw, self.time_window_min..=self.time_window_max)
                .logarithmic(true)
                .smart_aim(false)
                .show_value(true);
            let sresp = ui.add(slider);
            if sresp.changed() {
                self.time_window = tw.max(1e-6);
            }
            // Expand bounds only on release
            let was_drag = self.time_slider_dragging;
            let now_drag = sresp.is_pointer_button_down_on();
            self.time_slider_dragging = now_drag;
            if was_drag && !now_drag {
                let v = self.time_window;
                if v <= self.time_window_min || v >= self.time_window_max {
                    self.time_window_min = (v / 10.0).max(1e-6);
                    self.time_window_max = v * 10.0;
                }
            }

            // Points cap
            ui.label("Points cap:");
            ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));

            // Y controls (shared)
            if ui.button("Auto Zoom Y-Axis").clicked() {
                self.pending_auto_y = true;
            }
            ui.label("Y min / max:");
            let mut y_min_tmp = self.y_min.unwrap_or(self.y_min_input);
            let mut y_max_tmp = self.y_max.unwrap_or(self.y_max_input);
            let r1 = ui.add(egui::DragValue::new(&mut y_min_tmp).speed(0.1));
            ui.add_space(6.0);
            let r2 = ui.add(egui::DragValue::new(&mut y_max_tmp).speed(0.1));
            if r1.changed() || r2.changed() {
                self.y_min_input = y_min_tmp;
                self.y_max_input = y_max_tmp;
            }
            ui.add_space(6.0);
            if ui.button("Apply Y").clicked() {
                if self.y_min_input < self.y_max_input {
                    self.y_min = Some(self.y_min_input);
                    self.y_max = Some(self.y_max_input);
                }
            }
            ui.add_space(6.0);
            if ui.button("Clear Y").clicked() {
                self.y_min = None;
                self.y_max = None;
            }

            // Selection + pause/reset/clear (shared)
            if ui.button("Clear Selection").clicked() {
                self.point_selection.clear();
            }
            if ui
                .button(if self.paused { "Resume" } else { "Pause" })
                .clicked()
            {
                if self.paused {
                    self.paused = false;
                    for tr in self.traces.values_mut() {
                        tr.snap = None;
                    }
                } else {
                    for tr in self.traces.values_mut() {
                        tr.snap = Some(tr.live.clone());
                    }
                    self.paused = true;
                }
            }
            if ui.button("Reset View").clicked() {
                self.reset_view = true;
            }
            if ui.button("Clear").clicked() {
                for tr in self.traces.values_mut() {
                    tr.live.clear();
                    if let Some(s) = &mut tr.snap {
                        s.clear();
                    }
                }
            }

            // Optional extras: FFT toggle, Save PNG, Save raw (Main only)
            //if let ControlsMode::Main = mode {
            #[cfg(feature = "fft")]
            if ui
                .button(if self.show_fft {
                    "Hide FFT"
                } else {
                    "Show FFT"
                })
                .clicked()
            {
                self.show_fft = !self.show_fft;
                if let Some(ctrl) = &self.fft_controller {
                    let mut inner = ctrl.inner.lock().unwrap();
                    inner.show = self.show_fft;
                    let info = FftPanelInfo {
                        shown: inner.show,
                        current_size: inner.current_size,
                        requested_size: inner.request_set_size,
                    };
                    inner.listeners.retain(|s| s.send(info.clone()).is_ok());
                }
            }
            #[cfg(not(feature = "fft"))]
            {
                let _ = (FftWindow::Rect,);
            }

            if ui
                .button("Save PNG")
                .on_hover_text("Take an egui viewport screenshot")
                .clicked()
            {
                self.request_window_shot = true;
            }

            let hover_text: &str = {
                #[cfg(feature = "parquet")]
                {
                    "Export all traces as CSV or Parquet"
                }
                #[cfg(not(feature = "parquet"))]
                {
                    "Export all traces as CSV"
                }
            };
            if ui
                .button("Save raw data")
                .on_hover_text(hover_text)
                .clicked()
            {
                let mut dlg = rfd::FileDialog::new();
                dlg = dlg.add_filter("CSV", &["csv"]);
                #[cfg(feature = "parquet")]
                {
                    dlg = dlg.add_filter("Parquet", &["parquet"]);
                }
                if let Some(path) = dlg.set_file_name("liveplot_export.csv").save_file() {
                    let fmt = {
                        #[cfg(feature = "parquet")]
                        {
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            if ext.eq_ignore_ascii_case("parquet") {
                                RawExportFormat::Parquet
                            } else {
                                RawExportFormat::Csv
                            }
                        }
                        #[cfg(not(feature = "parquet"))]
                        {
                            RawExportFormat::Csv
                        }
                    };
                    if let Err(e) = super::export_helpers::save_raw_data_to_path(
                        fmt,
                        &path,
                        self.paused,
                        &self.traces,
                        &self.trace_order,
                    ) {
                        eprintln!("Failed to save raw data: {e}");
                    }
                }
            }
            //}

            // Dialogs entry (shared)
            if ui
                .button("Math…")
                .on_hover_text("Create and manage math traces")
                .clicked()
            {
                self.show_math_dialog = true;
            }
            if ui
                .button("Traces…")
                .on_hover_text("Select marker/display and colors")
                .clicked()
            {
                self.show_traces_dialog = true;
            }
            let thr_btn_label = if self.threshold_total_count > 0 {
                format!("Thresholds… ({})", self.threshold_total_count)
            } else {
                "Thresholds…".to_string()
            };
            if ui
                .button(thr_btn_label)
                .on_hover_text("Create and manage threshold detectors")
                .clicked()
            {
                self.show_thresholds_dialog = true;
            }
        });
    }
    pub fn new(rx: Receiver<MultiSample>) -> Self {
        Self {
            rx,
            traces: HashMap::new(),
            trace_order: Vec::new(),
            max_points: 10_000,
            time_window: 10.0,
            time_window_min: 1.0,
            time_window_max: 100.0,
            time_window_input: 10.0,
            time_slider_dragging: false,
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
            traces_controller: None,
            request_window_shot: false,
            last_viewport_capture: None,
            selection_trace: None,
            point_selection: PointSelection::default(),
            x_date_format: XDateFormat::default(),
            y_unit: None,
            y_log: false,
            y_min: None,
            y_max: None,
            y_min_input: 0.0,
            y_max_input: 1.0,
            pending_auto_y: false,
            math_defs: Vec::new(),
            math_states: HashMap::new(),
            show_math_dialog: false,
            show_traces_dialog: false,
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
            hover_trace: None,
        }
    }

    /// Render the LivePlot UI into an arbitrary egui container (e.g., inside an egui::Window).
    ///
    /// This variant avoids using global panels and viewport commands, making it suitable
    /// for embedding into another application's UI.
    pub fn ui_embed(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Shared non-UI tick
        self.tick_non_ui();

        // Controls (embedded friendly) via unified renderer
        ui.vertical(|ui| {
            self.controls_ui(ui, ControlsMode::Embedded);
        });

        // Dialogs
        self.show_dialogs_shared(&ctx);

        // Plot all traces within the provided Ui
        let plot_response = self.plot_traces_common(ui, &ctx, "scope_plot_multi_embedded");

        // Sync time window with zoom when following is active (not paused)
        self.sync_time_window_with_plot(&plot_response);

        // Handle click for selection in embedded mode
        self.handle_plot_click(&plot_response);

        // Repaint soon
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    pub(crate) fn add_math_trace_internal(&mut self, def: MathTraceDef) {
        if self.traces.contains_key(&def.name) {
            return;
        }
        let idx = self.trace_order.len();
        self.trace_order.push(def.name.clone());
        let color = Self::alloc_color(idx);
        self.traces.insert(
            def.name.clone(),
            TraceState {
                name: def.name.clone(),
                color,
                visible: true,
                offset: 0.0,
                live: VecDeque::new(),
                snap: None,
                last_fft: None,
                is_math: true,
            },
        );
        self.math_states
            .entry(def.name.clone())
            .or_insert_with(MathRuntimeState::new);
        self.math_defs.push(def);
    }

    pub(crate) fn remove_math_trace_internal(&mut self, name: &str) {
        self.math_defs.retain(|d| d.name != name);
        self.math_states.remove(name);
        self.traces.remove(name);
        self.trace_order.retain(|n| n != name);
    }

    /// Public API: add a math trace definition (creates a new virtual trace that auto-updates).
    pub fn add_math_trace(&mut self, def: MathTraceDef) {
        self.add_math_trace_internal(def);
    }

    /// Public API: remove a previously added math trace by name.
    pub fn remove_math_trace(&mut self, name: &str) {
        self.remove_math_trace_internal(name);
    }

    /// Public API: list current math trace definitions.
    pub fn math_traces(&self) -> &[MathTraceDef] {
        &self.math_defs
    }

    fn recompute_math_traces(&mut self) {
        if self.math_defs.is_empty() {
            return;
        }
        // Build sources from existing traces (prefer snapshot when paused)
        let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                if let Some(s) = &tr.snap {
                    Box::new(s.iter())
                } else {
                    Box::new(tr.live.iter())
                }
            } else {
                Box::new(tr.live.iter())
            };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        // Compute each math def in insertion order; allow math-of-math using updated sources.
        for def in &self.math_defs.clone() {
            let st = self
                .math_states
                .entry(def.name.clone())
                .or_insert_with(MathRuntimeState::new);
            // Provide previous output (from sources) and prune cutoff (based on time window)
            let prev_out = sources.get(&def.name).map(|v| v.as_slice());
            let prune_cut = {
                let latest = self
                    .trace_order
                    .iter()
                    .filter_map(|n| sources.get(n).and_then(|v| v.last().map(|p| p[0])))
                    .fold(f64::NEG_INFINITY, f64::max);
                if latest.is_finite() {
                    Some(latest - self.time_window * 1.2)
                } else {
                    None
                }
            };
            let pts = compute_math_trace(def, &sources, prev_out, prune_cut, st);
            sources.insert(def.name.clone(), pts.clone());
            // Update backing trace buffers
            if let Some(tr) = self.traces.get_mut(&def.name) {
                tr.live = pts.iter().copied().collect();
                if self.paused {
                    tr.snap = Some(tr.live.clone());
                } else {
                    tr.snap = None;
                }
            } else {
                // Create if missing (def might have been added but no entry created)
                let idx = self.trace_order.len();
                self.trace_order.push(def.name.clone());
                let mut dq: VecDeque<[f64; 2]> = VecDeque::new();
                dq.extend(pts.iter().copied());
                self.traces.insert(
                    def.name.clone(),
                    TraceState {
                        name: def.name.clone(),
                        color: Self::alloc_color(idx),
                        visible: true,
                        offset: 0.0,
                        live: dq.clone(),
                        snap: if self.paused { Some(dq.clone()) } else { None },
                        last_fft: None,
                        is_math: true,
                    },
                );
            }
        }
    }

    fn process_thresholds(&mut self) {
        if self.threshold_defs.is_empty() {
            return;
        }
        // Build sources from existing traces (prefer snapshot when paused)
        let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                if let Some(s) = &tr.snap {
                    Box::new(s.iter())
                } else {
                    Box::new(tr.live.iter())
                }
            } else {
                Box::new(tr.live.iter())
            };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        // Process each threshold incrementally
        for def in self.threshold_defs.clone().iter() {
            let state = self
                .threshold_states
                .entry(def.name.clone())
                .or_insert_with(ThresholdRuntimeState::new);
            let data = match sources.get(&def.target.0) {
                Some(v) => v,
                None => continue,
            };
            let mut start_idx = 0usize;
            if let Some(t0) = state.prev_in_t {
                // find first strictly after t0
                start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                    Ok(mut i) => {
                        while i < data.len() && data[i][0] <= t0 {
                            i += 1;
                        }
                        i
                    }
                    Err(i) => i,
                };
            }
            for p in data.iter().skip(start_idx) {
                let t = p[0];
                let v = p[1];
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
                        let evt = ThresholdEvent {
                            threshold: def.name.clone(),
                            trace: def.target.0.clone(),
                            start_t: state.start_t,
                            end_t,
                            duration: dur,
                            area: state.accum_area,
                        };
                        state.push_event_capped(evt.clone(), def.max_events);
                        // Update global counters/log (never capped counter)
                        self.threshold_total_count = self.threshold_total_count.saturating_add(1);
                        self.threshold_event_log.push_back(evt.clone());
                        while self.threshold_event_log.len() > self.threshold_event_log_cap {
                            self.threshold_event_log.pop_front();
                        }
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

    pub(crate) fn add_threshold_internal(&mut self, def: ThresholdDef) {
        if self.threshold_defs.iter().any(|d| d.name == def.name) {
            return;
        }
        self.threshold_states
            .entry(def.name.clone())
            .or_insert_with(ThresholdRuntimeState::new);
        self.threshold_defs.push(def);
    }

    pub(crate) fn remove_threshold_internal(&mut self, name: &str) {
        self.threshold_defs.retain(|d| d.name != name);
        self.threshold_states.remove(name);
    }

    /// Public API: add/remove/list thresholds; get events for a threshold (clone)
    pub fn add_threshold(&mut self, def: ThresholdDef) {
        self.add_threshold_internal(def);
    }
    pub fn remove_threshold(&mut self, name: &str) {
        self.remove_threshold_internal(name);
    }
    pub fn thresholds(&self) -> &[ThresholdDef] {
        &self.threshold_defs
    }
    pub fn threshold_events(&self, name: &str) -> Option<Vec<ThresholdEvent>> {
        self.threshold_states
            .get(name)
            .map(|s| s.events.iter().cloned().collect())
    }

    /// Update an existing math trace definition; supports renaming if the new name is unique.
    pub fn update_math_trace(
        &mut self,
        original_name: &str,
        new_def: MathTraceDef,
    ) -> Result<(), &'static str> {
        // Name collision check if renaming
        if new_def.name != original_name && self.traces.contains_key(&new_def.name) {
            return Err("A trace with the new name already exists");
        }
        // Replace def
        if let Some(pos) = self.math_defs.iter().position(|d| d.name == original_name) {
            self.math_defs[pos] = new_def.clone();
        } else {
            return Err("Original math trace not found");
        }

        // Reset runtime state for this math trace (operation may have changed)
        self.math_states
            .insert(new_def.name.clone(), MathRuntimeState::new());
        if new_def.name != original_name {
            self.math_states.remove(original_name);
        }

        // Rename/move underlying TraceState if needed
        if new_def.name != original_name {
            if let Some(mut tr) = self.traces.remove(original_name) {
                tr.name = new_def.name.clone();
                self.traces.insert(new_def.name.clone(), tr);
            }
            // Update order and selection
            for name in &mut self.trace_order {
                if name == original_name {
                    *name = new_def.name.clone();
                    break;
                }
            }
            if let Some(sel) = &mut self.selection_trace {
                if sel == original_name {
                    *sel = new_def.name.clone();
                }
            }
        }

        // Trigger recompute on next update cycle immediately
        self.recompute_math_traces();
        Ok(())
    }

    pub(crate) fn apply_add_or_edit(&mut self, def: MathTraceDef) {
        self.math_error = None;
        if let Some(orig) = self.math_editing.clone() {
            match self.update_math_trace(&orig, def) {
                Ok(()) => {
                    self.math_editing = None;
                    self.math_builder = MathBuilderState::default();
                }
                Err(e) => {
                    self.math_error = Some(e.to_string());
                }
            }
        } else {
            if self.traces.contains_key(&def.name) {
                self.math_error = Some("A trace with this name already exists".into());
                return;
            }
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
        // Shared non-UI tick
        self.tick_non_ui();

        // Controls
        egui::TopBottomPanel::top("controls_multi").show(ctx, |ui| {
            self.controls_ui(ui, ControlsMode::Main);
        });

        // Shared dialogs
        self.show_dialogs_shared(ctx);

        // FFT bottom panel for multi-traces
        #[cfg(feature = "fft")]
        if self.show_fft {
            super::fft_panel::show_fft_panel(self, ctx);
        }
        #[cfg(not(feature = "fft"))]
        {
            let _ = ctx; // suppress unused warnings
        }

        // Plot all traces in the central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            let plot_response = self.plot_traces_common(ui, ctx, "scope_plot_multi");
            self.sync_time_window_with_plot(&plot_response);
            self.handle_plot_click(&plot_response);
        });

        // Repaint
        ctx.request_repaint_after(Duration::from_millis(16));

        // Apply any external UI action requests (pause/resume/screenshot)
        if let Some(ctrl) = &self.ui_action_controller {
            let mut inner = ctrl.inner.lock().unwrap();
            if let Some(want_pause) = inner.request_pause.take() {
                if want_pause && !self.paused {
                    for tr in self.traces.values_mut() {
                        tr.snap = Some(tr.live.clone());
                    }
                    self.paused = true;
                } else if !want_pause && self.paused {
                    self.paused = false;
                    for tr in self.traces.values_mut() {
                        tr.snap = None;
                    }
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
                std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path);
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(fmt) = inner.request_save_raw.take() {
                drop(inner); // avoid holding the lock during file dialog/IO
                let mut dlg = rfd::FileDialog::new();
                dlg = dlg.add_filter("CSV", &["csv"]);
                #[cfg(feature = "parquet")]
                {
                    dlg = dlg.add_filter("Parquet", &["parquet"]);
                }
                if let Some(path) = dlg.save_file() {
                    if let Err(e) = super::export_helpers::save_raw_data_to_path(
                        fmt,
                        &path,
                        self.paused,
                        &self.traces,
                        &self.trace_order,
                    ) {
                        eprintln!("Failed to save raw data: {e}");
                    }
                }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some((fmt, path)) = inner.request_save_raw_to.take() {
                drop(inner);
                if let Err(e) = super::export_helpers::save_raw_data_to_path(
                    fmt,
                    &path,
                    self.paused,
                    &self.traces,
                    &self.trace_order,
                ) {
                    eprintln!("Failed to save raw data: {e}");
                }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(req) = inner.fft_request.take() {
                // Gather the requested trace's time-domain data and notify listeners
                let name_opt = match req {
                    FftDataRequest::CurrentTrace => self.selection_trace.clone(),
                    FftDataRequest::NamedTrace(s) => Some(s),
                };
                if let Some(name) = name_opt {
                    if let Some(tr) = self.traces.get(&name) {
                        let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap {
                                Box::new(snap.iter())
                            } else {
                                Box::new(tr.live.iter())
                            }
                        } else {
                            Box::new(tr.live.iter())
                        };
                        let data: Vec<[f64; 2]> = iter.cloned().collect();
                        let msg = FftRawData {
                            trace: name.clone(),
                            data,
                        };
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

            let info = WindowInfo {
                current_size: inner.current_size,
                current_pos: inner.current_pos,
                requested_size: inner.request_set_size,
                requested_pos: inner.request_set_pos,
            };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // Screenshot request
        if self.request_window_shot {
            self.request_window_shot = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        }) {
            self.last_viewport_capture = Some(image_arc.clone());
            // Save to explicit path if requested via env hook; else prompt user
            if let Ok(path_str) = std::env::var("LIVEPLOT_SAVE_SCREENSHOT_TO") {
                std::env::remove_var("LIVEPLOT_SAVE_SCREENSHOT_TO");
                let path = std::path::PathBuf::from(path_str);
                let egui::ColorImage {
                    size: [w, h],
                    pixels,
                    ..
                } = &*image_arc;
                let mut out = RgbaImage::new(*w as u32, *h as u32);
                for y in 0..*h {
                    for x in 0..*w {
                        let p = pixels[y * *w + x];
                        out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()]));
                    }
                }
                if let Err(e) = out.save(&path) {
                    eprintln!("Failed to save viewport screenshot: {e}");
                } else {
                    eprintln!("Saved viewport screenshot to {:?}", path);
                }
            } else {
                let default_name = format!(
                    "viewport_{:.0}.png",
                    chrono::Local::now().timestamp_millis()
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&default_name)
                    .save_file()
                {
                    let egui::ColorImage {
                        size: [w, h],
                        pixels,
                        ..
                    } = &*image_arc;
                    let mut out = RgbaImage::new(*w as u32, *h as u32);
                    for y in 0..*h {
                        for x in 0..*w {
                            let p = pixels[y * *w + x];
                            out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()]));
                        }
                    }
                    if let Err(e) = out.save(&path) {
                        eprintln!("Failed to save viewport screenshot: {e}");
                    } else {
                        eprintln!("Saved viewport screenshot to {:?}", path);
                    }
                }
            }
        }
    }
}

/// Run the multi-trace plotting UI with default window title and size.
/// Unified entry point to run the LivePlot multi-trace UI.
pub fn run_liveplot(
    rx: Receiver<MultiSample>,
    cfg: crate::config::LivePlotConfig,
) -> eframe::Result<()> {
    let mut options = cfg
        .native_options
        .unwrap_or_else(eframe::NativeOptions::default);
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    let title = cfg
        .title
        .clone()
        .unwrap_or_else(|| "LivePlot (multi)".to_string());
    eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| {
            Ok(Box::new({
                let mut app = ScopeAppMulti::new(rx);
                // Set config-derived values
                app.time_window = cfg.time_window_secs;
                app.max_points = cfg.max_points;
                app.x_date_format = cfg.x_date_format;
                app.y_unit = cfg.y_unit.clone();
                app.y_log = cfg.y_log;
                // Attach optional controllers
                app.window_controller = cfg.window_controller.clone();
                app.fft_controller = cfg.fft_controller.clone();
                app.ui_action_controller = cfg.ui_action_controller.clone();
                app.threshold_controller = cfg.threshold_controller.clone();
                app.traces_controller = cfg.traces_controller.clone();
                app
            }))
        }),
    )
}
