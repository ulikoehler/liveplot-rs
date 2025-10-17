//! Core multi-trace oscilloscope app wiring.

use chrono::Local;
use eframe::{self, egui};
use egui::{Align2, Color32};
use egui_plot::{HLine, Legend, Line, Plot, PlotPoint, Points, Text, VLine};
use image::{Rgba, RgbaImage};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "fft")]
use crate::controllers::FftPanelInfo;
use crate::controllers::{
    FftController, FftDataRequest, FftRawData, RawExportFormat, TraceInfo, TracesController,
    TracesInfo, UiActionController, WindowController, WindowInfo,
};
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ZoomMode {
    Off,
    X,
    Y,
    Both,
}

use crate::config::XDateFormat;
use crate::math::{compute_math_trace, MathRuntimeState, MathTraceDef};
use crate::point_selection::PointSelection;
use crate::sink::MultiSample;
use crate::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdRuntimeState};

#[cfg(feature = "fft")]
use super::fft_panel::FftPanel;
use super::math_ui::MathPanel;
use super::panel::DockPanel;
use super::thresholds_ui::ThresholdsPanel;
use super::traces_ui::TracesPanel;
use super::types::{MathBuilderState, TraceState};
use super::traceslook_ui::TraceLook;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ControlsMode {
    Embedded,
    Main,
}

// Removed RightTab enum: sidebar visibility and active tab are derived from per-panel DockState

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
    pub pending_auto_x: bool,
    /// Optional unit label for Y axis and value readouts
    pub y_unit: Option<String>,
    /// Whether to display Y axis in log10 scale (applied after per-trace offset)
    pub y_log: bool,
    // Manual Y-axis limits; when set, Y is locked to [y_min, y_max]
    pub y_min: f64,
    pub y_max: f64,
    // One-shot flag to compute Y-auto from current view
    pub pending_auto_y: bool,
    pub auto_zoom_y: bool,

    pub zoom_mode: ZoomMode,

    pub show_legend: bool,
    /// If true, append trace info to legend labels
    pub show_info_in_legend: bool,
    // Math traces
    pub math_defs: Vec<MathTraceDef>,
    pub(super) math_states: HashMap<String, MathRuntimeState>,

    // Thresholds
    pub threshold_controller: Option<ThresholdController>,
    pub threshold_defs: Vec<ThresholdDef>,
    pub(super) threshold_states: HashMap<String, ThresholdRuntimeState>,

    // Unified dock state lives in per-panel state structs
    // Threshold events (global)
    /// Global rolling log of recent threshold events (for the UI table).
    pub(super) threshold_event_log: VecDeque<ThresholdEvent>,
    /// Maximum number of events to keep in the global UI log (prevents unbounded memory growth).
    pub(super) threshold_event_log_cap: usize,
    /// Currently hovered trace name for UI highlighting
    pub(super) hover_trace: Option<String>,
    /// Currently hovered threshold name for UI highlighting
    pub(super) hover_threshold: Option<String>,
    // Right panel sidebar visibility/active tab are derived from per-panel DockState
    // New per-panel state holders
    pub(super) math_panel: MathPanel,
    pub(super) thresholds_panel: ThresholdsPanel,
    pub(super) traces_panel: TracesPanel,
    #[cfg(feature = "fft")]
    pub(super) fft_panel: FftPanel,
}

impl ScopeAppMulti {
    /// Render the right-side sidebar if any attached panel is visible; includes header and body.
    fn render_right_sidebar_panel(&mut self, ctx: &egui::Context) {
        // Check if any attached side panel should be shown
        let sidebar_visible = {
            let mut panels = self.side_panels();
            panels.iter_mut().any(|p| {
                let d = p.dock_mut();
                !d.detached && d.show_dialog
            })
        };
        if !sidebar_visible {
            return;
        }
        egui::SidePanel::right("right_tabs")
            .resizable(true)
            .default_width(350.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                // Header: choose active attached panel by toggling which one is attached
                ui.horizontal(|ui| {
                    let mut clicked_idx: Option<usize> = None;
                    // Read titles and active flags first to avoid holding borrows during UI
                    let titles_flags: Vec<(&'static str, bool)> = {
                        let mut panels = self.side_panels();
                        panels
                            .iter_mut()
                            .map(|p| {
                                let d = p.dock_mut();
                                (d.title, !d.detached && d.show_dialog)
                            })
                            .collect()
                    };
                    if titles_flags.len() == 1 {
                        ui.strong(titles_flags[0].0);
                    } else {
                        for (i, (title, active)) in titles_flags.iter().enumerate() {
                            if ui.selectable_label(*active, *title).clicked() {
                                clicked_idx = Some(i);
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Hide")
                            .on_hover_text("Hide the sidebar")
                            .clicked()
                        {
                            let mut panels = self.side_panels();
                            for p in panels.iter_mut() {
                                let d = p.dock_mut();
                                if !d.detached {
                                    d.show_dialog = false;
                                }
                            }
                        }
                        // Pop out is only relevant if any panel is attached
                        let any_attached = {
                            let mut panels = self.side_panels();
                            panels.iter_mut().any(|p| {
                                let d = p.dock_mut();
                                !d.detached && d.show_dialog
                            })
                        };
                        if any_attached {
                            if ui
                                .button("Pop out")
                                .on_hover_text("Open attached panel in a floating window")
                                .clicked()
                            {
                                // Convert current attached+visible panel(s) to detached dialog(s)
                                let mut panels = self.side_panels();
                                for p in panels.iter_mut() {
                                    let d = p.dock_mut();
                                    if !d.detached && d.show_dialog {
                                        d.detached = true;
                                        d.show_dialog = true;
                                    }
                                }
                            }
                        }
                    });
                    // Tab selection: focus chosen panel in sidebar and hide other attached panels
                    if let Some(i) = clicked_idx {
                        let mut panels = self.side_panels();
                        for (j, p) in panels.iter_mut().enumerate() {
                            let d = p.dock_mut();
                            if j == i {
                                d.detached = false;
                                d.show_dialog = true;
                            } else if !d.detached {
                                d.show_dialog = false;
                            }
                        }
                    }
                });
                ui.separator();
                // Show the currently attached panel's contents (first non-detached) safely
                // by temporarily taking the concrete panel out of `self` to avoid aliasing.
                let active_idx = {
                    let mut panels = self.side_panels();
                    panels.iter_mut().position(|p| {
                        let d = p.dock_mut();
                        !d.detached && d.show_dialog
                    })
                };
                if let Some(i) = active_idx {
                    match i {
                        0 => {
                            let mut panel = std::mem::take(&mut self.traces_panel);
                            panel.panel_contents(self, ui);
                            self.traces_panel = panel;
                        }
                        1 => {
                            let mut panel = std::mem::take(&mut self.math_panel);
                            panel.panel_contents(self, ui);
                            self.math_panel = panel;
                        }
                        2 => {
                            let mut panel = std::mem::take(&mut self.thresholds_panel);
                            panel.panel_contents(self, ui);
                            self.thresholds_panel = panel;
                        }
                        _ => {}
                    }
                }
            });
    }

