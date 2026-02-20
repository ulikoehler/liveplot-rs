use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static PANEL_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

use eframe::egui;
use eframe::egui::scroll_area::{ScrollBarVisibility, ScrollSource};

use crate::config::ScopeButton;
use crate::controllers::{
    FFTController, LiveplotController, ScopesController, ThresholdController, TracesController,
    UiActionController, WindowController,
};
use crate::data::export;
use crate::data::hotkeys as hotkey_helpers;
use crate::data::hotkeys::{
    format_button_tooltip, get_hotkey_for_name, should_collapse_topbar, Hotkeys,
};
use crate::data::traces::{TraceRef, TracesCollection};
use egui_phosphor::regular::BROOM;

use crate::data::data::{LivePlotData, LivePlotRequests};
use crate::panels::liveplot_ui::LiveplotPanel;
use crate::panels::panel_trait::Panel;
use crate::PlotCommand;

/// Computed layout describing which buttons appear where for a single frame.
struct EffectiveLayout {
    /// Buttons to render in the top menu bar (empty ⟹ top bar is not shown).
    top_bar_buttons: Vec<ScopeButton>,
    /// Buttons to render in the sidebar icon strip (empty ⟹ no icon strip).
    sidebar_buttons: Vec<ScopeButton>,
    /// Whether the top menu bar is visible.
    show_top_bar: bool,
    /// Whether sidebar panel content (attached panel widgets) is visible.
    show_sidebar_panels: bool,
}

// use crate::panels::{
//     export_ui::ExportPanel, fft_ui::FftPanel, math_ui::MathPanel, scope_ui::ScopePanel,
//     thresholds_ui::ThresholdsPanel, traces_ui::TracesPanel, triggers_ui::TriggersPanel,
// };
#[cfg(feature = "fft")]
use crate::panels::fft_ui::FftPanel;
use crate::panels::{
    export_ui::ExportPanel, hotkeys_ui::HotkeysPanel, math_ui::MathPanel,
    measurment_ui::MeasurementPanel, thresholds_ui::ThresholdsPanel, traces_ui::TracesPanel,
    triggers_ui::TriggersPanel,
};

pub struct MainPanel {
    // Traces
    pub traces_data: TracesCollection,
    pub hotkeys: Rc<RefCell<Hotkeys>>,
    // Panels
    pub liveplot_panel: LiveplotPanel,
    pub right_side_panels: Vec<Box<dyn Panel>>,
    pub left_side_panels: Vec<Box<dyn Panel>>,
    pub bottom_panels: Vec<Box<dyn Panel>>,
    pub detached_panels: Vec<Box<dyn Panel>>,
    pub empty_panels: Vec<Box<dyn Panel>>,
    // Optional controllers for embedded usage
    pub(crate) window_ctrl: Option<WindowController>,
    pub(crate) ui_ctrl: Option<UiActionController>,
    pub(crate) traces_ctrl: Option<TracesController>,
    pub(crate) scopes_ctrl: Option<ScopesController>,
    pub(crate) liveplot_ctrl: Option<LiveplotController>,
    pub(crate) fft_ctrl: Option<FFTController>,
    pub(crate) threshold_ctrl: Option<ThresholdController>,
    pub(crate) threshold_event_cursors: HashMap<String, usize>,

    pub pending_requests: LivePlotRequests,

    // ── Responsive button-layout configuration ───────────────────────────────
    /// Buttons placed in the top menu bar.  `None` = the full default set.
    pub top_bar_buttons: Option<Vec<ScopeButton>>,
    /// Buttons placed in the right sidebar icon strip.  `None` = empty (standard behaviour).
    pub sidebar_buttons: Option<Vec<ScopeButton>>,
    /// Minimum plot-area height (px) before the top bar is hidden and its buttons move to sidebar.
    pub min_height_for_top_bar: f32,
    /// Minimum plot-area width (px) before the sidebar is hidden and its buttons move to top bar.
    pub min_width_for_sidebar: f32,
    /// Minimum plot-area height (px) before the sidebar is hidden and its buttons move to top bar.
    pub min_height_for_sidebar: f32,
    /// Central-panel size captured at the end of the previous frame (for responsive decisions).
    last_plot_size: egui::Vec2,
    /// Unique ID for this panel instance, used to namespace egui panel IDs.
    panel_id: u64,
    /// When `true`, the inner CentralPanel is rendered with no frame/margin so the plot
    /// fills every pixel of the allocated space.  Useful for dense embedded grid layouts.
    pub compact: bool,
}

