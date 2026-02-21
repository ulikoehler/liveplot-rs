//! Standalone application wrapper for LivePlot.
//!
//! [`LivePlotApp`] wraps a [`LivePlotPanel`](super::LivePlotPanel) and implements
//! [`eframe::App`] so that LivePlot can run as a native window.  It also
//! holds the controller handles for the standalone case and owns the
//! logic that applies controller requests against the main panel each frame.

use eframe::egui;

use crate::controllers::{
    FFTController, LiveplotController, ScopesController, ThresholdController, TracesController,
    UiActionController, WindowController,
};
use crate::data::data::LivePlotData;
use crate::data::export;
use crate::data::hotkeys as hotkey_helpers;
use crate::data::traces::TraceRef;
use crate::PlotCommand;

use super::LivePlotPanel;

// ─────────────────────────────────────────────────────────────────────────────
// LivePlotApp
// ─────────────────────────────────────────────────────────────────────────────

/// Standalone LivePlot application that implements [`eframe::App`].
///
/// `LivePlotApp` is the top-level container used when LivePlot runs in its own
/// native window (via [`run_liveplot`](super::run_liveplot)).  It:
///
/// 1. Owns a [`LivePlotPanel`] that does the actual rendering.
/// 2. Holds optional controller handles for programmatic interaction.
/// 3. Processes controller requests each frame in [`apply_controllers`](Self::apply_controllers).
/// 4. Applies initial configuration from [`LivePlotConfig`](crate::config::LivePlotConfig).
pub struct LivePlotApp {
    /// The inner panel widget that owns all data and UI state.
    pub main_panel: LivePlotPanel,

    // ── Optional external controllers ────────────────────────────────────────
    /// Controls the host window (size, position).
    pub window_ctrl: Option<WindowController>,
    /// Programmatic UI actions (pause, screenshot, export).
    pub ui_ctrl: Option<UiActionController>,
    /// Programmatic trace manipulation (colour, visibility, offset, etc.).
    pub traces_ctrl: Option<TracesController>,
    /// Programmatic scope management (add/remove/configure scopes).
    pub scopes_ctrl: Option<ScopesController>,
    /// High-level liveplot control (pause all, clear all, save/load state).
    pub liveplot_ctrl: Option<LiveplotController>,
    /// FFT panel control (show/hide, resize).
    pub fft_ctrl: Option<FFTController>,
    /// Threshold management (add/remove thresholds, listen for threshold events).
    pub threshold_ctrl: Option<ThresholdController>,

    /// Optional heading text shown at the top of the window.
    pub headline: Option<String>,
    /// Optional sub-heading text shown below the headline.
    pub subheadline: Option<String>,

    /// Color scheme to apply to the egui context. Applied once on the first frame.
    pub color_scheme: Option<crate::config::ColorScheme>,
    /// Flag so we only apply the color scheme on the very first frame.
    color_scheme_applied: bool,
}