    /// Render export buttons (Save PNG screenshot and Save raw data CSV/Parquet) into the given Ui.
    fn render_export_buttons(&mut self, ui: &mut egui::Ui) {
        if ui
            .button("Save PNG")
            .on_hover_text("Take an egui viewport screenshot")
            .clicked()
        {
            self.request_window_shot = true;
        }
        ui.menu_button("Export", |ui| {
            let hover_text_traces: &str = {
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
                .button("Traces…")
                .on_hover_text(hover_text_traces)
                .clicked()
            {
                ui.close();
                self.prompt_and_save_raw_data();
            }

            if ui
                .button("Threshold events…")
                .on_hover_text("Export filtered or all threshold events as CSV")
                .clicked()
            {
                ui.close();
                self.prompt_and_save_threshold_events();
            }
        });
    }

    /// Show a file dialog and save raw data in the chosen format.
    fn prompt_and_save_raw_data(&mut self) {
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
                    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
                        "parquet" => RawExportFormat::Parquet,
                        _ => RawExportFormat::Csv,
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

    /// Show a file dialog and export threshold events to CSV (respects current events filter if set).
    fn prompt_and_save_threshold_events(&mut self) {
        // Collect filtered events (newest first as shown in the UI)
        let evts: Vec<&ThresholdEvent> = self
            .threshold_event_log
            .iter()
            .rev()
            .filter(|e| {
                self.thresholds_panel
                    .events_filter
                    .as_ref()
                    .map_or(true, |f| &e.threshold == f)
            })
            .collect();
        if evts.is_empty() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("threshold_events.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        {
            if let Err(e) = super::export_helpers::save_threshold_events_csv(&path, &evts) {
                eprintln!("Failed to export events CSV: {e}");
            }
        }
    }

    /// Handle pending screenshot request and save the resulting image to a chosen path or env path.
    fn handle_screenshot_result(&mut self, ctx: &egui::Context) {
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

    /// Handle focus requests coming from detached panels (Dock buttons) and hide other attached panels.
    fn process_focus_requests(&mut self) {
        let mut focus_idx: Option<usize> = None;
        {
            let mut panels = self.side_panels();
            for (i, p) in panels.iter_mut().enumerate() {
                if p.dock_mut().focus_dock {
                    focus_idx = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = focus_idx {
            let mut panels = self.side_panels();
            for (j, p) in panels.iter_mut().enumerate() {
                let d = p.dock_mut();
                if j == i {
                    d.focus_dock = false;
                    d.detached = false;
                    d.show_dialog = true;
                } else if !d.detached {
                    d.show_dialog = false;
                }
            }
        }
    }

    // show_right_sidebar_panel is now merged into render_right_sidebar_panel

    /// Render the central plot inside the default central panel and apply interactions.
    fn render_central_plot_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let plot_response = self.plot_traces_common(ui, ctx, "scope_plot_multi");
            //self.sync_time_window_with_plot(&plot_response);
            self.pause_on_click(&plot_response);
            self.apply_zoom(&plot_response);
            self.handle_plot_click(&plot_response);
        });
    }

    /// Apply any pending UI action controller requests (pause/resume/screenshot/raw save, FFT data).
    fn handle_ui_action_requests(&mut self) {
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
    }

    /// Publish current window info and apply any pending viewport requests from the window controller.
    fn handle_window_controller_requests(&mut self, ctx: &egui::Context) {
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
    }

    #[inline]
    fn repaint_tick(ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn side_panels(&mut self) -> Vec<&mut dyn DockPanel> {
        vec![
            &mut self.traces_panel,
            &mut self.math_panel,
            &mut self.thresholds_panel,
        ]
    }

    /// Drain incoming samples and append to per-trace buffers. Create traces on first sighting.
    fn drain_rx_and_update_traces(&mut self) {
        while let Ok(s) = self.rx.try_recv() {
            let is_new = !self.traces.contains_key(&s.trace);
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                let mut look = TraceLook::default();
                look.color = Self::alloc_color(idx);
                TraceState {
                    name: s.trace.clone(),
                    look,
                    offset: 0.0,
                    live: VecDeque::new(),
                    snap: None,
                    last_fft: None,
                    is_math: false,
                    info: String::new(),
                }
            });
            if is_new && self.selection_trace.is_none() {
                self.selection_trace = Some(s.trace.clone());
            }
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            // Set/refresh info if provided by producer
            if let Some(info) = s.info.as_ref() {
                entry.info = info.clone();
            }
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
                        tr.look.color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    }
                }
                for (name, vis) in inner.visible_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.look.visible = vis;
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
                        color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                        visible: tr.look.visible,
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
        // Handle each panel individually to avoid simultaneous mutable borrows of self
        {
            let mut panel = std::mem::take(&mut self.traces_panel);
            {
                let d = panel.dock_mut();
                if d.detached && d.show_dialog {
                    panel.show_detached_dialog(self, ctx);
                }
            }
            self.traces_panel = panel;
        }
        {
            let mut panel = std::mem::take(&mut self.math_panel);
            {
                let d = panel.dock_mut();
                if d.detached && d.show_dialog {
                    panel.show_detached_dialog(self, ctx);
                }
            }
            self.math_panel = panel;
        }
        {
            let mut panel = std::mem::take(&mut self.thresholds_panel);
            {
                let d = panel.dock_mut();
                if d.detached && d.show_dialog {
                    panel.show_detached_dialog(self, ctx);
                }
            }
            self.thresholds_panel = panel;
        }
        #[cfg(feature = "fft")]
        {
            let mut panel = std::mem::take(&mut self.fft_panel);
            {
                let d = panel.dock_mut();
                if d.detached && d.show_dialog {
                    panel.show_detached_dialog(self, ctx);
                }
            }
            self.fft_panel = panel;
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
        if t_latest_overall.is_finite() {
            Some(t_latest_overall)
        } else {
            None
        }
    }

    /// Shared plot for both embedded and main variants. Returns (x_width, zoomed) and full response.
    fn plot_traces_common(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        plot_id: &str,
    ) -> egui_plot::PlotResponse<bool> {
        let mut plot = Plot::new(plot_id)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(true)
            .x_axis_formatter(|x, _range| {
                let val = x.value;
                let secs = val as i64;
                let nsecs = ((val - secs as f64) * 1e9) as u32;
                let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
            })
            .y_axis_formatter(|y, _range| {
                let v = y.value;
                let step = y.step_size;
                let label_val = if self.y_log { 10f64.powf(v) } else { v };
                if let Some(unit) = &self.y_unit {
                    if step.abs() < 0.001 {
                        let exponent = step.log10().floor() + 1.0;
                        format!(
                            "{:.1}e{} {}",
                            label_val / 10f64.powf(exponent),
                            exponent,
                            unit
                        )
                    } else {
                        format!("{:.3} {}", label_val, unit)
                    }
                } else {
                    if step.abs() < 0.001 {
                        let exponent = step.log10().floor() + 1.0;
                        format!("{:.1}e{}", label_val / 10f64.powf(exponent), exponent)
                    } else {
                        format!("{:.3}", label_val)
                    }
                }
            });
        // Determine desired x-bounds for follow
        let t_latest = self.latest_time_overall().unwrap_or(0.0);

        if self.show_legend {
            plot = plot.legend(Legend::default());
        }
        let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
        let marker_font_size = base_body * 1.5;
        let plot_resp = plot.show(ui, |plot_ui| {
            // Handle zooming/panning/auto-zooming
            let resp = plot_ui.response();

            let is_zooming_rect = resp.drag_stopped_by(egui::PointerButton::Secondary);
            let is_panning =
                resp.dragged_by(egui::PointerButton::Primary) && resp.is_pointer_button_down_on();

            let scroll_data = resp.ctx.input(|i| i.raw_scroll_delta);
            let is_zooming_with_wheel =
                (scroll_data.x != 0.0 || scroll_data.y != 0.0) && resp.hovered();

            let bounds_changed =
                is_zooming_rect || is_panning || is_zooming_with_wheel || self.pending_auto_x;

            if is_zooming_with_wheel {
                let mut zoom_factor = egui::Vec2::new(1.0, 1.0);
                if scroll_data.y != 0.0
                    && (self.zoom_mode == ZoomMode::X || self.zoom_mode == ZoomMode::Both)
                {
                    zoom_factor.x = 1.0 + scroll_data.y * 0.001;
                } else if scroll_data.x != 0.0 {
                    zoom_factor.x = 1.0 - scroll_data.x * 0.001;
                }
                if self.zoom_mode == ZoomMode::Y || self.zoom_mode == ZoomMode::Both {
                    zoom_factor.y = 1.0 + scroll_data.y * 0.001;
                }

                if !self.paused {
                    plot_ui.set_plot_bounds_x(
                        t_latest - self.time_window * (2.0 - (zoom_factor.x as f64))..=t_latest,
                    );
                    zoom_factor.x = 1.0;
                }
                plot_ui.zoom_bounds_around_hovered(zoom_factor);
            } else if self.pending_auto_x {
                let mut xmin = f64::INFINITY;
                let mut xmax = f64::NEG_INFINITY;

                for tr in self.traces.values() {
                    if !tr.look.visible {
                        continue;
                    }

                    if self.paused {
                        if let Some(snap) = &tr.snap {
                            if let (Some(&[t_first, _]), Some(&[t_last, _])) =
                                (snap.front(), snap.back())
                            {
                                if t_first < xmin {
                                    xmin = t_first;
                                }
                                if t_last > xmax {
                                    xmax = t_last;
                                }
                            }
                        }
                    } else if let (Some(&[t_first, _]), Some(&[t_last, _])) =
                        (tr.live.front(), tr.live.back())
                    {
                        if t_first < xmin {
                            xmin = t_first;
                        }
                        if t_last > xmax {
                            xmax = t_last;
                        }
                    }
                }

                if xmin.is_finite() && xmax.is_finite() && xmin < xmax {
                    if !self.paused {
                        plot_ui.set_plot_bounds_x(t_latest - (xmax - xmin)..=t_latest);
                    } else {
                        plot_ui.set_plot_bounds_x(xmin..=xmax);
                    }
                }
                self.pending_auto_x = false;
            } else {
                if self.y_min.is_finite() && self.y_max.is_finite() && self.y_min < self.y_max {
                    let space = (self.y_max - self.y_min) * 0.05;
                    plot_ui.set_plot_bounds_y(self.y_min - space..=self.y_max + space);
                }
                if !self.paused {
                    plot_ui.set_plot_bounds_x(t_latest - self.time_window..=t_latest);
                } else {
                    let act_bounds = plot_ui.plot_bounds();
                    let xmax = act_bounds.range_x().end()
                        - (act_bounds.range_x().end()
                            - act_bounds.range_x().start()
                            - self.time_window)
                            / 2.0;
                    let xmin = xmax - self.time_window;
                    plot_ui.set_plot_bounds_x(xmin..=xmax);
                }
            }

            // Lines
            for name in self.trace_order.clone().into_iter() {
                if let Some(tr) = self.traces.get(&name) {
                    if !tr.look.visible {
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
                    let pts_vec: Vec<[f64; 2]> = iter
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
                    let mut color = tr.look.color;
                    let mut width: f32 = tr.look.width.max(0.1);
                    let style = tr.look.style;
                    if let Some(hov) = &self.hover_trace {
                        if &tr.name != hov {
                            // Strongly dim non-hovered traces
                            color = Color32::from_rgba_unmultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                40,
                            );
                        } else {
                            // Emphasize hovered trace
                            width = (width * 1.6).max(width + 1.0);
                        }
                    }
                    let mut line = Line::new(&tr.name, pts_vec.clone())
                        .color(color)
                        .width(width)
                        .style(style);
                    let legend_label = if self.show_info_in_legend && !tr.info.is_empty() {
                        format!("{} — {}", tr.name, tr.info)
                    } else {
                        tr.name.clone()
                    };
                    line = line.name(legend_label);
                    plot_ui.line(line);

                    // Optional point markers for each datapoint
                    if tr.look.show_points {
                        if !pts_vec.is_empty() {
                            let mut radius = tr.look.point_size.max(0.5);
                            if let Some(hov) = &self.hover_trace {
                                if &tr.name == hov {
                                    radius = (radius * 1.25).max(radius + 0.5);
                                }
                            }
                            let points = Points::new("", pts_vec.clone())
                                .radius(radius)
                                .shape(tr.look.marker)
                                .color(color);
                            plot_ui.points(points);
                        }
                    }
                }
            }

            // Threshold overlays: draw horizontal line(s) for each threshold on its target trace
            if !self.threshold_defs.is_empty() {
                let bounds = plot_ui.plot_bounds();
                let xr = bounds.range_x();
                let xmin = *xr.start();
                let xmax = *xr.end();
                // VLine/HLine draw across full axis; explicit y range not needed
                for def in &self.threshold_defs {
                    // Only draw if the target trace exists and is visible
                    if let Some(tr) = self.traces.get(&def.target.0) {
                        if !tr.look.visible {
                            continue;
                        }
                        // Effective looks for threshold line and events
                        let thr_look = self
                            .thresholds_panel
                            .looks
                            .get(&def.name)
                            .cloned()
                            .unwrap_or_else(|| {
                                let mut l = TraceLook::default();
                                if let Some(rgb) = def.color_hint {
                                    l.color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                } else {
                                    l.color = tr.look.color;
                                }
                                l.width = 1.5;
                                l
                            });
                        let ev_start_look = self
                            .thresholds_panel
                            .start_looks
                            .get(&def.name)
                            .cloned()
                            .unwrap_or_else(|| {
                                let mut l = TraceLook::default();
                                l.color = thr_look.color;
                                l.width = 2.0;
                                l
                            });
                        let ev_stop_look = self
                            .thresholds_panel
                            .stop_looks
                            .get(&def.name)
                            .cloned()
                            .unwrap_or_else(|| {
                                let mut l = TraceLook::default();
                                l.color = thr_look.color;
                                l.width = 2.0;
                                l
                            });
                        // Use provided colors (respecting any alpha set by the UI)
                        let mut thr_color = thr_look.color;
                        let mut thr_width = thr_look.width.max(0.1);
                        if let Some(hov_thr) = &self.hover_threshold {
                            if &def.name != hov_thr {
                                // Dim non-hovered thresholds
                                thr_color = Color32::from_rgba_unmultiplied(
                                    thr_color.r(),
                                    thr_color.g(),
                                    thr_color.b(),
                                    60,
                                );
                            } else {
                                // Emphasize hovered threshold
                                thr_width = (thr_width * 1.6).max(thr_width + 1.0);
                            }
                        }
                        // Event markers follow the same dimming when another threshold is hovered
                        let ev_base = thr_look.color;
                        let ev_color = if let Some(hov_thr) = &self.hover_threshold {
                            if &def.name != hov_thr {
                                Color32::from_rgba_unmultiplied(
                                    ev_base.r(),
                                    ev_base.g(),
                                    ev_base.b(),
                                    60,
                                )
                            } else {
                                ev_base
                            }
                        } else {
                            ev_base
                        };

                        // Helper: render one horizontal threshold line with a unique id and an optional legend label
                        let mut draw_hline = |id: &str, label: Option<String>, y_world: f64| {
                            // Apply per-trace offset and global y-log transform to map to plot space
                            let y_lin = y_world + tr.offset;
                            let y_plot = if self.y_log {
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                y_lin
                            };
                            if y_plot.is_finite() {
                                let mut h = HLine::new(id.to_string(), y_plot)
                                    .color(thr_color)
                                    .width(thr_width)
                                    .style(thr_look.style);
                                if let Some(lbl) = &label {
                                    h = h.name(lbl.clone());
                                } else {
                                    h = h.name("");
                                }
                                plot_ui.hline(h);
                            }
                        };

                        // Compose compact condition expression for legend/info
                        let expr = match &def.kind {
                            crate::thresholds::ThresholdKind::GreaterThan { value } => {
                                if let Some(u) = &self.y_unit {
                                    format!("> {:.3} {}", value, u)
                                } else {
                                    format!("> {:.3}", value)
                                }
                            }
                            crate::thresholds::ThresholdKind::LessThan { value } => {
                                if let Some(u) = &self.y_unit {
                                    format!("< {:.3} {}", value, u)
                                } else {
                                    format!("< {:.3}", value)
                                }
                            }
                            crate::thresholds::ThresholdKind::InRange { low, high } => {
                                if let Some(u) = &self.y_unit {
                                    format!("[{:.3}, {:.3}] {}", low, high, u)
                                } else {
                                    format!("[{:.3}, {:.3}]", low, high)
                                }
                            }
                        };
                        // Legend label: like math traces -- base is name, optionally append info
                        let thr_info = format!("{} {}", def.target.0, expr);
                        let legend_label = if self.show_info_in_legend {
                            format!("{} — {}", def.name, thr_info)
                        } else {
                            def.name.clone()
                        };

                        match def.kind {
                            crate::thresholds::ThresholdKind::GreaterThan { value } => {
                                let id = format!("thr:{}", def.name);
                                draw_hline(&id, Some(legend_label), value);
                            }
                            crate::thresholds::ThresholdKind::LessThan { value } => {
                                let id = format!("thr:{}", def.name);
                                draw_hline(&id, Some(legend_label), value);
                            }
                            crate::thresholds::ThresholdKind::InRange { low, high } => {
                                // Draw both bounds; only one legend entry (low)
                                let id_low = format!("thr:{}:low", def.name);
                                let id_high = format!("thr:{}:high", def.name);
                                draw_hline(&id_low, Some(legend_label), low);
                                draw_hline(&id_high, None, high);
                            }
                        }

                        // Draw event markers for this threshold: vertical lines or points at start/end
                        if let Some(state) = self.threshold_states.get(&def.name) {
                            if ev_start_look.show_points || ev_stop_look.show_points {
                                // Determine a representative Y for the marker (threshold value or midpoint for ranges) and map to plot space
                                let marker_y_world = match def.kind {
                                    crate::thresholds::ThresholdKind::GreaterThan { value } => {
                                        value
                                    }
                                    crate::thresholds::ThresholdKind::LessThan { value } => value,
                                    crate::thresholds::ThresholdKind::InRange { low, high } => {
                                        (low + high) * 0.5
                                    }
                                };
                                let y_lin = marker_y_world + tr.offset;
                                let marker_y_plot = if self.y_log {
                                    if y_lin > 0.0 {
                                        y_lin.log10()
                                    } else {
                                        f64::NAN
                                    }
                                } else {
                                    y_lin
                                };
                                if marker_y_plot.is_finite() {
                                    for ev in state.events.iter() {
                                        if ev.end_t < xmin || ev.start_t > xmax {
                                            continue;
                                        }
                                        if ev_start_look.show_points {
                                            let p =
                                                Points::new("", vec![[ev.start_t, marker_y_plot]])
                                                    .radius(ev_start_look.point_size.max(0.5))
                                                    .shape(ev_start_look.marker)
                                                    .color(ev_color);
                                            plot_ui.points(p);
                                        } else {
                                            let s = VLine::new("", ev.start_t)
                                                .color(ev_color)
                                                .width(ev_start_look.width.max(0.1))
                                                .style(ev_start_look.style)
                                                .name("");
                                            plot_ui.vline(s);
                                        }
                                        if ev_stop_look.show_points {
                                            let p =
                                                Points::new("", vec![[ev.end_t, marker_y_plot]])
                                                    .radius(ev_stop_look.point_size.max(0.5))
                                                    .shape(ev_stop_look.marker)
                                                    .color(ev_color);
                                            plot_ui.points(p);
                                        } else {
                                            let e = VLine::new("", ev.end_t)
                                                .color(ev_color)
                                                .width(ev_stop_look.width.max(0.1))
                                                .style(ev_stop_look.style)
                                                .name("");
                                            plot_ui.vline(e);
                                        }
                                    }
                                    if state.active {
                                        let start_t = state.start_t;
                                        let end_t = state.last_t.unwrap_or(start_t);
                                        if !(end_t < xmin || start_t > xmax) {
                                            if ev_start_look.show_points {
                                                let p =
                                                    Points::new("", vec![[start_t, marker_y_plot]])
                                                        .radius(ev_start_look.point_size.max(0.5))
                                                        .shape(ev_start_look.marker)
                                                        .color(ev_color);
                                                plot_ui.points(p);
                                            } else {
                                                let s = VLine::new("", start_t)
                                                    .color(ev_color)
                                                    .width(ev_start_look.width.max(0.1))
                                                    .style(ev_start_look.style)
                                                    .name("");
                                                plot_ui.vline(s);
                                            }
                                            if ev_stop_look.show_points {
                                                let p =
                                                    Points::new("", vec![[end_t, marker_y_plot]])
                                                        .radius(ev_stop_look.point_size.max(0.5))
                                                        .shape(ev_stop_look.marker)
                                                        .color(ev_color);
                                                plot_ui.points(p);
                                            } else {
                                                let e = VLine::new("", end_t)
                                                    .color(ev_color)
                                                    .width(ev_stop_look.width.max(0.1))
                                                    .style(ev_stop_look.style)
                                                    .name("");
                                                plot_ui.vline(e);
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Draw as lines with potentially different styles for start/stop
                                for ev in state.events.iter() {
                                    if ev.end_t < xmin || ev.start_t > xmax {
                                        continue;
                                    }
                                    let ls = VLine::new("", ev.start_t)
                                        .color(ev_color)
                                        .width(ev_start_look.width.max(0.1))
                                        .style(ev_start_look.style)
                                        .name("");
                                    plot_ui.vline(ls);
                                    let le = VLine::new("", ev.end_t)
                                        .color(ev_color)
                                        .width(ev_stop_look.width.max(0.1))
                                        .style(ev_stop_look.style)
                                        .name("");
                                    plot_ui.vline(le);
                                }
                                if state.active {
                                    let start_t = state.start_t;
                                    let end_t = state.last_t.unwrap_or(start_t);
                                    if !(end_t < xmin || start_t > xmax) {
                                        let s = VLine::new("", start_t)
                                            .color(ev_color)
                                            .width(ev_start_look.width.max(0.1))
                                            .style(ev_start_look.style)
                                            .name("");
                                        plot_ui.vline(s);
                                        let e = VLine::new("", end_t)
                                            .color(ev_color)
                                            .width(ev_stop_look.width.max(0.1))
                                            .style(ev_stop_look.style)
                                            .name("");
                                        plot_ui.vline(e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Shared selection overlays with smart label placement to avoid overlap
            let p1_opt = self.point_selection.selected_p1;
            let p2_opt = self.point_selection.selected_p2;

            let ox = 0.01 * self.time_window; // horizontal offset for labels
            let oy = 0.01 * (self.y_max - self.y_min); // vertical offset for labels

            //if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
            let (dx, dy) = if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                (p2[0] - p1[0], p2[1] - p1[1])
            } else {
                (0.0, 0.0)
            };

            // Inline helper closure to compute anchor, text alignment and base point
            let label_pos = |dx: f64,
                             dy: f64,
                             p: &[f64; 2],
                             ox: f64,
                             oy: f64|
             -> (Align2, egui::Align, PlotPoint) {
                let slope = if dx != 0.0 || oy != 0.0 || ox != 0.0 {
                    (dy / oy) / (dx / ox)
                } else {
                    0.0
                };
                if dx <= 0.0 || slope.abs() > 8.0 {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (
                            Align2::LEFT_TOP,
                            egui::Align::LEFT,
                            PlotPoint::new(p[0] + ox, p[1] - oy),
                        )
                    } else {
                        (
                            Align2::LEFT_BOTTOM,
                            egui::Align::LEFT,
                            PlotPoint::new(p[0] + ox, p[1] + oy),
                        )
                    }
                } else {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (
                            Align2::RIGHT_TOP,
                            egui::Align::RIGHT,
                            PlotPoint::new(p[0] - ox, p[1] - oy),
                        )
                    } else {
                        (
                            Align2::RIGHT_BOTTOM,
                            egui::Align::RIGHT,
                            PlotPoint::new(p[0] - ox, p[1] + oy),
                        )
                    }
                }
            };

            // Always draw the point markers at exact locations
            if let Some(p) = p1_opt {
                plot_ui.points(
                    Points::new("Measurement", vec![p])
                        .radius(5.0)
                        .color(Color32::YELLOW),
                );

                let (halign_anchor, text_align, base) = label_pos(dx, dy, &p, ox, oy);

                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit {
                    format!("{:.6} {}", y_lin, u)
                } else {
                    format!("{:.6}", y_lin)
                };
                let txt = format!(
                    "P1\nx = {}\ny = {}",
                    self.x_date_format.format_value(p[0]),
                    ytxt
                );

                // Build multi-line layout with alignment handled by job.halign
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();

                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::YELLOW)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let Some(p) = p2_opt {
                plot_ui.points(
                    Points::new("Measurement", vec![p])
                        .radius(5.0)
                        .color(Color32::LIGHT_BLUE),
                );

                let (halign_anchor, text_align, base) = label_pos(-dx, -dy, &p, ox, oy);

                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit {
                    format!("{:.6} {}", y_lin, u)
                } else {
                    format!("{:.6}", y_lin)
                };
                let txt = format!(
                    "P2\nx = {}\ny = {}",
                    self.x_date_format.format_value(p[0]),
                    ytxt
                );

                // Build multi-line layout with alignment handled by job.halign
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();

                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_BLUE)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                plot_ui.line(Line::new("Measurement", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                let dx = p2[0] - p1[0];
                let y1 = if self.y_log { 10f64.powf(p1[1]) } else { p1[1] };
                let y2 = if self.y_log { 10f64.powf(p2[1]) } else { p2[1] };
                let dy_lin = y2 - y1;
                let slope = if dx.abs() > 1e-12 {
                    dy_lin / dx
                } else {
                    f64::INFINITY
                };
                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                let dy_txt = if let Some(u) = &self.y_unit {
                    format!("{:.6} {}", dy_lin, u)
                } else {
                    format!("{:.6}", dy_lin)
                };
                let txt = if slope.is_finite() {
                    format!("Δx={:.6}\nΔy={}\nslope={:.4}", dx, dy_txt, slope)
                } else {
                    format!("Δx=0\nΔy={}\nslope=∞", dy_txt)
                };

                let slope_plot = if dx != 0.0 || oy != 0.0 || ox != 0.0 {
                    (dy / oy) / (dx / ox)
                } else {
                    0.0
                };
                let (halign_anchor, base) = if slope_plot.abs() > 8.0 {
                    (Align2::RIGHT_CENTER, PlotPoint::new(mid[0] - ox, mid[1]))
                } else if slope_plot.abs() < 0.2 {
                    (Align2::CENTER_BOTTOM, PlotPoint::new(mid[0], mid[1] + oy))
                } else if slope_plot >= 0.0 {
                    (Align2::LEFT_TOP, PlotPoint::new(mid[0] + ox, mid[1] - oy))
                } else {
                    (
                        Align2::LEFT_BOTTOM,
                        PlotPoint::new(mid[0] + ox, mid[1] + oy),
                    )
                };

                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();

                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_GREEN)
                    .append_to(
                        &mut job,
                        &style,
                        egui::FontSelection::Default,
                        egui::Align::LEFT,
                    );
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }

            // // Compute bounds-based offsets in plot units
            // let act_bounds = plot_ui.plot_bounds();
            // let xr = act_bounds.range_x();
            // let yr = act_bounds.range_y();
            // let xw = (*xr.end() - *xr.start()).abs().max(1e-12);
            // let yw = (*yr.end() - *yr.start()).abs().max(1e-12);
            // let base_dx = 0.012_f64 * xw; // horizontal component for base radius computation
            // let base_dy = 0.020_f64 * yw; // vertical component for base radius computation
            // // Use a single radial distance so the anchor corner sits on a circle around the point.
            // let base_r = (base_dx * base_dx + base_dy * base_dy).sqrt();

            // // (text anchor handled per-line with LEFT/RIGHT_CENTER)

            // // Draw P1 label with offset away from P2 if available
            // if let Some(p1) = p1_opt {
            //     let (ox, oy) = if let Some(p2) = p2_opt {
            //         let vx = p2[0] - p1[0];
            //         let vy = p2[1] - p1[1];
            //         let sx = if vx >= 0.0 { -1.0 } else { 1.0 }; // push away from P2
            //         let sy = if vy >= 0.0 { -1.0 } else { 1.0 };
            //         // Amplify radius if extremely close to keep text readable
            //         let amp = if vx.abs() < 0.02 * xw && vy.abs() < 0.02 * yw { 1.8 } else { 1.0 };
            //         // Radial diagonal vector with constant length: components = R / sqrt(2)
            //         let comp = (base_r * amp) / std::f64::consts::SQRT_2;
            //         (sx * comp, sy * comp)
            //     } else {
            //         // Default up-left diagonal
            //         let comp = base_r / std::f64::consts::SQRT_2;
            //         (-comp, -comp)
            //     };
            //     let y_lin = if self.y_log { 10f64.powf(p1[1]) } else { p1[1] };
            //     let ytxt = if let Some(u) = &self.y_unit {
            //         format!("{:.6} {}", y_lin, u)
            //     } else {
            //         format!("{:.6}", y_lin)
            //     };
            //     // Prepare per-line strings below
            //     // Multiline alignment by drawing each line with a left/right anchor
            //     let base = PlotPoint::new(p1[0] + ox, p1[1] + oy);
            //     let halign_anchor = if ox >= 0.0 {
            //         if oy >= 0.0 { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP }
            //     } else {
            //         if oy >= 0.0 { Align2::RIGHT_BOTTOM } else { Align2::RIGHT_TOP }
            //     };
            //     // Build multi-line layout with alignment handled by job.halign
            //     let mut job = egui::text::LayoutJob::default();
            //     job.halign = if ox >= 0.0 { egui::Align::LEFT } else { egui::Align::RIGHT };
            //     let font_id = egui::FontId::proportional(marker_font_size);
            //     let fmt = egui::TextFormat { font_id: font_id.clone(), color: Color32::YELLOW, ..Default::default() };
            //     for (i, line) in [
            //         "P1".to_string(),
            //         format!("x={}", self.x_date_format.format_value(p1[0])),
            //         format!("y={}", ytxt),
            //     ].into_iter().enumerate() {
            //         let mut text = line;
            //         if i < 2 { text.push('\n'); }
            //         job.append(&text, 0.0, fmt.clone());
            //     }
            //     plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            //     } else {
            //         let comp = base_r / std::f64::consts::SQRT_2;
            //         (comp, comp)
            //     };
            //     let y_lin = if self.y_log { 10f64.powf(p2[1]) } else { p2[1] };
            //     let ytxt = if let Some(u) = &self.y_unit {
            //         format!("{:.6} {}", y_lin, u)
            //     } else {
            //         format!("{:.6}", y_lin)
            //     };
            //     // Prepare per-line strings below
            //     let base = PlotPoint::new(p2[0] + ox, p2[1] + oy);
            //     let halign_anchor = if ox >= 0.0 {
            //         if oy >= 0.0 { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP }
            //     } else {
            //         if oy >= 0.0 { Align2::RIGHT_BOTTOM } else { Align2::RIGHT_TOP }
            //     };
            //     let mut job = egui::text::LayoutJob::default();
            //     job.halign = if ox >= 0.0 { egui::Align::LEFT } else { egui::Align::RIGHT };
            //     let font_id = egui::FontId::proportional(marker_font_size);
            //     let fmt = egui::TextFormat { font_id: font_id.clone(), color: Color32::LIGHT_BLUE, ..Default::default() };
            //     for (i, line) in [
            //         "P2".to_string(),
            //         format!("x={}", self.x_date_format.format_value(p2[0])),
            //         format!("y={}", ytxt),
            //     ].into_iter().enumerate() {
            //         let mut text = line;
            //         if i < 2 { text.push('\n'); }
            //         job.append(&text, 0.0, fmt.clone());
            // Removed hardcoded Math/Thresholds buttons; handled by registry above
            //         format!("Δx={:.6}\nΔy={}\nslope={:.4}", dx, dy_txt, slope)
            //     } else {
            //         format!("Δx=0\nΔy={}\nslope=∞", dy_txt)
            //     };
            //     // Perpendicular offset vector (scaled separately in x/y to match axes)
            //     let vx = p2[0] - p1[0];
            //     let vy = p2[1] - p1[1];
            //     let nx = -vy;
            //     let ny = vx;
            //     let nlen = (nx * nx + ny * ny).sqrt().max(1e-12);
            //     // Use constant radial distance for delta label as well (scaled by 1.5)
            //     let radial = base_r * 1.5;
            //     let off_x = (nx / nlen) * radial;
            //     let off_y = (ny / nlen) * radial;
            //     let base = PlotPoint::new(mid[0] + off_x, mid[1] + off_y);
            //     let halign_anchor = if off_x >= 0.0 {
            //         if off_y >= 0.0 { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP }
            //     } else {
            //         if off_y >= 0.0 { Align2::RIGHT_BOTTOM } else { Align2::RIGHT_TOP }
            //     };
            //     let mut job = egui::text::LayoutJob::default();
            //     job.halign = if off_x >= 0.0 { egui::Align::LEFT } else { egui::Align::RIGHT };
            //     let font_id = egui::FontId::proportional(marker_font_size);
            //     let fmt = egui::TextFormat { font_id: font_id.clone(), color: Color32::LIGHT_GREEN, ..Default::default() };
            //     let parts: Vec<&str> = overlay.split('\n').collect();
            //     for (i, part) in parts.iter().enumerate() {
            //         let mut text = part.to_string();
            //         if i + 1 != parts.len() { text.push('\n'); }
            //         job.append(&text, 0.0, fmt.clone());
            //     }
            //     plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            // }

            bounds_changed
        });
        // After plot (outside closure) we can safely render registry-driven buttons if needed
        // (No-op here: buttons are already rendered in controls_ui)
        plot_resp
    }

    fn pause_on_click(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.response.clicked()
            || plot_response
                .response
                .dragged_by(egui::PointerButton::Secondary)
        {
            if !self.paused {
                self.paused = true;
                for tr in self.traces.values_mut() {
                    tr.snap = Some(tr.live.clone());
                }
            }
        }
    }

    // Update zoom and pan state from plot response
    fn apply_zoom(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.inner {
            let bounds = plot_response.transform.bounds();

            let w = {
                let r = bounds.range_x();
                let (a, b) = (*r.start(), *r.end());
                (b - a).abs()
            };
            if w.is_finite()
                && w > 0.0
                && (w - self.time_window).abs() / self.time_window.max(1e-6) > 0.02
            {
                self.time_window = w;
            }

            let r = bounds.range_y();
            let ymin = *r.start();
            let ymax = *r.end();
            if ymin.is_finite() && ymax.is_finite() && ymin < ymax {
                let space = (0.05 / 1.1) * (ymax - ymin);
                self.y_min = ymin + space;
                self.y_max = ymax - space;
            }
        } else if self.pending_auto_y {
            let act_bounds = plot_response.transform.bounds();
            let mut ymin = f64::INFINITY;
            let mut ymax = f64::NEG_INFINITY;
            // Limit to currently visible X-range
            let rx = act_bounds.range_x();
            let (xmin, xmax) = (*rx.start(), *rx.end());
            for tr in self.traces.values() {
                if !tr.look.visible {
                    continue;
                }
                // Use snapshot when paused, else live
                let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                    if let Some(snap) = &tr.snap {
                        Box::new(snap.iter())
                    } else {
                        Box::new(tr.live.iter())
                    }
                } else {
                    Box::new(tr.live.iter())
                };
                for p in iter {
                    let x = p[0];
                    if !(x >= xmin && x <= xmax) {
                        continue;
                    }
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

            self.y_min = ymin;
            self.y_max = ymax;
            self.pending_auto_y = false;
        }
    }

    /// Handle click selection on the plot using nearest point logic.
    fn handle_plot_click(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.response.clicked() {
            if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                let transform = plot_response.transform;
                let plot_pos = transform.value_from_position(screen_pos);
                let selected_trace_name = self.selection_trace.clone();
                let sel_data_points: Option<Vec<[f64; 2]>> =
                    if let Some(name) = &selected_trace_name {
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
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    continue;
                                }
                            } else {
                                y_lin
                            };
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
                        self.point_selection
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

            ui.label("X-Axis Time:");
            let mut tw = self.time_window;
            if !self.time_slider_dragging {
                if tw <= self.time_window_min {
                    self.time_window_min = self.time_window_min / 10.0;
                    self.time_window_max = self.time_window_max / 10.0;
                } else if tw >= self.time_window_max {
                    self.time_window_min = self.time_window_min * 10.0;
                    self.time_window_max = self.time_window_max * 10.0;
                }
            }
            let slider = egui::Slider::new(&mut tw, self.time_window_min..=self.time_window_max)
                .logarithmic(true)
                .smart_aim(true)
                .show_value(true)
                .clamping(egui::SliderClamping::Edits)
                .suffix(" s");
            let sresp = ui.add(slider);
            if sresp.changed() {
                self.time_window = tw; //.max(1e-6);
            }
            // Expand bounds only on release
            self.time_slider_dragging = sresp.is_pointer_button_down_on();

            // Points cap
            ui.label("Points:");
            ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));

            if ui
                .button("Fit")
                .on_hover_text("Fit the X-axis to the visible data")
                .clicked()
            {
                // Clear manual bounds and request one-shot auto fit
                self.pending_auto_x = true;
            }

            ui.separator();

            // Y controls (shared)
            let mut y_min_tmp = self.y_min;
            let mut y_max_tmp = self.y_max;
            let y_range = y_max_tmp - y_min_tmp;

            ui.label("Y-Axis Min:");
            let r1 = ui.add(
                egui::DragValue::new(&mut y_min_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| {
                        if let Some(unit) = &self.y_unit {
                            if y_range.abs() < 0.001 {
                                let exponent = y_range.log10().floor() + 1.0;
                                format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit)
                            } else {
                                format!("{:.3} {}", n, unit)
                            }
                        } else {
                            if y_range.abs() < 0.001 {
                                let exponent = y_range.log10().floor() + 1.0;
                                format!("{:.1}e{}", n / 10f64.powf(exponent), exponent)
                            } else {
                                format!("{:.3}", n)
                            }
                        }
                    }),
            );
            ui.label("Max:");
            let r2 = ui.add(
                egui::DragValue::new(&mut y_max_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| {
                        if let Some(unit) = &self.y_unit {
                            if y_range.abs() < 0.001 {
                                let exponent = y_range.log10().floor() + 1.0;
                                format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit)
                            } else {
                                format!("{:.3} {}", n, unit)
                            }
                        } else {
                            if y_range.abs() < 0.001 {
                                let exponent = y_range.log10().floor() + 1.0;
                                format!("{:.1}e{}", n / 10f64.powf(exponent), exponent)
                            } else {
                                format!("{:.3}", n)
                            }
                        }
                    }),
            );
            if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp {
                self.y_min = y_min_tmp;
                self.y_max = y_max_tmp;
                self.pending_auto_y = false;
            }

            if ui
                .button("Fit")
                .on_hover_text("Fit the Y-axis to the visible data")
                .clicked()
            {
                // Clear manual bounds and request one-shot auto fit
                self.pending_auto_y = true;
            }
            // Continuous Y auto-zoom
            ui.checkbox(&mut self.auto_zoom_y, "Auto Zoom")
                .on_hover_text("Continuously fit the Y-axis to the currently visible data range");
            if self.auto_zoom_y {
                // Re-run auto-fit each frame while enabled
                self.pending_auto_y = true;
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Zoom:")
                    .on_hover_text("Select the zoom mode for mouse wheel zooming");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X-Axis");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y-Axis");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both");
            });

            ui.separator();

            if ui
                .button("Fit to View")
                .on_hover_text("Fit the view to the available data")
                .clicked()
            {
                self.pending_auto_x = true;
                self.pending_auto_y = true;
            }

            ui.separator();

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

            // Selection + pause/reset/clear (shared)
            if ui.button("Clear Measurement").clicked() {
                self.point_selection.clear();
            }

            if ui.button("Clear All").clicked() {
                for tr in self.traces.values_mut() {
                    tr.live.clear();
                    if let Some(s) = &mut tr.snap {
                        s.clear();
                    }
                }
                // Also reset all math runtime storage so integrators/filters/min/max start fresh
                self.reset_all_math_storage();
                self.point_selection.clear();
                // Clear all threshold events (global log and per-threshold buffers)
                self.clear_all_threshold_events();
            }

            ui.checkbox(&mut self.show_legend, "Legend")
                .on_hover_text("Show legend");

            // Optional extras: FFT toggle, Save PNG, Save raw (Main only)
            //if let ControlsMode::Main = mode {
            // Bottom panels entry (shared) using bottom_panels(), identical style to side_panels()
            {
                let mut panels = self.bottom_panels();
                for p in panels.iter_mut() {
                    // Read title with a short borrow, then drop it before doing UI
                    let title = { p.dock_mut().title };
                    if ui.button(format!("{}…", title)).clicked() {
                        let d = p.dock_mut();
                        d.show_dialog = true;
                        if !d.detached {
                            d.focus_dock = true;
                        }
                    }
                }
            }
            // Export actions: Save PNG + Save raw (CSV/Parquet)
            self.render_export_buttons(ui);
            //}

            // Dialogs entry (shared) using side_panels()
            {
                let mut panels = self.side_panels();
                for p in panels.iter_mut() {
                    // Read title with a short borrow, then drop it before doing UI
                    let title = { p.dock_mut().title };
                    if ui.button(format!("{}…", title)).clicked() {
                        let d = p.dock_mut();
                        d.show_dialog = true;
                        if !d.detached {
                            d.focus_dock = true;
                        }
                    }
                }
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
            pending_auto_x: false,
            y_unit: None,
            y_log: false,
            y_min: 0.0,
            y_max: 1.0,
            pending_auto_y: true,
            auto_zoom_y: false,
            show_legend: true,
            show_info_in_legend: false,
            zoom_mode: ZoomMode::X,
            math_defs: Vec::new(),
            math_states: HashMap::new(),
            threshold_controller: None,
            threshold_defs: Vec::new(),
            threshold_states: HashMap::new(),
            threshold_event_log: VecDeque::new(),
            threshold_event_log_cap: 10_000,
            hover_trace: None,
            hover_threshold: None,

            math_panel: MathPanel::default(),
            thresholds_panel: ThresholdsPanel::default(),
            traces_panel: TracesPanel::default(),
            #[cfg(feature = "fft")]
            fft_panel: FftPanel::default(),
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

        self.pause_on_click(&plot_response);

        // Sync time window with zoom when following is active (not paused)
        //self.sync_time_window_with_plot(&plot_response);
        self.apply_zoom(&plot_response);

        // Handle click for selection in embedded mode
        self.handle_plot_click(&plot_response);

        // Repaint soon
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn bottom_panels(&mut self) -> Vec<&mut dyn super::panel::DockPanel> {
        #[cfg(feature = "fft")]
        {
            vec![&mut self.fft_panel]
        }
        #[cfg(not(feature = "fft"))]
        {
            Vec::new()
        }
    }

    /// Update any external controllers about attached bottom-panel visibility (e.g., FFT).
    fn update_bottom_panels_controller_visibility(&mut self) {
        #[cfg(feature = "fft")]
        {
            if let Some(ctrl) = &self.fft_controller {
                let d = self.fft_panel.dock.clone();
                let mut inner = ctrl.inner.lock().unwrap();
                inner.show = d.show_dialog && !d.detached;
                let info = FftPanelInfo {
                    shown: inner.show,
                    current_size: inner.current_size,
                    requested_size: inner.request_set_size,
                };
                inner.listeners.retain(|s| s.send(info.clone()).is_ok());
            }
        }
    }

    /// Call a closure with the bottom panel at the given index temporarily moved out,
    /// then put it back. The index corresponds to the order returned by `bottom_panels()`.
    fn with_bottom_panel_at<F>(&mut self, index: usize, f: F)
    where
        F: FnMut(&mut dyn super::panel::DockPanel, &mut Self),
    {
        #[cfg(feature = "fft")]
        {
            if index == 0 {
                let mut p = std::mem::take(&mut self.fft_panel);
                f(&mut p, self);
                self.fft_panel = p;
                return;
            }
        }
        // no-op for unknown index or non-fft builds; avoid moving `f`
        let _ = index;
        // Ensure we don't move or require mut outside cfg
        let _ = &f;
    }

    // (take/put helper no longer needed here; side/bottom panels are handled inline like sidebar)

    /// Render the bottom panel container if any attached bottom panel is visible; includes header and body.
    fn render_bottom_panel(&mut self, ctx: &egui::Context) {
        // If any bottom panel is attached+visible, show a shared bottom container.
        let visible = {
            let mut panels = self.bottom_panels();
            panels.iter_mut().any(|p| {
                let d = p.dock_mut();
                !d.detached && d.show_dialog
            })
        };
        if !visible {
            return;
        }

        egui::TopBottomPanel::bottom("bottom_panels")
            .resizable(true)
            .default_height(300.0)
            .min_height(120.0)
            .show(ctx, |ui| {
                // Simple tabs like the right sidebar, with Hide/Pop out actions on the right
                let titles_flags: Vec<(&'static str, bool)> = {
                    let mut panels = self.bottom_panels();
                    panels
                        .iter_mut()
                        .map(|p| {
                            let d = p.dock_mut();
                            (d.title, !d.detached && d.show_dialog)
                        })
                        .collect()
                };
                let active_idx_current = titles_flags.iter().position(|(_, active)| *active);
                let mut clicked_idx: Option<usize> = None;
                ui.horizontal(|ui| {
                    if titles_flags.len() == 1 {
                        ui.strong(titles_flags[0].0);
                    } else {
                        for (i, (title, active)) in titles_flags.iter().enumerate() {
                            if ui.selectable_label(*active, *title).clicked() {
                                clicked_idx = Some(i);
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(ai) = active_idx_current {
                            if ui.button("Pop out").clicked() {
                                self.with_bottom_panel_at(ai, |p, app| {
                                    let d = p.dock_mut();
                                    d.detached = true;
                                    d.show_dialog = true;
                                    app.update_bottom_panels_controller_visibility();
                                });
                            }
                            if ui.button("Hide").clicked() {
                                self.with_bottom_panel_at(ai, |p, app| {
                                    let d = p.dock_mut();
                                    d.show_dialog = false;
                                    app.update_bottom_panels_controller_visibility();
                                });
                            }
                        }
                    });
                });
                if let Some(i) = clicked_idx {
                    let mut panels = self.bottom_panels();
                    for (j, p) in panels.iter_mut().enumerate() {
                        let d = p.dock_mut();
                        if j == i {
                            d.show_dialog = true;
                            d.detached = false;
                        } else if !d.detached {
                            d.show_dialog = false;
                        }
                    }
                }

                ui.separator();

                // Render the active attached bottom panel body (generic via bottom_panels), no extra header inside
                let active_idx: Option<usize> = {
                    let mut panels = self.bottom_panels();
                    panels.iter_mut().position(|p| {
                        let d = p.dock_mut();
                        !d.detached && d.show_dialog
                    })
                };
                if let Some(i) = active_idx {
                    self.with_bottom_panel_at(i, |p, app| {
                        let show_attached = {
                            let d = p.dock_mut();
                            !d.detached && d.show_dialog
                        };
                        if show_attached {
                            p.panel_contents(app, ui);
                        }
                    });
                }
            });
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
                look: {
                    let mut l = TraceLook::default();
                    l.color = color;
                    l
                },
                offset: 0.0,
                live: VecDeque::new(),
                snap: None,
                last_fft: None,
                is_math: true,
                info: String::new(),
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
                // Update info string with formula
                tr.info = Self::math_formula_string(def);
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
                        look: {
                            let mut l = TraceLook::default();
                            l.color = Self::alloc_color(idx);
                            l
                        },
                        offset: 0.0,
                        live: dq.clone(),
                        snap: if self.paused { Some(dq.clone()) } else { None },
                        last_fft: None,
                        is_math: true,
                        info: Self::math_formula_string(def),
                    },
                );
            }
        }
    }

    /// Reset runtime storage for all math traces that maintain state (filters, integrators, min/max).
    pub(crate) fn reset_all_math_storage(&mut self) {
        // Only reset traces that maintain internal state (integrators, filters, min/max)
        for def in self.math_defs.clone().into_iter() {
            let is_stateful = matches!(
                def.kind,
                crate::math::MathKind::Integrate { .. }
                    | crate::math::MathKind::Filter { .. }
                    | crate::math::MathKind::MinMax { .. }
            );
            if is_stateful {
                self.reset_math_storage(&def.name);
            }
        }
    }

    /// Reset runtime storage for a specific math trace (clears integrator, filter states, min/max, etc.).
    pub(crate) fn reset_math_storage(&mut self, name: &str) {
        if let Some(st) = self.math_states.get_mut(name) {
            *st = MathRuntimeState::new();
        }
        if let Some(tr) = self.traces.get_mut(name) {
            tr.live.clear();
            if let Some(s) = &mut tr.snap {
                s.clear();
            }
        }
    }

    /// Build a human-readable formula description for a math trace
    fn math_formula_string(def: &MathTraceDef) -> String {
        use crate::math::{FilterKind, MathKind, MinMaxMode};
        match &def.kind {
            MathKind::Add { inputs } => {
                if inputs.is_empty() {
                    "0".to_string()
                } else {
                    let mut s = String::new();
                    for (i, (r, g)) in inputs.iter().enumerate() {
                        if i > 0 {
                            s.push_str(" + ");
                        }
                        if (*g - 1.0).abs() < 1e-12 {
                            s.push_str(&r.0);
                        } else {
                            s.push_str(&format!("{:.3}*{}", g, r.0));
                        }
                    }
                    s
                }
            }
            MathKind::Multiply { a, b } => format!("{} * {}", a.0, b.0),
            MathKind::Divide { a, b } => format!("{} / {}", a.0, b.0),
            MathKind::Differentiate { input } => format!("d({})/dt", input.0),
            MathKind::Integrate { input, y0 } => format!("∫ {} dt  (y0={:.3})", input.0, y0),
            MathKind::Filter { input, kind } => {
                let k = match kind {
                    FilterKind::Lowpass { cutoff_hz } => format!("LP fc={:.3} Hz", cutoff_hz),
                    FilterKind::Highpass { cutoff_hz } => format!("HP fc={:.3} Hz", cutoff_hz),
                    FilterKind::Bandpass {
                        low_cut_hz,
                        high_cut_hz,
                    } => format!("BP [{:.3},{:.3}] Hz", low_cut_hz, high_cut_hz),
                    FilterKind::BiquadLowpass { cutoff_hz, q } => {
                        format!("BQ-LP fc={:.3} Q={:.3}", cutoff_hz, q)
                    }
                    FilterKind::BiquadHighpass { cutoff_hz, q } => {
                        format!("BQ-HP fc={:.3} Q={:.3}", cutoff_hz, q)
                    }
                    FilterKind::BiquadBandpass { center_hz, q } => {
                        format!("BQ-BP f0={:.3} Q={:.3}", center_hz, q)
                    }
                    FilterKind::Custom { .. } => "Custom biquad".to_string(),
                };
                format!("{} -> {}", input.0, k)
            }
            MathKind::MinMax {
                input,
                decay_per_sec,
                mode,
            } => {
                let mm = match mode {
                    MinMaxMode::Min => "Min",
                    MinMaxMode::Max => "Max",
                };
                match decay_per_sec {
                    Some(d) => format!("{}({}) with decay {:.3} 1/s", mm, input.0, d),
                    None => format!("{}({})", mm, input.0),
                }
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

    /// Clear all threshold events from the global log and from each threshold's runtime state.
    pub(crate) fn clear_all_threshold_events(&mut self) {
        self.threshold_event_log.clear();
        for (_name, st) in self.threshold_states.iter_mut() {
            st.events.clear();
        }
    }

    /// Clear all events for a specific threshold: removes from its buffer and from the global log.
    pub(crate) fn clear_threshold_events(&mut self, name: &str) {
        if let Some(st) = self.threshold_states.get_mut(name) {
            st.events.clear();
        }
        self.threshold_event_log.retain(|e| e.threshold != name);
    }

    /// Remove a specific threshold event from the global log and the corresponding threshold's buffer.
    pub(crate) fn remove_threshold_event(&mut self, event: &ThresholdEvent) {
        // Remove from global log (first match)
        if let Some(pos) = self.threshold_event_log.iter().position(|e| {
            e.threshold == event.threshold
                && e.trace == event.trace
                && e.start_t == event.start_t
                && e.end_t == event.end_t
                && e.duration == event.duration
                && e.area == event.area
        }) {
            self.threshold_event_log.remove(pos);
        }
        // Remove from per-threshold buffer
        if let Some(st) = self.threshold_states.get_mut(&event.threshold) {
            if let Some(pos) = st.events.iter().position(|e| {
                e.trace == event.trace
                    && e.start_t == event.start_t
                    && e.end_t == event.end_t
                    && e.duration == event.duration
                    && e.area == event.area
            }) {
                st.events.remove(pos);
            }
        }
    }

    pub(crate) fn remove_threshold_internal(&mut self, name: &str) {
        self.threshold_defs.retain(|d| d.name != name);
        self.threshold_states.remove(name);
        // Remove any stored looks for this threshold
        self.thresholds_panel.looks.remove(name);
        self.thresholds_panel.start_looks.remove(name);
        self.thresholds_panel.stop_looks.remove(name);
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
        // Prefer panel state; keep legacy fields in sync if still used elsewhere
        self.math_panel.error = None;
        if let Some(orig) = self.math_panel.editing.clone() {
            match self.update_math_trace(&orig, def) {
                Ok(()) => {
                    self.math_panel.editing = None;
                    self.math_panel.builder = MathBuilderState::default();
                }
                Err(e) => {
                    self.math_panel.error = Some(e.to_string());
                }
            }
        } else {
            if self.traces.contains_key(&def.name) {
                self.math_panel.error = Some("A trace with this name already exists".into());
                return;
            }
            self.add_math_trace_internal(def);
            self.math_panel.builder = MathBuilderState::default();
        }
        // no-op: legacy mirrors removed
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
        // Focus requests from detached windows
        self.process_focus_requests();

        // Controls
        egui::TopBottomPanel::top("controls_multi").show(ctx, |ui| {
            self.controls_ui(ui, ControlsMode::Main);
        });

        // Right-side panel
        self.render_right_sidebar_panel(ctx);

        // Shared dialogs
        self.show_dialogs_shared(ctx);

        // Bottom dock panels (FFT etc.)
        self.render_bottom_panel(ctx);

        // Plot all traces in the central panel
        self.render_central_plot_panel(ctx);

        // Repaint
        Self::repaint_tick(ctx);

        // Apply any external UI action requests (pause/resume/screenshot)
        self.handle_ui_action_requests();

        // Window controller: publish current window info and apply any pending requests.
        self.handle_window_controller_requests(ctx);

        // Screenshot request and saving
        self.handle_screenshot_result(ctx);
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