impl MainPanel {
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        let hotkeys = Rc::new(RefCell::new(Hotkeys::default()));
        Self {
            traces_data: TracesCollection::new(rx),
            hotkeys: hotkeys.clone(),
            liveplot_panel: LiveplotPanel::default(),
            right_side_panels: vec![
                Box::new(TracesPanel::default()),
                Box::new(MathPanel::default()),
                Box::new(HotkeysPanel::new(hotkeys.clone())),
                Box::new(ThresholdsPanel::default()),
                Box::new(TriggersPanel::default()),
                Box::new(MeasurementPanel::default()),
            ],
            left_side_panels: vec![],
            #[cfg(feature = "fft")]
            bottom_panels: vec![Box::new(FftPanel::default())],
            #[cfg(not(feature = "fft"))]
            bottom_panels: vec![],
            detached_panels: vec![],
            empty_panels: vec![Box::new(ExportPanel::default())],
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            scopes_ctrl: None,
            liveplot_ctrl: None,
            fft_ctrl: None,
            threshold_ctrl: None,
            threshold_event_cursors: HashMap::new(),
            pending_requests: LivePlotRequests::default(),
            top_bar_buttons: None,
            sidebar_buttons: None,
            min_height_for_top_bar: 200.0,
            min_width_for_sidebar: 150.0,
            min_height_for_sidebar: 200.0,
            // Initialise to a large number so that no suppression happens on the first frame.
            last_plot_size: egui::Vec2::new(10_000.0, 10_000.0),
            panel_id: PANEL_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            compact: false,
        }
    }

    /// Attach controllers for embedded usage. These mirror the controllers used by MainApp.
    pub fn set_controllers(
        &mut self,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        scopes_ctrl: Option<ScopesController>,
        liveplot_ctrl: Option<LiveplotController>,
        fft_ctrl: Option<FFTController>,
        threshold_ctrl: Option<ThresholdController>,
    ) {
        self.window_ctrl = window_ctrl;
        self.ui_ctrl = ui_ctrl;
        self.traces_ctrl = traces_ctrl;
        self.scopes_ctrl = scopes_ctrl;
        self.liveplot_ctrl = liveplot_ctrl;
        self.fft_ctrl = fft_ctrl;
        self.threshold_ctrl = threshold_ctrl;
    }

    pub fn update(&mut self, ui: &mut egui::Ui) {
        // Capture the full widget size BEFORE any layout (top bar, sidebars, etc.)
        // is applied.  This is the total area available to the entire plot widget
        // and is used for responsive min-width / min-height decisions.
        self.last_plot_size = ui.max_rect().size();

        self.update_data();

        // Propagate the total widget size to every scope panel so their tick-label
        // hide decisions also use the complete widget dimensions.
        self.liveplot_panel
            .set_total_widget_size(self.last_plot_size);

        // In compact mode, skip all chrome (menu bar, sidebars, bottom panels)
        // so the plot fills the entire allocated area.  This avoids collapsed
        // panel stubs stealing space from very small embedded cells.
        if !self.compact {
            self.render_menu(ui);
            self.render_panels(ui);
        }

        // Draw additional overlay objects from other panels (e.g., thresholds)
        let central_panel = egui::CentralPanel::default();
        let central_panel = if self.compact {
            central_panel.frame(egui::Frame::NONE)
        } else {
            central_panel
        };
        central_panel.show_inside(ui, |ui| {
            use std::cell::RefCell;
            // Temporarily take panel lists to build a local overlay drawer without borrowing self
            let left = RefCell::new(std::mem::take(&mut self.left_side_panels));
            let right = RefCell::new(std::mem::take(&mut self.right_side_panels));
            let bottom = RefCell::new(std::mem::take(&mut self.bottom_panels));
            let detached = RefCell::new(std::mem::take(&mut self.detached_panels));
            let empty = RefCell::new(std::mem::take(&mut self.empty_panels));

            let mut draw_overlays =
                |plot_ui: &mut egui_plot::PlotUi,
                 scope: &crate::data::scope::ScopeData,
                 traces: &crate::data::traces::TracesCollection| {
                    for p in right
                        .borrow_mut()
                        .iter_mut()
                        .chain(left.borrow_mut().iter_mut())
                        .chain(bottom.borrow_mut().iter_mut())
                        .chain(detached.borrow_mut().iter_mut())
                        .chain(empty.borrow_mut().iter_mut())
                    {
                        p.draw(plot_ui, scope, traces);
                    }
                };

            // Render the liveplot panel; `draw_overlays` supplies per-panel overlays.
            self.liveplot_panel
                .render_panel(ui, &mut draw_overlays, &mut self.traces_data);

            // Return panel lists back to self
            self.left_side_panels = left.into_inner();
            self.right_side_panels = right.into_inner();
            self.bottom_panels = bottom.into_inner();
            self.detached_panels = detached.into_inner();
            self.empty_panels = empty.into_inner();

            self.traces_data.hover_trace = None;
        });
    }

    /// Update and render the panel when embedded in a parent app, and also apply controllers.
    pub fn update_embedded(&mut self, ui: &mut egui::Ui) {
        self.update(ui);
        self.apply_controllers_embedded(ui.ctx());
    }

    fn update_data(&mut self) {
        // Process incoming plot commands; collect any newly created traces.
        let new_traces = self.traces_data.update();

        // Apply any queued threshold add/remove requests before processing data so new defs
        // participate in this frame's evaluation.
        self.apply_threshold_controller_requests();

        self.liveplot_panel.update_data(&self.traces_data);
        let data = &mut LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
            pending_requests: &mut self.pending_requests,
        };

        // Attach newly created traces to the primary (first) scope only.
        if let Some(scope) = data.primary_scope_mut() {
            for name in new_traces {
                if !scope.trace_order.iter().any(|n| n == &name) {
                    scope.trace_order.push(name);
                }
            }
        }

        for p in &mut self.left_side_panels {
            p.update_data(data);
        }
        for p in &mut self.right_side_panels {
            p.update_data(data);
        }
        for p in &mut self.bottom_panels {
            p.update_data(data);
        }
        for p in &mut self.detached_panels {
            p.update_data(data);
        }
        for p in &mut self.empty_panels {
            p.update_data(data);
        }

        // After threshold processing, forward freshly generated events to controller listeners.
        self.publish_threshold_events();
    }

    /// Apply controller requests and publish state, for embedded usage (no stand-alone window frame).
    pub fn apply_controllers_embedded(&mut self, ctx: &egui::Context) {
        // WindowController: publish current viewport info; apply requested size if any
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

        self.apply_threshold_controller_requests();
        self.publish_threshold_events();

        // UiActionController: pause/resume, screenshot, export
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

            let mut data = LivePlotData {
                scope_data: self.liveplot_panel.get_data_mut(),
                traces: &mut self.traces_data,
                pending_requests: &mut self.pending_requests,
            };
            let primary_scope_id = data.primary_scope().map(|s| s.id);

            if let Some(p) = take_actions.0 {
                if p {
                    data.pause_all();
                } else {
                    data.resume_all();
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
                if let Some(scope_id) = primary_scope_id {
                    let tol = 1e-9;
                    let order = data
                        .primary_scope()
                        .map(|s| s.trace_order.clone())
                        .unwrap_or_default();
                    let series = order
                        .iter()
                        .filter_map(|name| {
                            data.get_drawn_points(name, scope_id)
                                .map(|v| (name.clone(), v.into_iter().collect()))
                        })
                        .collect();
                    let _ = if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                        export::write_csv_aligned_path(&path, &order, &series, tol)
                    } else {
                        export::write_parquet_aligned_path(&path, &order, &series, tol)
                    };
                }
            }
            if let Some(_req) = take_actions.5.take() {
                // Placeholder for FFT data requests in embedded mode
            }
        }

        // TracesController: apply queued changes and publish snapshot info
        if let Some(ctrl) = self.traces_ctrl.clone() {
            let (show_request, detached_request) = {
                let mut inner = ctrl.inner.lock().unwrap();

                let show_request = inner.show_request.take();
                let detached_request = inner.detached_request.take();

                let mut data = LivePlotData {
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
                };
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
                                is_math: false,
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
                if let Some(tp) = self.traces_panel_mut() {
                    tp.state.visible = show;
                }
            }
            if let Some(detached) = detached_request {
                if let Some(tp) = self.traces_panel_mut() {
                    tp.state.detached = detached;
                    if detached {
                        tp.state.visible = true;
                    }
                }
            }

            let mut trace_states: Vec<crate::controllers::TraceControlState> = Vec::new();
            for (name, tr) in self.traces_data.traces_iter() {
                trace_states.push(crate::controllers::TraceControlState {
                    name: name.clone(),
                    color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                    width: tr.look.width,
                    style: tr.look.style,
                    visible: tr.look.visible,
                    offset: tr.offset,
                    is_math: false,
                });
            }
            let (panel_show, panel_detached) = {
                let mut show = true;
                let mut detached = false;
                if let Some(tp) = self.traces_panel_mut() {
                    show = tp.state.visible;
                    detached = tp.state.detached;
                }
                (show, detached)
            };
            let panel_state = crate::controllers::TracesPanelState {
                max_points: self.traces_data.max_points,
                points_bounds: self.traces_data.points_bounds,
                hover_trace: self.traces_data.hover_trace.clone(),
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

        // ScopesController: apply requests and publish state
        if let Some(ctrl) = self.scopes_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            if requests.add_scope {
                self.liveplot_panel.add_scope();
            }
            if let Some(id) = requests.remove_scope {
                let _ = self.liveplot_panel.remove_scope_by_id(id);
            }
            if requests.save_screenshot {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if !requests.set_scopes.is_empty() {
                let traces = &mut self.traces_data;
                for scope_req in requests.set_scopes {
                    let mut scopes = self.liveplot_panel.get_data_mut();
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
                let scopes = self.liveplot_panel.get_data_mut();
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

        // LiveplotController: apply requests and publish state
        if let Some(ctrl) = self.liveplot_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            {
                let mut data = LivePlotData {
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
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
                    self.liveplot_panel.add_scope();
                }
                if let Some(id) = requests.remove_scope {
                    let _ = self.liveplot_panel.remove_scope_by_id(id);
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
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
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

        // FFT controller: publish basic info
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

        // ScopesController: apply requests and publish state
        if let Some(ctrl) = self.scopes_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            if requests.add_scope {
                self.liveplot_panel.add_scope();
            }
            if let Some(id) = requests.remove_scope {
                let _ = self.liveplot_panel.remove_scope_by_id(id);
            }
            if requests.save_screenshot {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if !requests.set_scopes.is_empty() {
                let traces = &mut self.traces_data;
                for scope_req in requests.set_scopes {
                    let mut scopes = self.liveplot_panel.get_data_mut();
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
                let scopes = self.liveplot_panel.get_data_mut();
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
            inner
                .listeners
                .retain(|s| s.send(scopes_state.clone()).is_ok());
        }

        // LiveplotController: apply requests and publish state
        if let Some(ctrl) = self.liveplot_ctrl.clone() {
            let requests = {
                let mut inner = ctrl.inner.lock().unwrap();
                std::mem::take(&mut inner.requests)
            };

            {
                let mut data = LivePlotData {
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
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
                    self.liveplot_panel.add_scope();
                }
                if let Some(id) = requests.remove_scope {
                    let _ = self.liveplot_panel.remove_scope_by_id(id);
                }
                // Reorder not yet supported in liveplot panel; consume request.
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
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
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
            inner
                .listeners
                .retain(|s| s.send(liveplot_state.clone()).is_ok());
        }
    }

    fn thresholds_panel_mut(&mut self) -> Option<&mut ThresholdsPanel> {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if let Some(tp) = p.downcast_mut::<ThresholdsPanel>() {
                return Some(tp);
            }
        }
        None
    }

    fn traces_panel_mut(&mut self) -> Option<&mut TracesPanel> {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if let Some(tp) = p.downcast_mut::<TracesPanel>() {
                return Some(tp);
            }
        }
        None
    }

    fn apply_threshold_controller_requests(&mut self) {
        let Some(ctrl) = self.threshold_ctrl.clone() else {
            return;
        };

        let (adds, removes) = {
            let mut inner = ctrl.inner.lock().unwrap();
            (
                inner.add_requests.drain(..).collect::<Vec<_>>(),
                inner.remove_requests.drain(..).collect::<Vec<_>>(),
            )
        };

        if adds.is_empty() && removes.is_empty() {
            return;
        }
        if let Some(tp) = self.thresholds_panel_mut() {
            let mut added_names: Vec<String> = Vec::new();
            for name in &removes {
                tp.thresholds.remove(name);
            }
            for def in adds {
                added_names.push(def.name.clone());
                tp.thresholds.insert(def.name.clone(), def);
            }

            for name in removes {
                self.threshold_event_cursors.remove(&name);
            }
            for name in added_names {
                self.threshold_event_cursors.entry(name).or_insert(0);
            }
        }
    }

    fn publish_threshold_events(&mut self) {
        let Some(ctrl) = self.threshold_ctrl.clone() else {
            return;
        };

        let mut pending: Vec<crate::data::thresholds::ThresholdEvent> = Vec::new();
        let mut collected: Vec<(String, Vec<crate::data::thresholds::ThresholdEvent>)> = Vec::new();

        if let Some(tp) = self.thresholds_panel_mut() {
            for (name, def) in tp.thresholds.iter() {
                let events: Vec<crate::data::thresholds::ThresholdEvent> =
                    def.get_runtime_state().events.iter().cloned().collect();
                collected.push((name.clone(), events));
            }
        }

        // Drop cursors for thresholds no longer present (e.g., removed via UI)
        let present: HashMap<_, _> = collected
            .iter()
            .map(|(n, evts)| (n.clone(), evts.len()))
            .collect();
        self.threshold_event_cursors
            .retain(|name, _| present.contains_key(name));

        for (name, events) in collected {
            let prev = self
                .threshold_event_cursors
                .get(&name)
                .copied()
                .unwrap_or(0);
            let len = events.len();
            if len < prev {
                self.threshold_event_cursors.insert(name.clone(), len);
                continue;
            }
            if len > prev {
                pending.extend(events.into_iter().skip(prev));
                self.threshold_event_cursors.insert(name.clone(), len);
            }
        }

        if pending.is_empty() {
            return;
        }

        let mut inner = ctrl.inner.lock().unwrap();
        inner.listeners.retain(|s| {
            for ev in &pending {
                if s.send(ev.clone()).is_err() {
                    return false;
                }
            }
            true
        });
    }

    pub(crate) fn toggle_panel_visibility<T: 'static + Panel>(&mut self) -> bool {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if p.downcast_ref::<T>().is_some() {
                let st = p.state_mut();
                let currently_shown = st.visible && !st.detached;
                st.visible = !currently_shown;
                st.detached = false;
                return true;
            }
        }
        false
    }

    /// Hide the Hotkeys panel (useful when focus switches away via hotkeys)
    pub fn hide_hotkeys_panel(&mut self) {
        for p in self
            .left_side_panels
            .iter_mut()
            .chain(self.right_side_panels.iter_mut())
            .chain(self.bottom_panels.iter_mut())
            .chain(self.detached_panels.iter_mut())
            .chain(self.empty_panels.iter_mut())
        {
            if p.downcast_ref::<HotkeysPanel>().is_some() {
                p.state_mut().visible = false;
            }
        }
    }

    /// Describes which buttons appear in the top bar and sidebar for this frame.
    fn compute_effective_layout(&self) -> EffectiveLayout {
        let plot_h = self.last_plot_size.y;
        let plot_w = self.last_plot_size.x;
        let suppress_top = plot_h < self.min_height_for_top_bar;
        let suppress_sidebar =
            plot_w < self.min_width_for_sidebar || plot_h < self.min_height_for_sidebar;

        let user_top: Vec<ScopeButton> = self
            .top_bar_buttons
            .clone()
            .unwrap_or_else(ScopeButton::all_defaults);
        let user_sidebar: Vec<ScopeButton> = self.sidebar_buttons.clone().unwrap_or_default();

        if suppress_top && suppress_sidebar {
            EffectiveLayout {
                top_bar_buttons: vec![],
                sidebar_buttons: vec![],
                show_top_bar: false,
                show_sidebar_panels: false,
            }
        } else if suppress_top {
            // Top bar is hidden → move its buttons into the sidebar (after sidebar buttons).
            let mut sidebar = user_sidebar;
            sidebar.extend(user_top);
            EffectiveLayout {
                top_bar_buttons: vec![],
                sidebar_buttons: sidebar,
                show_top_bar: false,
                show_sidebar_panels: true,
            }
        } else if suppress_sidebar {
            // Sidebar is hidden → move its icon-strip buttons into the top bar (after top buttons).
            let mut top = user_top;
            top.extend(user_sidebar);
            EffectiveLayout {
                top_bar_buttons: top,
                sidebar_buttons: vec![],
                show_top_bar: true,
                show_sidebar_panels: false,
            }
        } else {
            EffectiveLayout {
                top_bar_buttons: user_top,
                sidebar_buttons: user_sidebar,
                show_top_bar: true,
                show_sidebar_panels: true,
            }
        }
    }

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        // ── Responsive layout: should we show the top bar at all? ───────────
        let layout = self.compute_effective_layout();
        if !layout.show_top_bar {
            return; // top bar suppressed – its buttons have been moved to the sidebar
        }
        let top_bar_btns = layout.top_bar_buttons;

        // ── When to collapse text labels to icon-only ────────────────────────
        // Compute whether top-bar buttons should collapse to icon-only.
        // We calculate the full width required to display all buttons with text labels.
        // If the available width is less than this, we switch to icon-only mode.
        let button_font = egui::TextStyle::Button.resolve(ui.style());
        let button_padding = ui.spacing().button_padding.x * 2.0;
        let item_spacing = ui.spacing().item_spacing.x;
        let mut required_width = 0.0;

        let calc_width = |text: &str| -> f32 {
            let w = ui.fonts_mut(|f| {
                f.layout_no_wrap(text.to_string(), button_font.clone(), egui::Color32::WHITE)
                    .rect
                    .width()
            });
            w + button_padding + item_spacing
        };

        // 1. Scopes button (only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::Scopes) {
            required_width += calc_width("🔭 Scopes");
        }

        // 2. All other panels (only those in top_bar_btns)
        let all_panels = self
            .left_side_panels
            .iter()
            .chain(self.right_side_panels.iter())
            .chain(self.bottom_panels.iter())
            .chain(self.detached_panels.iter())
            .chain(self.empty_panels.iter());

        for p in all_panels {
            if top_bar_btns
                .iter()
                .any(|b| b.matches_panel_title(p.title()))
            {
                required_width += calc_width(&p.title_and_icon());
            }
        }

        // 3. Separator (approximate width)
        required_width += item_spacing * 2.0; // visuals usually take some space

        // 4. Pause / Resume (take the wider one, only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::PauseResume) {
            required_width += calc_width("⏸ Pause");
        }

        // 5. Clear All (only if in top_bar_btns)
        if top_bar_btns.contains(&ScopeButton::ClearAll) {
            required_width += calc_width(&format!("{BROOM} Clear All"));
        }

        // Remove trailing spacing
        required_width -= item_spacing;

        let topbar_collapsed = should_collapse_topbar(ui.available_width(), required_width);

        // Clone Rc so it can be borrowed independently inside the closure.
        let hk_rc = self.hotkeys.clone();

        egui::MenuBar::new().ui(ui, |ui| {
            // Render the Scopes button only if configured
            if top_bar_btns.contains(&ScopeButton::Scopes) {
                self.liveplot_panel
                    .render_menu(ui, &mut self.traces_data, topbar_collapsed);
            }

            let (save_req, load_req, add_scope_req, remove_scope_req) = {
                let scope_data = self.liveplot_panel.get_data_mut();
                let mut data = LivePlotData {
                    scope_data,
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
                };

                {
                    let hk = hk_rc.borrow();
                    for p in &mut self.left_side_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.right_side_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.bottom_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.detached_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }
                    for p in &mut self.empty_panels {
                        if !top_bar_btns
                            .iter()
                            .any(|b| b.matches_panel_title(p.title()))
                        {
                            continue;
                        }
                        let tt = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name))
                            .map(|k| format_button_tooltip(p.title(), Some(k)))
                            .unwrap_or_else(|| p.title().to_string());
                        p.render_menu(ui, &mut data, topbar_collapsed, &tt);
                    }

                    // Pause / Resume button (only if configured)
                    if top_bar_btns.contains(&ScopeButton::PauseResume)
                        || top_bar_btns.contains(&ScopeButton::ClearAll)
                    {
                        ui.separator();
                    }
                    if top_bar_btns.contains(&ScopeButton::PauseResume) {
                        let pause_tt = format_button_tooltip("Pause / Resume", hk.pause.as_ref());
                        if !data.are_all_paused() {
                            let pause_label = if topbar_collapsed { "⏸" } else { "⏸ Pause" };
                            if ui.button(pause_label).on_hover_text(&pause_tt).clicked() {
                                data.pause_all();
                            }
                        } else {
                            let resume_label = if topbar_collapsed {
                                "▶"
                            } else {
                                "▶ Resume"
                            };
                            if ui.button(resume_label).on_hover_text(&pause_tt).clicked() {
                                data.resume_all();
                            }
                        }
                    }

                    if top_bar_btns.contains(&ScopeButton::ClearAll) {
                        let clear_all_label = if topbar_collapsed {
                            BROOM.to_string()
                        } else {
                            format!("{BROOM} Clear All")
                        };
                        let clear_tt = format_button_tooltip("Clear All", hk.clear_all.as_ref());
                        if ui.button(clear_all_label).on_hover_text(clear_tt).clicked() {
                            data.request_clear_all();
                        }
                    }
                }

                (
                    data.pending_requests.save_state.take(),
                    data.pending_requests.load_state.take(),
                    std::mem::take(&mut data.pending_requests.add_scope),
                    data.pending_requests.remove_scope.take(),
                )
            };

            if add_scope_req {
                self.liveplot_panel.add_scope();
            }
            if let Some(scope_id) = remove_scope_req {
                let _ = self.liveplot_panel.remove_scope_by_id(scope_id);
            }

            if let Some(path) = save_req {
                // Save state: build a serializable AppStateSerde and write it
                let ctx = ui.ctx();
                let rect = ctx.input(|i| i.content_rect());
                let win_size = Some([rect.width(), rect.height()]);
                let win_pos = Some([rect.left(), rect.top()]);
                let live_data = LivePlotData {
                    scope_data: self.liveplot_panel.get_data_mut(),
                    traces: &mut self.traces_data,
                    pending_requests: &mut self.pending_requests,
                };

                // Save all scopes
                let scope_states: Vec<crate::persistence::ScopeStateSerde> = live_data
                    .scope_data
                    .iter()
                    .map(|s| crate::persistence::ScopeStateSerde::from(&**s))
                    .collect();

                // Helper to convert Panel::state() to PanelVisSerde
                let mut panels_state: Vec<crate::persistence::PanelVisSerde> = Vec::new();
                let mut push_panel = |p: &Box<dyn Panel>| {
                    let st = p.state();
                    panels_state.push(crate::persistence::PanelVisSerde {
                        title: st.title.to_string(),
                        visible: st.visible,
                        detached: st.detached,
                        window_pos: st.window_pos,
                        window_size: st.window_size,
                    });
                };
                for p in &self.left_side_panels {
                    push_panel(p);
                }
                for p in &self.right_side_panels {
                    push_panel(p);
                }
                for p in &self.bottom_panels {
                    push_panel(p);
                }
                for p in &self.detached_panels {
                    push_panel(p);
                }
                for p in &self.empty_panels {
                    push_panel(p);
                }

                // Trace styles from all scopes
                let trace_styles: Vec<crate::persistence::TraceStyleSerde> = {
                    let mut seen = std::collections::HashSet::new();
                    let mut snapshot: Vec<(String, crate::data::trace_look::TraceLook, f64)> =
                        Vec::new();
                    for scope in live_data.scope_data.iter() {
                        for name in scope.trace_order.iter() {
                            if seen.insert(name.0.clone()) {
                                if let Some(tr) = live_data.traces.get_trace(name) {
                                    snapshot.push((name.0.clone(), tr.look.clone(), tr.offset));
                                }
                            }
                        }
                    }
                    snapshot
                        .into_iter()
                        .map(|(n, look, off)| crate::persistence::TraceStyleSerde {
                            name: n,
                            look: crate::persistence::TraceLookSerde::from(&look),
                            offset: off,
                        })
                        .collect()
                };

                // Math traces: extract from MathPanel
                let math_traces_ser: Vec<crate::data::math::MathTrace> = {
                    let mut out = Vec::new();
                    for p in self
                        .left_side_panels
                        .iter()
                        .chain(self.right_side_panels.iter())
                        .chain(self.bottom_panels.iter())
                        .chain(self.detached_panels.iter())
                        .chain(self.empty_panels.iter())
                    {
                        let any: &dyn Panel = &**p;
                        if let Some(mp) = any.downcast_ref::<crate::panels::math_ui::MathPanel>() {
                            out.extend(mp.get_math_traces().iter().cloned());
                        }
                    }
                    out
                };

                // Thresholds & Triggers: extract from specialized panels, if present
                let mut thresholds_ser: Vec<crate::persistence::ThresholdSerde> = Vec::new();
                let mut triggers_ser: Vec<crate::persistence::TriggerSerde> = Vec::new();
                for p in self
                    .left_side_panels
                    .iter()
                    .chain(self.right_side_panels.iter())
                    .chain(self.bottom_panels.iter())
                    .chain(self.detached_panels.iter())
                    .chain(self.empty_panels.iter())
                {
                    let any: &dyn Panel = &**p;
                    if let Some(tp) =
                        any.downcast_ref::<crate::panels::thresholds_ui::ThresholdsPanel>()
                    {
                        for (_n, d) in tp.thresholds.iter() {
                            thresholds_ser
                                .push(crate::persistence::ThresholdSerde::from_threshold(d));
                        }
                    }
                    if let Some(trg) =
                        any.downcast_ref::<crate::panels::triggers_ui::TriggersPanel>()
                    {
                        for (_n, t) in trg.triggers.iter() {
                            triggers_ser.push(crate::persistence::TriggerSerde::from_trigger(t));
                        }
                    }
                }

                let state = crate::persistence::AppStateSerde {
                    window_size: win_size,
                    window_pos: win_pos,
                    scope: None,
                    scopes: scope_states,
                    panels: panels_state,
                    traces_style: trace_styles,
                    thresholds: thresholds_ser,
                    triggers: triggers_ser,
                    math_traces: math_traces_ser,
                    next_scope_idx: Some(self.liveplot_panel.next_scope_idx()),
                };

                let _ = crate::persistence::save_state_to_path(&state, &path);
            }

            if let Some(path) = load_req {
                if let Ok(loaded) = crate::persistence::load_state_from_path(&path) {
                    // Window: attempt to request size/pos via ctx
                    if let Some(sz) = loaded.window_size {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(
                            egui::Vec2::new(sz[0], sz[1]),
                        ));
                    }

                    // Restore all scopes (or fall back to legacy single-scope)
                    let scope_states = loaded.all_scopes();
                    if !scope_states.is_empty() {
                        self.liveplot_panel
                            .restore_scopes(scope_states, loaded.next_scope_idx);
                    }

                    // Panels: match by title and set visible/detached/pos/size
                    let apply_panel_state = |p: &mut Box<dyn Panel>| {
                        let st = p.state_mut();
                        for pser in &loaded.panels {
                            if pser.title == st.title {
                                st.visible = pser.visible;
                                st.detached = pser.detached;
                                st.window_pos = pser.window_pos;
                                st.window_size = pser.window_size;
                                break;
                            }
                        }
                    };
                    for p in &mut self.left_side_panels {
                        apply_panel_state(p);
                    }
                    for p in &mut self.right_side_panels {
                        apply_panel_state(p);
                    }
                    for p in &mut self.bottom_panels {
                        apply_panel_state(p);
                    }
                    for p in &mut self.detached_panels {
                        apply_panel_state(p);
                    }
                    for p in &mut self.empty_panels {
                        apply_panel_state(p);
                    }

                    // Apply traces styles (uses pending_styles for traces not yet created)
                    {
                        crate::persistence::apply_trace_styles(
                            &loaded.traces_style,
                            |name, look, off| {
                                self.traces_data.set_pending_style(name, look, off);
                            },
                        );
                    }

                    // Apply math traces
                    if !loaded.math_traces.is_empty() {
                        for p in self
                            .left_side_panels
                            .iter_mut()
                            .chain(self.right_side_panels.iter_mut())
                            .chain(self.bottom_panels.iter_mut())
                            .chain(self.detached_panels.iter_mut())
                            .chain(self.empty_panels.iter_mut())
                        {
                            let any: &mut dyn Panel = &mut **p;
                            if let Some(mp) =
                                any.downcast_mut::<crate::panels::math_ui::MathPanel>()
                            {
                                mp.set_math_traces(loaded.math_traces.clone());
                            }
                        }
                    }

                    // Apply thresholds and triggers to specialized panels
                    for p in self
                        .left_side_panels
                        .iter_mut()
                        .chain(self.right_side_panels.iter_mut())
                        .chain(self.bottom_panels.iter_mut())
                        .chain(self.detached_panels.iter_mut())
                        .chain(self.empty_panels.iter_mut())
                    {
                        let any: &mut dyn Panel = &mut **p;
                        if let Some(tp) =
                            any.downcast_mut::<crate::panels::thresholds_ui::ThresholdsPanel>()
                        {
                            tp.thresholds.clear();
                            for tser in &loaded.thresholds {
                                let def = tser.clone().into_threshold();
                                tp.thresholds.insert(def.name.clone(), def);
                            }
                        }
                        if let Some(trg) =
                            any.downcast_mut::<crate::panels::triggers_ui::TriggersPanel>()
                        {
                            trg.triggers.clear();
                            for trser in &loaded.triggers {
                                let def = trser.clone().into_trigger();
                                trg.triggers.insert(def.name.clone(), def);
                            }
                        }
                    }
                }
            }
        });
    }

    fn render_panels(&mut self, ui: &mut egui::Ui) {
        let layout = self.compute_effective_layout();
        let has_icon_strip = !layout.sidebar_buttons.is_empty();

        // ── Persistent sidebar icon strip (rightmost panel) ──────────────────
        // Rendered first so it occupies the far-right slot; the panel-content
        // sidebar renders to its left.
        if has_icon_strip {
            let sidebar_btns = layout.sidebar_buttons.clone();
            let hk_rc = self.hotkeys.clone();
            // Snapshot pause state before the closure so the closure only needs a copy.
            let all_paused = self.liveplot_panel.get_data().iter().all(|s| s.paused);
            let mut clicked_btns: Vec<ScopeButton> = Vec::new();

            egui::SidePanel::right(format!("right_icon_strip_{}", self.panel_id))
                .resizable(false)
                .exact_width(36.0)
                .show_inside(ui, |ui| {
                    let hk = hk_rc.borrow();
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        for btn in &sidebar_btns {
                            match btn {
                                ScopeButton::PauseResume => {
                                    let (icon, tooltip) = if all_paused {
                                        ("▶", "Resume")
                                    } else {
                                        ("⏸", "Pause")
                                    };
                                    if ui.button(icon).on_hover_text(tooltip).clicked() {
                                        clicked_btns.push(ScopeButton::PauseResume);
                                    }
                                }
                                ScopeButton::ClearAll => {
                                    let tt =
                                        format_button_tooltip("Clear All", hk.clear_all.as_ref());
                                    if ui.button(BROOM.to_string()).on_hover_text(tt).clicked() {
                                        clicked_btns.push(ScopeButton::ClearAll);
                                    }
                                }
                                ScopeButton::Scopes => {
                                    ui.button("🔭").on_hover_text("Scopes (use the top bar)");
                                }
                                other => {
                                    // Find panel info across all lists (immutable borrows only).
                                    let panel_info: Option<(bool, String, String)> = {
                                        let all = self
                                            .left_side_panels
                                            .iter()
                                            .chain(self.right_side_panels.iter())
                                            .chain(self.bottom_panels.iter())
                                            .chain(self.detached_panels.iter())
                                            .chain(self.empty_panels.iter());
                                        let mut found = None;
                                        for p in all {
                                            if other.matches_panel_title(p.title()) {
                                                let active =
                                                    p.state().visible && !p.state().detached;
                                                let icon =
                                                    p.icon_only().unwrap_or(p.title()).to_string();
                                                let hk_str = p
                                                    .hotkey_name()
                                                    .and_then(|n| get_hotkey_for_name(&hk, n));
                                                let tt = format_button_tooltip(p.title(), hk_str);
                                                found = Some((active, icon, tt));
                                                break;
                                            }
                                        }
                                        found
                                    };
                                    if let Some((active, icon, tt)) = panel_info {
                                        if ui
                                            .selectable_label(active, icon)
                                            .on_hover_text(tt)
                                            .clicked()
                                        {
                                            clicked_btns.push(other.clone());
                                        }
                                    }
                                }
                            }
                        }
                    });
                });

            // Apply icon-strip actions now that the closure (and its borrows) is done.
            for btn in clicked_btns {
                match btn {
                    ScopeButton::PauseResume => {
                        if all_paused {
                            for s in self.liveplot_panel.get_data_mut() {
                                s.paused = false;
                            }
                        } else {
                            for s in self.liveplot_panel.get_data_mut() {
                                s.paused = true;
                            }
                            self.traces_data.take_snapshot();
                        }
                    }
                    ScopeButton::ClearAll => {
                        self.traces_data.clear_all();
                        for s in self.liveplot_panel.get_data_mut() {
                            s.clicked_point = None;
                        }
                    }
                    other => {
                        // Toggle the matching panel.
                        for p in self
                            .left_side_panels
                            .iter_mut()
                            .chain(self.right_side_panels.iter_mut())
                            .chain(self.bottom_panels.iter_mut())
                            .chain(self.detached_panels.iter_mut())
                            .chain(self.empty_panels.iter_mut())
                        {
                            if other.matches_panel_title(p.title()) {
                                let st = p.state_mut();
                                let is_shown = st.visible && !st.detached;
                                st.visible = !is_shown;
                                st.detached = false;
                                if !is_shown {
                                    st.request_focus = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        // ── Sidebar panel content (left / right / bottom) ─────────────────────
        // When show_sidebar_panels is false (responsive suppression), we skip rendering
        // attached panel content but detached windows are still shown below.
        if layout.show_sidebar_panels {
            // Layout: left, right side optional; bottom optional; main center
            let show_left = !self.left_side_panels.is_empty()
                && self
                    .left_side_panels
                    .iter()
                    .any(|p| p.state().visible && !p.state().detached);
            let show_right = !self.right_side_panels.is_empty()
                && self
                    .right_side_panels
                    .iter()
                    .any(|p| p.state().visible && !p.state().detached);
            let show_bottom = !self.bottom_panels.is_empty()
                && self
                    .bottom_panels
                    .iter()
                    .any(|p| p.state().visible && !p.state().detached);

            if show_left {
                let mut list = std::mem::take(&mut self.left_side_panels);
                egui::SidePanel::left(format!("left_sidebar_{}", self.panel_id))
                    .resizable(true)
                    .default_width(280.0)
                    .min_width(160.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                            .scroll_source(ScrollSource::NONE)
                            .show(ui, |ui| {
                                self.render_tabs(ui, &mut list);
                            });
                    });
                self.left_side_panels = list;
            } else if !self.left_side_panels.is_empty() {
                let mut list = std::mem::take(&mut self.left_side_panels);
                let hk_rc_left = self.hotkeys.clone();
                egui::SidePanel::left(format!("left_sidebar_{}", self.panel_id))
                    .resizable(true)
                    .default_width(30.0)
                    .min_width(30.0)
                    .show_inside(ui, |ui| {
                        let hk = hk_rc_left.borrow();
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                            .scroll_source(ScrollSource::NONE)
                            .show(ui, |ui| {
                                let mut clicked: Option<usize> = None;
                                ui.vertical(|ui| {
                                    for (i, p) in list.iter_mut().enumerate() {
                                        let active = p.state().visible && !p.state().detached;
                                        let label = p.icon_only().unwrap_or(p.title()).to_string();
                                        let hotkey = p
                                            .hotkey_name()
                                            .and_then(|name| get_hotkey_for_name(&hk, name));
                                        let tooltip = format_button_tooltip(p.title(), hotkey);
                                        if ui
                                            .selectable_label(active, label)
                                            .on_hover_text(tooltip)
                                            .clicked()
                                        {
                                            clicked = Some(i);
                                        }
                                    }
                                });
                                if let Some(ci) = clicked {
                                    for (i, p) in list.iter_mut().enumerate() {
                                        if i == ci {
                                            p.state_mut().visible = true;
                                            p.state_mut().request_focus = true;
                                        } else if !p.state().detached {
                                            p.state_mut().visible = false;
                                        }
                                    }
                                }
                            });
                    });
                self.left_side_panels = list;
            }

            if show_right {
                let mut list = std::mem::take(&mut self.right_side_panels);
                egui::SidePanel::right(format!("right_sidebar_{}", self.panel_id))
                    .resizable(true)
                    .default_width(320.0)
                    .min_width(200.0)
                    .show_inside(ui, |ui| {
                        self.render_tabs(ui, &mut list);
                    });
                self.right_side_panels = list;
            } else if !self.right_side_panels.is_empty() && !has_icon_strip {
                // Only show the collapsed icon strip when there is no persistent
                // icon strip (which already provides this navigation).
                let mut list = std::mem::take(&mut self.right_side_panels);
                let hk_rc_right = self.hotkeys.clone();
                egui::SidePanel::right(format!("right_sidebar_{}", self.panel_id))
                    .resizable(true)
                    .default_width(30.0)
                    .min_width(30.0)
                    .show_inside(ui, |ui| {
                        let hk = hk_rc_right.borrow();
                        let mut clicked: Option<usize> = None;
                        ui.vertical(|ui| {
                            for (i, p) in list.iter_mut().enumerate() {
                                let active = p.state().visible && !p.state().detached;
                                let label = p.icon_only().unwrap_or(p.title()).to_string();
                                let hotkey = p
                                    .hotkey_name()
                                    .and_then(|name| get_hotkey_for_name(&hk, name));
                                let tooltip = format_button_tooltip(p.title(), hotkey);
                                if ui
                                    .selectable_label(active, label)
                                    .on_hover_text(tooltip)
                                    .clicked()
                                {
                                    clicked = Some(i);
                                }
                            }
                        });
                        if let Some(ci) = clicked {
                            for (i, p) in list.iter_mut().enumerate() {
                                if i == ci {
                                    p.state_mut().visible = true;
                                    p.state_mut().request_focus = true;
                                } else if !p.state().detached {
                                    p.state_mut().visible = false;
                                }
                            }
                        }
                    });
                self.right_side_panels = list;
            }

            if show_bottom {
                let mut list = std::mem::take(&mut self.bottom_panels);
                egui::TopBottomPanel::bottom(format!("bottom_bar_{}", self.panel_id))
                    .resizable(true)
                    .default_height(220.0)
                    .min_height(120.0)
                    .show_inside(ui, |ui| {
                        self.render_tabs(ui, &mut list);
                    });
                self.bottom_panels = list;
            } else if !self.bottom_panels.is_empty() {
                let mut list = std::mem::take(&mut self.bottom_panels);
                let hk_rc_bottom = self.hotkeys.clone();
                egui::TopBottomPanel::bottom(format!("bottom_bar_{}", self.panel_id))
                    .resizable(false)
                    .default_height(24.0)
                    .min_height(24.0)
                    .show_inside(ui, |ui| {
                        let hk = hk_rc_bottom.borrow();
                        let mut clicked: Option<usize> = None;
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            for (i, p) in list.iter_mut().enumerate() {
                                let label = p.title_and_icon();
                                let hotkey = p
                                    .hotkey_name()
                                    .and_then(|name| get_hotkey_for_name(&hk, name));
                                let tooltip = format_button_tooltip(p.title(), hotkey);
                                if ui.button(label).on_hover_text(tooltip).clicked() {
                                    clicked = Some(i);
                                }
                            }
                        });
                        if let Some(ci) = clicked {
                            for (i, p) in list.iter_mut().enumerate() {
                                if i == ci {
                                    p.state_mut().visible = true;
                                    p.state_mut().request_focus = true;
                                } else if !p.state().detached {
                                    p.state_mut().visible = false;
                                }
                            }
                        }
                    });
                self.bottom_panels = list;
            }
        } // end show_sidebar_panels

        // ── Detached windows (always shown regardless of responsive state) ────
        for p in &mut self.left_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                    },
                );
            }
        }

        for p in &mut self.right_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                    },
                );
            }
        }

        for p in &mut self.bottom_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                    },
                );
            }
        }

        for p in &mut self.detached_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        pending_requests: &mut self.pending_requests,
                    },
                );
            }
        }
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui, list: &mut Vec<Box<dyn Panel>>) {
        let count = list.len();

        let mut clicked: Option<usize> = None;

        // Clone the Rc so it can be borrowed inside the borrow block below.
        let hk_rc_tabs = self.hotkeys.clone();

        let (add_scope_req, remove_scope_req) = {
            let scope_data = self.liveplot_panel.get_data_mut();
            let data = &mut LivePlotData {
                scope_data,
                traces: &mut self.traces_data,
                pending_requests: &mut self.pending_requests,
            };

            if count > 0 {
                // Honor focus requests from panels (request_docket): make that panel the active attached tab
                if let Some(req_idx) = list.iter().enumerate().find_map(|(i, p)| {
                    if p.state().request_docket {
                        Some(i)
                    } else {
                        None
                    }
                }) {
                    for (j, p) in list.iter_mut().enumerate() {
                        if j == req_idx {
                            let st = p.state_mut();
                            st.visible = true;
                            st.detached = false;
                            st.request_docket = false;
                        } else if !p.state().detached {
                            p.state_mut().visible = false;
                        }
                    }
                }
                // Keep action buttons pinned in the top-right corner.
                // If space gets tight, render tabs as icon-only with a tooltip; if still tight,
                // wrap only the tabs onto additional lines (actions never wrap/move).
                let available = ui.available_width();
                let button_font = egui::TextStyle::Button.resolve(ui.style());
                let txt_width = |text: &str, ui: &egui::Ui| -> f32 {
                    ui.fonts_mut(|f| {
                        f.layout_no_wrap(text.to_owned(), button_font.clone(), egui::Color32::WHITE)
                            .rect
                            .width()
                    })
                };
                let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;

                let actions_w = txt_width("Pop out", ui) + pad + txt_width("Hide", ui) + pad;

                let full_tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => txt_width(&list[0].title_and_icon(), ui) + pad,
                    _ => list
                        .iter()
                        .map(|p| txt_width(&p.title_and_icon(), ui) + pad)
                        .sum(),
                };

                let icon_tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => {
                        let label = list[0]
                            .icon_only()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| list[0].title_and_icon());
                        txt_width(&label, ui) + pad
                    }
                    _ => list
                        .iter()
                        .map(|p| {
                            let label = p
                                .icon_only()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| p.title_and_icon());
                            txt_width(&label, ui) + pad
                        })
                        .sum(),
                };

                let use_icon_only = full_tabs_w + actions_w > available;
                let wrap_tabs = use_icon_only && (icon_tabs_w + actions_w > available);

                // Pre-compute per-panel tooltips (title + hotkey hint) once.
                let hk = hk_rc_tabs.borrow();
                let tooltips: Vec<String> = list
                    .iter()
                    .map(|p| {
                        let hotkey = p
                            .hotkey_name()
                            .and_then(|name| get_hotkey_for_name(&hk, name));
                        format_button_tooltip(p.title(), hotkey)
                    })
                    .collect();
                drop(hk);

                // Single header area:
                // - actions are always pinned top-right
                // - tabs take remaining space to the left, and can wrap beneath if needed
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    // Right: actions
                    if ui.button("Hide").clicked() {
                        for p in list.iter_mut() {
                            if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                    if ui.button("Pop out").clicked() {
                        for p in list.iter_mut() {
                            if p.state().visible && !p.state().detached {
                                p.state_mut().detached = true;
                                p.state_mut().request_docket = false;
                                p.state_mut().visible = true;
                                p.state_mut().request_focus = true;
                            }
                        }
                    }

                    // Left: tabs in remaining width
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        let render_tabs =
                            |ui: &mut egui::Ui,
                             list: &mut Vec<Box<dyn Panel>>,
                             clicked: &mut Option<usize>,
                             use_icon_only: bool,
                             count: usize,
                             tooltips: &[String]| {
                                if count > 1 {
                                    for (i, p) in list.iter_mut().enumerate() {
                                        let active = p.state().visible && !p.state().detached;
                                        let tooltip =
                                            tooltips.get(i).map(|s| s.as_str()).unwrap_or("");

                                        let label = if use_icon_only {
                                            p.icon_only()
                                                .map(|s| s.to_string())
                                                .unwrap_or_else(|| p.title_and_icon())
                                        } else {
                                            p.title_and_icon()
                                        };

                                        let mut resp = ui.selectable_label(active, label);
                                        if !tooltip.is_empty() {
                                            resp = resp.on_hover_text(tooltip);
                                        }
                                        if resp.clicked() {
                                            *clicked = Some(i);
                                        }
                                    }
                                } else {
                                    let p = &mut list[0];
                                    let tooltip =
                                        tooltips.first().map(|s| s.as_str()).unwrap_or("");
                                    let label = if use_icon_only {
                                        p.icon_only()
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| p.title_and_icon())
                                    } else {
                                        p.title_and_icon()
                                    };
                                    if !tooltip.is_empty() {
                                        ui.label(label).on_hover_text(tooltip);
                                    } else {
                                        ui.label(label);
                                    }
                                    *clicked = Some(0);
                                }
                            };

                        if wrap_tabs {
                            ui.horizontal_wrapped(|ui| {
                                render_tabs(
                                    ui,
                                    list,
                                    &mut clicked,
                                    use_icon_only,
                                    count,
                                    &tooltips,
                                );
                            });
                        } else {
                            ui.horizontal(|ui| {
                                render_tabs(
                                    ui,
                                    list,
                                    &mut clicked,
                                    use_icon_only,
                                    count,
                                    &tooltips,
                                );
                            });
                        }
                    });
                });

                // Apply clicked selection when multiple tabs are present
                if count > 1 {
                    if let Some(i) = clicked {
                        for (j, p) in list.iter_mut().enumerate() {
                            if j == i {
                                p.state_mut().visible = true;
                                p.state_mut().detached = false;
                            } else if !p.state().detached {
                                p.state_mut().visible = false;
                            }
                        }
                    }
                }
            }

            ui.separator();
            // Body: find first attached+visible panel
            if let Some((idx, _)) = list
                .iter()
                .enumerate()
                .find(|(_i, p)| p.state().visible && !p.state().detached)
            {
                let p = &mut list[idx];
                p.render_panel(ui, data);
            } else {
                ui.label("No panel active");
            }
            (
                std::mem::take(&mut data.pending_requests.add_scope),
                data.pending_requests.remove_scope.take(),
            )
        };

        // Apply any scope add/remove requests issued by the rendered panel(s)
        if add_scope_req {
            self.liveplot_panel.add_scope();
        }
        if let Some(scope_id) = remove_scope_req {
            let _ = self.liveplot_panel.remove_scope_by_id(scope_id);
        }
    }
}