impl LivePlotApp {
    /// Create a new `LivePlotApp` without any controllers.
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        Self {
            main_panel: LivePlotPanel::new(rx),
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            scopes_ctrl: None,
            liveplot_ctrl: None,
            fft_ctrl: None,
            threshold_ctrl: None,
            headline: None,
            subheadline: None,
            color_scheme: None,
            color_scheme_applied: false,
        }
    }

    /// Create a new `LivePlotApp` with the given controller handles already wired.
    pub fn with_controllers(
        rx: std::sync::mpsc::Receiver<PlotCommand>,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        scopes_ctrl: Option<ScopesController>,
        liveplot_ctrl: Option<LiveplotController>,
        fft_ctrl: Option<FFTController>,
        threshold_ctrl: Option<ThresholdController>,
    ) -> Self {
        let mut main_panel = LivePlotPanel::new(rx);
        main_panel.set_controllers(
            window_ctrl.clone(),
            ui_ctrl.clone(),
            traces_ctrl.clone(),
            scopes_ctrl.clone(),
            liveplot_ctrl.clone(),
            fft_ctrl.clone(),
            threshold_ctrl.clone(),
        );
        Self {
            main_panel,
            window_ctrl,
            ui_ctrl,
            traces_ctrl,
            scopes_ctrl,
            liveplot_ctrl,
            fft_ctrl,
            threshold_ctrl,
            headline: None,
            subheadline: None,
            color_scheme: None,
            color_scheme_applied: false,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Configuration
    // ─────────────────────────────────────────────────────────────────────────

    /// Apply a [`LivePlotConfig`](crate::config::LivePlotConfig) to configure
    /// axes, trace limits, hotkeys, responsive thresholds, and more.
    ///
    /// Typically called once right after construction, before entering the event loop.
    pub(crate) fn apply_config(&mut self, cfg: &mut crate::config::LivePlotConfig) {
        // Axis / time window settings.
        {
            let scope = self.main_panel.liveplot_panel.get_data_mut();
            for s in scope {
                s.time_window = cfg.time_window_secs;
                s.y_axis.set_unit(cfg.y_unit.clone());
                s.y_axis.log_scale = cfg.y_log;
                s.x_axis.axis_type =
                    crate::data::scope::AxisType::Time(crate::data::scope::XDateFormat::default());
                s.x_axis.x_formatter = cfg.x_formatter.clone();
                s.show_legend = cfg.features.legend;
                s.auto_fit_to_view = cfg.auto_fit.auto_fit_to_view;
                s.keep_max_fit = cfg.auto_fit.keep_max_fit;
            }
        }

        // Trace storage limits.
        self.main_panel.traces_data.max_points = cfg.max_points;

        // Hotkeys: configured or fallback to default path, then defaults.
        {
            let mut hk = self.main_panel.hotkeys.borrow_mut();
            *hk = cfg
                .hotkeys
                .clone()
                .or_else(|| crate::data::hotkeys::Hotkeys::load_from_default_path().ok())
                .unwrap_or_default();
        }

        // Headline / subheadline for optional top banner.
        self.headline = cfg.headline.clone();
        self.subheadline = cfg.subheadline.clone();

        // ── Responsive layout configuration ──────────────────────────────────
        self.main_panel.top_bar_buttons = cfg.layout.top_bar_buttons.clone();
        self.main_panel.sidebar_buttons = cfg.layout.sidebar_buttons.clone();
        self.main_panel.min_height_for_top_bar = cfg.layout.min_height_for_top_bar;
        self.main_panel.min_width_for_sidebar = cfg.layout.min_width_for_sidebar;
        self.main_panel.min_height_for_sidebar = cfg.layout.min_height_for_sidebar;

        // ── Tick-label visibility thresholds ─────────────────────────────────
        self.main_panel.liveplot_panel.set_tick_label_thresholds(
            cfg.layout.min_width_for_y_ticklabels,
            cfg.layout.min_height_for_x_ticklabels,
        );
        self.main_panel.liveplot_panel.set_legend_thresholds(
            cfg.layout.min_width_for_legend,
            cfg.layout.min_height_for_legend,
        );

        // ── Color scheme ─────────────────────────────────────────────────────
        self.color_scheme = Some(cfg.color_scheme.clone());
        // take overlay callback out of config so ownership moves into panel
        self.main_panel.overlays = cfg.overlays.take();
        self.color_scheme_applied = false;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Controller processing (standalone mode)
    // ─────────────────────────────────────────────────────────────────────────

    /// Process controller requests and publish state snapshots (standalone mode).
    ///
    /// This is called once per frame *after* the main panel has rendered.
    /// It mirrors [`LivePlotPanel::apply_controllers_embedded`](LivePlotPanel::apply_controllers_embedded)
    /// but operates on a [`LivePlotApp`] that holds controller clones separately
    /// and has access to the full eframe context.
    fn apply_controllers(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── WindowController ─────────────────────────────────────────────────
        if let Some(ctrl) = &self.window_ctrl {
            let (req_size, req_pos) = {
                let mut inner = ctrl.inner.lock().unwrap();
                (inner.request_set_size.take(), inner.request_set_pos.take())
            };
            if let Some([w, h]) = req_size {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
            }
            let rect = ctx.input(|i| i.content_rect());
            let size = [rect.width(), rect.height()];
            let pos = [rect.left(), rect.top()];
            let info = crate::controllers::WindowInfo {
                current_size: Some(size),
                current_pos: Some(pos),
                requested_size: req_size,
                requested_pos: req_pos,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.current_size = Some(size);
            inner.current_pos = Some(pos);
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // ── UiActionController ───────────────────────────────────────────────
        if let Some(ctrl) = &self.ui_ctrl {
            let mut take_actions = {
                let mut inner = ctrl.inner.lock().unwrap();
                (
                    inner.request_pause.take(),
                    {
                        let v = inner.request_screenshot;
                        inner.request_screenshot = false;
                        v
                    },
                    inner.request_screenshot_to.take(),
                    inner.request_save_raw.take(),
                    inner.request_save_raw_to.take(),
                    inner.fft_request.take(),
                )
            };

            let scopes = self.main_panel.liveplot_panel.get_data_mut();

            for scope in scopes {
                if let Some(p) = take_actions.0 {
                    if p {
                        scope.paused = true;
                        self.main_panel.traces_data.take_snapshot();
                    } else {
                        scope.paused = false;
                    }
                }
                if take_actions.1 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                if let Some(path) = take_actions.2.take() {
                    std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                if let Some((_fmt, path)) = take_actions.4.take() {
                    let tol = 1e-9;
                    let order = scope.trace_order.clone();
                    let series = order
                        .iter()
                        .filter_map(|name| {
                            scope
                                .get_drawn_points(name, &self.main_panel.traces_data)
                                .map(|v| (name.clone(), v.into_iter().collect()))
                        })
                        .collect();
                    let _ = if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                        export::write_csv_aligned_path(&path, &order, &series, tol)
                    } else {
                        export::write_parquet_aligned_path(&path, &order, &series, tol)
                    };
                }
                if let Some(_req) = take_actions.5.take() {
                    // Placeholder for FFT data requests
                }
            }
        }

        // ── TracesController ─────────────────────────────────────────────────
        if let Some(ctrl) = self.traces_ctrl.clone() {
            let (show_request, detached_request) = {
                let mut inner = ctrl.inner.lock().unwrap();
                let show_request = inner.show_request.take();
                let detached_request = inner.detached_request.take();

                let mut data = LivePlotData {
                    scope_data: self.main_panel.liveplot_panel.get_data_mut(),
                    traces: &mut self.main_panel.traces_data,
                    pending_requests: &mut self.main_panel.pending_requests,
                    event_ctrl: self.main_panel.event_ctrl.clone(),
                };

                // Apply trace property mutations.
                for (name, rgb) in inner.color_requests.drain(..) {
                    let tref = TraceRef(name.clone());
                    if let Some(tr) = data.traces.get_trace_mut(&tref) {
                        tr.look.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    }
                }
                for (name, vis) in inner.visible_requests.drain(..) {
                    let tref = TraceRef(name.clone());
                    if let Some(tr) = data.traces.get_trace_mut(&tref) {
                        tr.look.visible = vis;
                    }
                }
                for (name, off) in inner.offset_requests.drain(..) {
                    let tref = TraceRef(name.clone());
                    if let Some(tr) = data.traces.get_trace_mut(&tref) {
                        tr.offset = off;
                    }
                }
                if let Some(unit) = inner.y_unit_request.take() {
                    for scope in data.scope_data.iter_mut() {
                        let scope = &mut **scope;
                        scope.y_axis.set_unit(unit.clone());
                    }
                }
                if let Some(ylog) = inner.y_log_request.take() {
                    for scope in data.scope_data.iter_mut() {
                        let scope = &mut **scope;
                        scope.y_axis.log_scale = ylog;
                    }
                }
                if let Some(mp) = inner.max_points_request.take() {
                    data.traces.max_points = mp;
                }
                if let Some(bounds) = inner.points_bounds_request.take() {
                    data.traces.points_bounds = bounds;
                    data.traces.max_points = data.traces.max_points.clamp(bounds.0, bounds.1);
                }
                if let Some(ht) = inner.hover_trace_request.take() {
                    data.traces.hover_trace = ht;
                }
                for (name, width) in inner.width_requests.drain(..) {
                    let tref = TraceRef(name.clone());
                    if let Some(tr) = data.traces.get_trace_mut(&tref) {
                        tr.look.width = width;
                    }
                }
                for (name, style) in inner.style_requests.drain(..) {
                    let tref = TraceRef(name.clone());
                    if let Some(tr) = data.traces.get_trace_mut(&tref) {
                        tr.look.style = style;
                    }
                }

                // Build and publish trace info snapshot.
                let mut infos: Vec<crate::controllers::TraceInfo> = Vec::new();
                if let Some(scope) = data.primary_scope() {
                    for name in scope.trace_order.iter() {
                        if let Some(tr) = data.traces.get_trace(name) {
                            infos.push(crate::controllers::TraceInfo {
                                name: name.0.clone(),
                                color_rgb: [
                                    tr.look.color.r(),
                                    tr.look.color.g(),
                                    tr.look.color.b(),
                                ],
                                visible: tr.look.visible,
                                offset: tr.offset,
                            });
                        }
                    }
                    let y_unit = scope.y_axis.get_unit();
                    let y_log = scope.y_axis.log_scale;
                    let snapshot = crate::controllers::TracesInfo {
                        traces: infos,
                        y_unit,
                        y_log,
                    };
                    inner.last_snapshot = Some(snapshot.clone());
                    inner.listeners.retain(|s| s.send(snapshot.clone()).is_ok());
                }

                (show_request, detached_request)
            };

            if let Some(show) = show_request {
                if let Some(tp) = self.main_panel.traces_panel_mut() {
                    tp.state.visible = show;
                }
            }
            if let Some(detached) = detached_request {
                if let Some(tp) = self.main_panel.traces_panel_mut() {
                    tp.state.detached = detached;
                    if detached {
                        tp.state.visible = true;
                    }
                }
            }

            // Publish panel-level state snapshot.
            let mut trace_states: Vec<crate::controllers::TraceControlState> = Vec::new();
            for (name, tr) in self.main_panel.traces_data.traces_iter() {
                trace_states.push(crate::controllers::TraceControlState {
                    name: name.clone(),
                    color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                    width: tr.look.width,
                    style: tr.look.style,
                    visible: tr.look.visible,
                    offset: tr.offset,
                });
            }
            let (panel_show, panel_detached) = {
                let mut show = true;
                let mut detached = false;
                if let Some(tp) = self.main_panel.traces_panel_mut() {
                    show = tp.state.visible;
                    detached = tp.state.detached;
                }
                (show, detached)
            };
            let panel_state = crate::controllers::TracesPanelState {
                max_points: self.main_panel.traces_data.max_points,
                points_bounds: self.main_panel.traces_data.points_bounds,
                hover_trace: self.main_panel.traces_data.hover_trace.clone(),
                traces: trace_states,
                show: panel_show,
                detached: panel_detached,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.last_panel_state = Some(panel_state.clone());
            inner
                .panel_listeners
                .retain(|s| s.send(panel_state.clone()).is_ok());
        }

        // ── ScopesController ─────────────────────────────────────────────────
        if let Some(ctrl) = self.scopes_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            if requests.add_scope {
                self.main_panel.liveplot_panel.add_scope();
            }
            if let Some(id) = requests.remove_scope {
                let _ = self.main_panel.liveplot_panel.remove_scope_by_id(id);
            }
            if requests.save_screenshot {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if !requests.set_scopes.is_empty() {
                let traces = &mut self.main_panel.traces_data;
                for scope_req in requests.set_scopes {
                    let mut scopes = self.main_panel.liveplot_panel.get_data_mut();
                    if let Some(scope) = scopes.iter_mut().find(|s| s.id == scope_req.id) {
                        scope.name = scope_req.name.clone();
                        scope.y_axis = scope_req.y_axis.clone();
                        scope.x_axis = scope_req.x_axis.clone();
                        scope.time_window = scope_req.time_window;
                        scope.paused = scope_req.paused;
                        scope.show_legend = scope_req.show_legend;
                        scope.show_info_in_legend = scope_req.show_info_in_legend;
                        scope.scope_type = scope_req.scope_type;
                        scope.trace_order = scope_req.trace_order.clone();
                        scope.trace_order.retain(|t| traces.contains_key(t));
                    }
                }
            }

            let scopes_state = {
                let scopes = self.main_panel.liveplot_panel.get_data_mut();
                let mut scopes_info: Vec<crate::controllers::ScopeControlState> = Vec::new();
                for scope in scopes {
                    scopes_info.push(crate::controllers::ScopeControlState {
                        id: scope.id,
                        name: scope.name.clone(),
                        y_axis: scope.y_axis.clone(),
                        x_axis: scope.x_axis.clone(),
                        time_window: scope.time_window,
                        paused: scope.paused,
                        show_legend: scope.show_legend,
                        show_info_in_legend: scope.show_info_in_legend,
                        trace_order: scope.trace_order.clone(),
                        scope_type: scope.scope_type,
                    });
                }
                crate::controllers::ScopesState {
                    scopes: scopes_info,
                    show: true,
                    detached: false,
                }
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.last_state = Some(scopes_state.clone());
            inner
                .listeners
                .retain(|s| s.send(scopes_state.clone()).is_ok());
        }

        // ── LiveplotController ───────────────────────────────────────────────
        if let Some(ctrl) = self.liveplot_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            {
                let mut data = LivePlotData {
                    scope_data: self.main_panel.liveplot_panel.get_data_mut(),
                    traces: &mut self.main_panel.traces_data,
                    pending_requests: &mut self.main_panel.pending_requests,
                    event_ctrl: self.main_panel.event_ctrl.clone(),
                };
                if let Some(pause) = requests.pause_all {
                    if pause {
                        data.pause_all();
                    } else {
                        data.resume_all();
                    }
                }
                if requests.clear_all {
                    data.request_clear_all();
                }
                if let Some(path) = requests.save_state {
                    data.pending_requests.save_state = Some(path);
                }
                if let Some(path) = requests.load_state {
                    data.pending_requests.load_state = Some(path);
                }
                if requests.add_scope {
                    self.main_panel.liveplot_panel.add_scope();
                }
                if let Some(id) = requests.remove_scope {
                    let _ = self.main_panel.liveplot_panel.remove_scope_by_id(id);
                }
                // Reorder not yet supported; consume request.
                let _ = requests.reorder_scopes;
            }

            if let Some(size) = requests.set_window_size {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    size[0], size[1],
                )));
            }
            if let Some(pos) = requests.set_window_pos {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(
                    pos[0], pos[1],
                )));
            }
            if requests.request_focus {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }

            let rect = ctx.input(|i| i.content_rect());
            let paused = {
                let data = LivePlotData {
                    scope_data: self.main_panel.liveplot_panel.get_data_mut(),
                    traces: &mut self.main_panel.traces_data,
                    pending_requests: &mut self.main_panel.pending_requests,
                    event_ctrl: self.main_panel.event_ctrl.clone(),
                };
                data.are_all_paused()
            };
            let liveplot_state = crate::controllers::LiveplotState {
                paused,
                show: true,
                detached: false,
                window_size: Some([rect.width(), rect.height()]),
                window_pos: Some([rect.left(), rect.top()]),
                fft_size: requests.set_fft_size,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.last_state = Some(liveplot_state.clone());
            inner
                .listeners
                .retain(|s| s.send(liveplot_state.clone()).is_ok());
        }

        // ── FFTController ────────────────────────────────────────────────────
        if let Some(ctrl) = &self.fft_ctrl {
            let mut inner = ctrl.inner.lock().unwrap();
            let info = crate::controllers::FFTPanelInfo {
                shown: inner.show,
                current_size: None,
                requested_size: inner.request_set_size,
            };
            inner.last_info = Some(info.clone());
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// eframe integration
// ─────────────────────────────────────────────────────────────────────────────

impl eframe::App for LivePlotApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Apply color scheme once on the first frame (after egui context is available).
        if !self.color_scheme_applied {
            if let Some(scheme) = &self.color_scheme {
                scheme.apply(ctx);
                // existing traces may have been created before the scheme was
                // applied, so ensure their colours are updated to match the
                // new palette.  This is a no-op if the palette hasn't changed
                // (or if there are no traces yet).
                self.main_panel.traces_data.recolor_using_palette();
            }
            self.color_scheme_applied = true;
        }

        // Process global hotkey bindings (panel toggles, pause/resume, etc.).
        hotkey_helpers::handle_hotkeys(&mut self.main_panel, ctx);

        // Optional headline banner at the top of the window.
        if self.headline.is_some() || self.subheadline.is_some() {
            egui::TopBottomPanel::top("liveplot_headline").show(ctx, |ui| {
                if let Some(h) = &self.headline {
                    ui.heading(h);
                }
                if let Some(sub) = &self.subheadline {
                    ui.label(sub);
                }
            });
        }

        // Main content area.
        egui::CentralPanel::default().show(ctx, |ui| {
            self.main_panel.update(ui);
        });

        // Apply and publish controller requests after the main panel has updated.
        self.apply_controllers(ctx, frame);

        // Request continuous repainting (~60 fps).
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
