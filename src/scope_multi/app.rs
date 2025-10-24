//! Core multi-trace oscilloscope app wiring.

use eframe::{self, egui};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::controllers::{FFTController, TracesController, UiActionController, WindowController};
#[cfg(feature = "fft")]
pub use crate::fft::FFTWindow;
#[cfg(not(feature = "fft"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FFTWindow {
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
use crate::math::{MathRuntimeState, MathTraceDef};
use crate::point_selection::PointSelection;
use crate::sink::PlotCommand;
use crate::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdRuntimeState};

#[cfg(feature = "fft")]
use super::fft_panel::FFTPanel;
use super::math_ui::MathPanel;
use super::thresholds_ui::ThresholdsPanel;
use super::traces_ui::TracesPanel;
use super::types::TraceState;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum ControlsMode {
    Embedded,
    Main,
}

// Removed RightTab enum: sidebar visibility and active tab are derived from per-panel DockState

/// Egui app that displays multiple traces and supports point selection and FFT.
pub struct ScopeAppMulti {
    pub rx: Receiver<PlotCommand>,
    /// Map of trace ID -> trace name (for resolving incoming data to existing name-keyed storage)
    pub(super) id_to_name: std::collections::HashMap<crate::sink::TraceId, String>,
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
    pub fft_controller: Option<FFTController>,
    /// Optional controller for high-level UI actions (pause/resume/screenshot)
    pub ui_action_controller: Option<UiActionController>,
    /// Optional controller to observe and modify trace colors/visibility/marker selection
    pub traces_controller: Option<TracesController>,
    // FFT related
    pub show_fft: bool,
    pub fft_size: usize,
    pub fft_window: FFTWindow,
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
    pub(super) fft_panel: FFTPanel,
}

impl ScopeAppMulti {
    /// One-shot shared update: ingest -> prune -> recompute math -> thresholds -> traces publish
    pub(super) fn tick_non_ui(&mut self) {
        self.drain_rx_and_update_traces();
        self.prune_by_time_window();
        self.recompute_math_traces();
        self.apply_threshold_controller_requests();
        self.process_thresholds();
        self.apply_traces_controller_requests_and_publish();
    }

    pub fn new(rx: Receiver<PlotCommand>) -> Self {
        Self {
            rx,
            id_to_name: std::collections::HashMap::new(),
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
            fft_window: FFTWindow::Hann,
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
            fft_panel: FFTPanel::default(),
        }
    }
}

impl eframe::App for ScopeAppMulti {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Shared non-UI tick
        self.tick_non_ui();
        // Focus requests from detached windows
        self.process_focus_requests();

        // Top-left application menu bar: File and Functions
        let mut did_toggle_bottom_panel = false;
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("🗁 File", |ui| {
                    if ui.button("🖼 Save PNG").on_hover_text("Take a screenshot of the entire window").clicked() {
                        self.request_window_shot = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("🗠 Export traces…").on_hover_text({
                        #[cfg(feature = "parquet")] { "Export all traces as CSV or Parquet" }
                        #[cfg(not(feature = "parquet"))] { "Export all traces as CSV" }
                    }).clicked() {
                        self.prompt_and_save_raw_data();
                        ui.close();
                    }
                    if ui.button("⚠️ Export threshold events…").on_hover_text("Export filtered or all threshold events as CSV").clicked() {
                        self.prompt_and_save_threshold_events();
                        ui.close();
                    }
                });
                ui.menu_button("👁 View", |ui| {
                    // Zoom mode moved from controls into the View menu as a single-line picker
                    use super::app::ZoomMode;
                    ui.horizontal(|ui| {
                        ui.label("Zoom mode:");
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X-Axis").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y-Axis").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both").clicked() { ui.close(); }
                    });
                });
                ui.menu_button("☆ Functions", |ui| {
                    // Bottom panels (e.g., FFT)
                    {
                        let mut panels = self.bottom_panels();
                        for p in panels.iter_mut() {
                            let title = { p.dock_mut().title };
                            if ui.button(title).clicked() {
                                let d = p.dock_mut();
                                d.show_dialog = true;
                                d.detached = false;
                                d.focus_dock = true;
                                did_toggle_bottom_panel = true;
                                ui.close();
                            }
                        }
                    }
                    ui.separator();
                    // Right-side panels (Traces, Math, Thresholds)
                    {
                        let mut panels = self.side_panels();
                        for p in panels.iter_mut() {
                            let title = { p.dock_mut().title };
                            if ui.button(title).clicked() {
                                let d = p.dock_mut();
                                d.show_dialog = true;
                                if !d.detached { d.focus_dock = true; }
                                ui.close();
                            }
                        }
                    }
                });
            });
        });
        if did_toggle_bottom_panel { self.update_bottom_panels_controller_visibility(); }

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
    rx: Receiver<PlotCommand>,
    cfg: crate::config::LivePlotConfig,
) -> eframe::Result<()> {
    let mut options = cfg
        .native_options
        .unwrap_or_else(eframe::NativeOptions::default);
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    let title = cfg
        .title
        .clone()
        .unwrap_or_else(|| "LivePlot".to_string());
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