pub struct MainApp {
    pub main_panel: MainPanel,
    // Optional external controllers
    pub window_ctrl: Option<WindowController>,
    pub ui_ctrl: Option<UiActionController>,
    pub traces_ctrl: Option<TracesController>,
    pub scopes_ctrl: Option<ScopesController>,
    pub liveplot_ctrl: Option<LiveplotController>,
    pub fft_ctrl: Option<FFTController>,
    pub threshold_ctrl: Option<ThresholdController>,
    pub headline: Option<String>,
    pub subheadline: Option<String>,
}

impl MainApp {
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        Self {
            main_panel: MainPanel::new(rx),
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            scopes_ctrl: None,
            liveplot_ctrl: None,
            fft_ctrl: None,
            threshold_ctrl: None,
            headline: None,
            subheadline: None,
        }
    }

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
        let mut main_panel = MainPanel::new(rx);
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
        }
    }

    fn apply_config(&mut self, cfg: &crate::config::LivePlotConfig) {
        // Axis/time window settings
        {
            let scope = self.main_panel.liveplot_panel.get_data_mut();
            for s in scope {
                s.time_window = cfg.time_window_secs;
                s.y_axis.set_unit(cfg.y_unit.clone());
                s.y_axis.log_scale = cfg.y_log;
                // Set X axis to a time axis using default time format (per-scope formatting chooses date/time based on bounds)
                s.x_axis.axis_type =
                    crate::data::scope::AxisType::Time(crate::data::scope::XDateFormat::default());
                // Apply the configured X formatter (Auto selects TimeFormatter for time axes)
                s.x_axis.x_formatter = cfg.x_formatter.clone();
                s.show_legend = cfg.show_legend;
            }
        }

        // Trace storage limits
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

        // Headline/subheadline for optional top banner
        self.headline = cfg.headline.clone();
        self.subheadline = cfg.subheadline.clone();

        // ── Responsive layout configuration ──────────────────────────────────
        self.main_panel.top_bar_buttons = cfg.top_bar_buttons.clone();
        self.main_panel.sidebar_buttons = cfg.sidebar_buttons.clone();
        self.main_panel.min_height_for_top_bar = cfg.min_height_for_top_bar;
        self.main_panel.min_width_for_sidebar = cfg.min_width_for_sidebar;
        self.main_panel.min_height_for_sidebar = cfg.min_height_for_sidebar;

        // ── Tick-label visibility thresholds (applied to every scope panel) ──
        self.main_panel.liveplot_panel.set_tick_label_thresholds(
            cfg.min_width_for_y_ticklabels,
            cfg.min_height_for_x_ticklabels,
        );
        self.main_panel
            .liveplot_panel
            .set_legend_thresholds(cfg.min_width_for_legend, cfg.min_height_for_legend);
    }

    fn apply_controllers(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // WindowController: apply requested size/pos; publish current size/pos
        if let Some(ctrl) = &self.window_ctrl {
            // Apply requests
            let (req_size, req_pos) = {
                let mut inner = ctrl.inner.lock().unwrap();
                (inner.request_set_size.take(), inner.request_set_pos.take())
            };
            if let Some([w, h]) = req_size {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
            }
            // Positioning is not applied here due to API variability across platforms.
            // Publish current info
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

        // UiActionController: pause/resume, screenshot, exports, FFT requests (best-effort)
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
                // pause/resume
                if let Some(p) = take_actions.0 {
                    if p {
                        scope.paused = true;
                        self.main_panel.traces_data.take_snapshot();
                    } else {
                        scope.paused = false;
                    }
                }
                // screenshot now
                if take_actions.1 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                // screenshot to path: set env var for scope handler and trigger capture
                if let Some(path) = take_actions.2.take() {
                    std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                // save raw aligned snapshot (CSV/Parquet) to path
                if let Some((_fmt, path)) = take_actions.4.take() {
                    // Build aligned series from currently drawn points
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

                // save raw without path not handled here (needs UI to ask for path)

                // FFT requests not implemented in detail; clear them so callers don't block
                if let Some(_req) = take_actions.5.take() {
                    // No-op: placeholder until FFT panel provides data pipeline
                }
            }
        }

        // TracesController: apply queued changes and publish snapshot info
        if let Some(ctrl) = self.traces_ctrl.clone() {
            let (show_request, detached_request) = {
                let mut inner = ctrl.inner.lock().unwrap();
                let show_request = inner.show_request.take();
                let detached_request = inner.detached_request.take();

                let mut data = LivePlotData {
                    scope_data: self.main_panel.liveplot_panel.get_data_mut(),
                    traces: &mut self.main_panel.traces_data,
                    pending_requests: &mut self.main_panel.pending_requests,
                };
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
                                is_math: false,
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

            let mut trace_states: Vec<crate::controllers::TraceControlState> = Vec::new();
            for (name, tr) in self.main_panel.traces_data.traces_iter() {
                trace_states.push(crate::controllers::TraceControlState {
                    name: name.clone(),
                    color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                    width: tr.look.width,
                    style: tr.look.style,
                    visible: tr.look.visible,
                    offset: tr.offset,
                    is_math: false,
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

        // FFTController: reflect desired show state if FFT panel exists; publish panel size if present
        if let Some(ctrl) = &self.fft_ctrl {
            // Try to find an FFT panel and set its visibility/size
            // Currently not part of default layout; best-effort placeholder
            let mut inner = ctrl.inner.lock().unwrap();
            // We don't have actual panel size; set current_size to None for now
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

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        hotkey_helpers::handle_hotkeys(&mut self.main_panel, ctx);

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

        egui::CentralPanel::default().show(ctx, |ui| {
            // Non-UI calculations first
            self.main_panel.update(ui);
        });
        // Apply and publish controller requests after update
        self.apply_controllers(ctx, frame);
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub fn run_liveplot(
    rx: std::sync::mpsc::Receiver<PlotCommand>,
    mut cfg: crate::config::LivePlotConfig,
) -> eframe::Result<()> {
    let window_ctrl = cfg.window_controller.take();
    let ui_ctrl = cfg.ui_action_controller.take();
    let traces_ctrl = cfg.traces_controller.take();
    let scopes_ctrl = None;
    let liveplot_ctrl = None;
    let fft_ctrl = cfg.fft_controller.take();
    let threshold_ctrl = cfg.threshold_controller.take();
    let mut app = MainApp::with_controllers(
        rx,
        window_ctrl,
        ui_ctrl,
        traces_ctrl,
        scopes_ctrl,
        liveplot_ctrl,
        fft_ctrl,
        threshold_ctrl,
    );
    app.apply_config(&cfg);

    let title = cfg.title.clone();
    let mut opts = cfg
        .native_options
        .take()
        .unwrap_or_else(eframe::NativeOptions::default);
    // Try to set application icon from icon.svg if available
    if opts.viewport.icon.is_none() {
        if let Some(icon) = load_app_icon_svg() {
            opts.viewport = egui::ViewportBuilder::default().with_icon(icon);
        }
    }
    // Set a bigger default window size if one is not provided by config
    // Use `ViewportBuilder::with_inner_size` (winit/egui window attributes) instead of the
    // non-existent `initial_window_size` on NativeOptions in this eframe/egui version.
    if opts.viewport.inner_size.is_none() {
        opts.viewport = opts
            .viewport
            .clone()
            .with_inner_size(egui::vec2(1400.0, 900.0));
    }
    eframe::run_native(
        &title,
        opts,
        Box::new(|cc| {
            // Install Phosphor icon font before creating the app
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app))
        }),
    )
}

fn load_app_icon_svg() -> Option<egui::IconData> {
    // Prefer project-root icon.svg; fall back to none if not present.
    let svg_path = concat!(env!("CARGO_MANIFEST_DIR"), "/icon.svg");
    let data = std::fs::read(svg_path).ok()?;

    // Parse and render SVG to RGBA using usvg + resvg
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).ok()?;
    let size = tree.size().to_int_size();
    if size.width() == 0 || size.height() == 0 {
        return None;
    }
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    let mut canvas = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut canvas);
    let rgba = pixmap.take();
    Some(egui::IconData {
        rgba,
        width: size.width(),
        height: size.height(),
    })
}
