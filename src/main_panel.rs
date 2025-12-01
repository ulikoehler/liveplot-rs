//! MainPanel architecture: modular panel-based UI for the plotting application.
//!
//! This module provides a flexible panel-based architecture where different UI
//! components (traces panel, math panel, thresholds panel, etc.) can be docked
//! to different locations or detached as floating windows.

use eframe::egui;

use crate::controllers::{FFTController, TracesController, UiActionController, WindowController};
use crate::data::data::LivePlotData;
use crate::data::scope::ScopeData;
use crate::data::traces::{TraceRef, TracesCollection};
use crate::panels::panel_trait::Panel;
use crate::sink::PlotCommand;

/// The scope/plot panel that renders the main plot area.
pub struct ScopePanel {
    data: ScopeData,
    time_slider_dragging: bool,
    time_window_bounds: (f64, f64),
}

impl Default for ScopePanel {
    fn default() -> Self {
        Self {
            data: ScopeData::default(),
            time_slider_dragging: false,
            time_window_bounds: (0.1, 100.0),
        }
    }
}

impl ScopePanel {
    /// Get mutable access to scope data.
    pub fn get_data_mut(&mut self) -> &mut ScopeData {
        &mut self.data
    }

    /// Update scope data from traces collection.
    pub fn update_data(&mut self, traces: &TracesCollection) {
        self.data.update(traces);
    }
}

/// Wrapper around ScopePanel for the LivePlot main view.
pub struct LiveplotPanel {
    scope_ui: ScopePanel,
    points_bounds: (usize, usize),
}

impl Default for LiveplotPanel {
    fn default() -> Self {
        Self {
            scope_ui: ScopePanel::default(),
            points_bounds: (5000, 200000),
        }
    }
}

impl LiveplotPanel {
    /// Get mutable access to scope data.
    pub fn get_data_mut(&mut self) -> &mut ScopeData {
        self.scope_ui.get_data_mut()
    }

    /// Update from traces collection.
    pub fn update_data(&mut self, traces: &TracesCollection) {
        self.scope_ui.update_data(traces);
    }

    /// Render the menu bar items for this panel.
    pub fn render_menu(&mut self, _ui: &mut egui::Ui) {
        // Menu items specific to the main plot
    }
}

/// The main panel that orchestrates all sub-panels.
///
/// This is the Janosch-style modular architecture where panels can be
/// docked to different regions (left, right, bottom) or detached as
/// floating windows.
pub struct MainPanel {
    /// The trace data collection.
    pub traces_data: TracesCollection,
    /// The main liveplot/scope panel.
    pub liveplot_panel: LiveplotPanel,
    /// Panels docked to the right side.
    pub right_side_panels: Vec<Box<dyn Panel>>,
    /// Panels docked to the left side.
    pub left_side_panels: Vec<Box<dyn Panel>>,
    /// Panels docked to the bottom.
    pub bottom_panels: Vec<Box<dyn Panel>>,
    /// Detached (floating) panels.
    pub detached_panels: Vec<Box<dyn Panel>>,
    /// Panels that are hidden/empty by default.
    pub empty_panels: Vec<Box<dyn Panel>>,
    // Optional controllers for embedded usage
    pub(crate) window_ctrl: Option<WindowController>,
    pub(crate) ui_ctrl: Option<UiActionController>,
    pub(crate) traces_ctrl: Option<TracesController>,
    pub(crate) fft_ctrl: Option<FFTController>,
}

impl MainPanel {
    /// Create a new MainPanel with the given data receiver.
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        Self {
            traces_data: TracesCollection::new(rx),
            liveplot_panel: LiveplotPanel::default(),
            right_side_panels: Vec::new(),
            left_side_panels: Vec::new(),
            bottom_panels: Vec::new(),
            detached_panels: Vec::new(),
            empty_panels: Vec::new(),
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            fft_ctrl: None,
        }
    }

    /// Attach controllers for embedded usage.
    pub fn set_controllers(
        &mut self,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        fft_ctrl: Option<FFTController>,
    ) {
        self.window_ctrl = window_ctrl;
        self.ui_ctrl = ui_ctrl;
        self.traces_ctrl = traces_ctrl;
        self.fft_ctrl = fft_ctrl;
    }

    /// Add a panel to the right side.
    pub fn add_right_panel(&mut self, panel: Box<dyn Panel>) {
        self.right_side_panels.push(panel);
    }

    /// Add a panel to the left side.
    pub fn add_left_panel(&mut self, panel: Box<dyn Panel>) {
        self.left_side_panels.push(panel);
    }

    /// Add a panel to the bottom.
    pub fn add_bottom_panel(&mut self, panel: Box<dyn Panel>) {
        self.bottom_panels.push(panel);
    }

    /// Main update function - call this in your egui update loop.
    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.update_data();
        self.render_menu(ui);
        self.render_panels(ui);
    }

    /// Update and render when embedded, also applying controllers.
    pub fn update_embedded(&mut self, ui: &mut egui::Ui) {
        self.update(ui);
        self.apply_controllers_embedded(ui.ctx());
    }

    fn update_data(&mut self) {
        self.traces_data.update();
        self.liveplot_panel.update_data(&self.traces_data);

        let data = &mut LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
        };

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
    }

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            self.liveplot_panel.render_menu(ui);

            let scope_data = self.liveplot_panel.get_data_mut();
            let data = &mut LivePlotData {
                scope_data,
                traces: &mut self.traces_data,
            };

            for p in &mut self.left_side_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.right_side_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.bottom_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.detached_panels {
                p.render_menu(ui, data);
            }
            for p in &mut self.empty_panels {
                p.render_menu(ui, data);
            }

            ui.menu_button("Panels", |ui| {
                let mut all_panels: Vec<&mut Box<dyn Panel>> = Vec::new();
                all_panels.extend(self.left_side_panels.iter_mut());
                all_panels.extend(self.right_side_panels.iter_mut());
                all_panels.extend(self.bottom_panels.iter_mut());
                all_panels.extend(self.detached_panels.iter_mut());

                for p in all_panels {
                    if ui
                        .selectable_label(p.state().visible, p.title())
                        .clicked()
                    {
                        p.state_mut().visible = !p.state().visible;
                        p.state_mut().detached = false;
                    }
                }
            });

            // State save/load menu
            ui.menu_button("State", |ui| {
                if ui.button("Save state...").clicked() {
                    self.save_state_dialog(ui.ctx());
                    ui.close();
                }
                if ui.button("Load state...").clicked() {
                    self.load_state_dialog(ui.ctx());
                    ui.close();
                }
            });
        });
    }

    fn render_panels(&mut self, ui: &mut egui::Ui) {
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
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(280.0)
                .min_width(160.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list);
                });
            self.left_side_panels = list;
        }

        if show_right {
            let mut list = std::mem::take(&mut self.right_side_panels);
            egui::SidePanel::right("right_sidebar")
                .resizable(true)
                .default_width(320.0)
                .min_width(200.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list);
                });
            self.right_side_panels = list;
        }

        if show_bottom {
            let mut list = std::mem::take(&mut self.bottom_panels);
            egui::TopBottomPanel::bottom("bottom_bar")
                .resizable(true)
                .default_height(220.0)
                .min_height(120.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list);
                });
            self.bottom_panels = list;
        }

        // Render detached windows
        self.render_detached_windows(ui.ctx());

        // Central panel for main plot
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_main_plot(ui);
        });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui, list: &mut Vec<Box<dyn Panel>>) {
        let count = list.len();
        if count == 0 {
            return;
        }

        let scope_data = self.liveplot_panel.get_data_mut();
        let data = &mut LivePlotData {
            scope_data,
            traces: &mut self.traces_data,
        };

        // Tab bar
        let mut clicked: Option<usize> = None;
        ui.horizontal(|ui| {
            if count > 1 {
                for (i, p) in list.iter_mut().enumerate() {
                    let active = p.state().visible && !p.state().detached;
                    if ui.selectable_label(active, p.title()).clicked() {
                        clicked = Some(i);
                    }
                }
            } else {
                ui.label(list[0].title());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                        }
                    }
                }
            });
        });

        // Apply tab selection
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

        ui.separator();

        // Render active panel content
        if let Some(p) = list
            .iter_mut()
            .find(|p| p.state().visible && !p.state().detached)
        {
            p.render_panel(ui, data);
        } else {
            ui.label("No panel active");
        }
    }

    fn render_detached_windows(&mut self, ctx: &egui::Context) {
        let scope_data = self.liveplot_panel.get_data_mut();
        let data = &mut LivePlotData {
            scope_data,
            traces: &mut self.traces_data,
        };

        for p in &mut self.left_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ctx, data);
            }
        }
        for p in &mut self.right_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ctx, data);
            }
        }
        for p in &mut self.bottom_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ctx, data);
            }
        }
        for p in &mut self.detached_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(ctx, data);
            }
        }
    }

    fn render_main_plot(&mut self, ui: &mut egui::Ui) {
        // Placeholder for main plot rendering
        // In a full implementation, this would render the scope panel
        ui.horizontal(|ui| {
            ui.label("Data Points:");
            ui.add(egui::Slider::new(
                &mut self.traces_data.max_points,
                self.liveplot_panel.points_bounds.0..=self.liveplot_panel.points_bounds.1,
            ));

            ui.separator();

            let scope = self.liveplot_panel.get_data_mut();
            if !scope.paused {
                if ui.button("⏸ Pause").clicked() {
                    scope.paused = true;
                    self.traces_data.take_snapshot();
                }
            } else if ui.button("▶ Resume").clicked() {
                scope.paused = false;
            }

            ui.separator();

            if ui.button("Clear All").clicked() {
                self.traces_data.clear_all();
                self.liveplot_panel.get_data_mut().clicked_point = None;
            }
        });

        ui.separator();

        // Simple status display
        let trace_count = self.traces_data.len();
        ui.label(format!("Traces: {}", trace_count));
    }

    fn save_state_dialog(&self, _ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("liveplot_state.json")
            .save_file()
        {
            let state = self.capture_state();
            if let Err(e) = crate::persistence::save_state_to_path(&state, &path) {
                eprintln!("Failed to save state: {}", e);
            }
        }
    }

    fn load_state_dialog(&mut self, _ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match crate::persistence::load_state_from_path(&path) {
                Ok(state) => self.apply_state(state),
                Err(e) => eprintln!("Failed to load state: {}", e),
            }
        }
    }

    /// Capture the current state for persistence.
    pub fn capture_state(&self) -> crate::persistence::AppStateSerde {
        use crate::persistence::*;

        let scope_data = &self.liveplot_panel.scope_ui.data;

        // Capture trace styles manually (avoid closure lifetime issues)
        let mut traces_style = Vec::new();
        for name in scope_data.trace_order.iter() {
            if let Some(tr) = self.traces_data.get_trace(name) {
                traces_style.push(TraceStyleSerde {
                    name: name.0.clone(),
                    look: TraceLookSerde::from(&tr.look),
                    offset: tr.offset,
                });
            }
        }

        // Capture panel visibility
        let mut panels = Vec::new();
        let collect_panels = |list: &[Box<dyn Panel>], out: &mut Vec<PanelVisSerde>| {
            for p in list {
                let st = p.state();
                out.push(PanelVisSerde {
                    title: st.title.to_string(),
                    visible: st.visible,
                    detached: st.detached,
                    window_pos: st.window_pos,
                    window_size: st.window_size,
                });
            }
        };
        collect_panels(&self.left_side_panels, &mut panels);
        collect_panels(&self.right_side_panels, &mut panels);
        collect_panels(&self.bottom_panels, &mut panels);
        collect_panels(&self.detached_panels, &mut panels);
        collect_panels(&self.empty_panels, &mut panels);

        AppStateSerde {
            window_size: None,
            window_pos: None,
            scope: ScopeStateSerde::from(scope_data),
            panels,
            traces_style,
            thresholds: Vec::new(),
            triggers: Vec::new(),
        }
    }

    /// Apply a loaded state.
    pub fn apply_state(&mut self, state: crate::persistence::AppStateSerde) {
        use crate::persistence::*;

        // Apply scope settings
        state.scope.apply_to(self.liveplot_panel.get_data_mut());

        // Apply trace styles
        apply_trace_styles(&state.traces_style, |name, look, offset| {
            if let Some(tr) = self.traces_data.get_trace_mut(&TraceRef(name.to_string())) {
                tr.look = look;
                tr.offset = offset;
            }
        });

        // Apply panel visibility
        let panel_info: std::collections::HashMap<
            String,
            (bool, bool, Option<[f32; 2]>, Option<[f32; 2]>),
        > = state
            .panels
            .into_iter()
            .map(|p| (p.title, (p.visible, p.detached, p.window_pos, p.window_size)))
            .collect();

        let apply_vis = |list: &mut [Box<dyn Panel>],
                         infos: &std::collections::HashMap<
            String,
            (bool, bool, Option<[f32; 2]>, Option<[f32; 2]>),
        >| {
            for p in list {
                if let Some((vis, det, pos, sz)) = infos.get(p.title()) {
                    let st = p.state_mut();
                    st.visible = *vis;
                    st.detached = *det;
                    st.window_pos = *pos;
                    st.window_size = *sz;
                }
            }
        };
        apply_vis(&mut self.left_side_panels, &panel_info);
        apply_vis(&mut self.right_side_panels, &panel_info);
        apply_vis(&mut self.bottom_panels, &panel_info);
        apply_vis(&mut self.detached_panels, &panel_info);
        apply_vis(&mut self.empty_panels, &panel_info);
    }

    /// Apply controller requests for embedded usage.
    pub fn apply_controllers_embedded(&mut self, ctx: &egui::Context) {
        // WindowController
        if let Some(ctrl) = &self.window_ctrl {
            let (req_size, _req_pos) = {
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
                requested_pos: None,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.current_size = Some(size);
            inner.current_pos = Some(pos);
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // UiActionController
        if let Some(ctrl) = &self.ui_ctrl {
            let pause_req = {
                let mut inner = ctrl.inner.lock().unwrap();
                inner.request_pause.take()
            };

            let data = self.liveplot_panel.get_data_mut();
            if let Some(p) = pause_req {
                let mut lp = LivePlotData {
                    scope_data: data,
                    traces: &mut self.traces_data,
                };
                if p {
                    lp.pause();
                } else {
                    lp.resume();
                }
            }
        }

        // TracesController
        if let Some(ctrl) = &self.traces_ctrl {
            let mut inner = ctrl.inner.lock().unwrap();
            let data = self.liveplot_panel.get_data_mut();
            let traces = &mut self.traces_data;

            for (name, rgb) in inner.color_requests.drain(..) {
                let tref = TraceRef(name);
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            }
            for (name, vis) in inner.visible_requests.drain(..) {
                let tref = TraceRef(name);
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.visible = vis;
                }
            }
            for (name, off) in inner.offset_requests.drain(..) {
                let tref = TraceRef(name);
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.offset = off;
                }
            }
            if let Some(unit) = inner.y_unit_request.take() {
                data.y_axis.unit = unit;
            }
            if let Some(ylog) = inner.y_log_request.take() {
                data.y_axis.log_scale = ylog;
            }
            if let Some(sel) = inner.selection_request.take() {
                data.selection_trace = sel.map(TraceRef);
            }

            // Publish snapshot
            let mut infos: Vec<crate::controllers::TraceInfo> = Vec::new();
            for name in data.trace_order.iter() {
                if let Some(tr) = self.traces_data.get_trace(name) {
                    infos.push(crate::controllers::TraceInfo {
                        name: name.0.clone(),
                        color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                        visible: tr.look.visible,
                        is_math: false,
                        offset: tr.offset,
                    });
                }
            }
            let snapshot = crate::controllers::TracesInfo {
                traces: infos,
                marker_selection: data.selection_trace.as_ref().map(|t| t.0.clone()),
                y_unit: data.y_axis.unit.clone(),
                y_log: data.y_axis.log_scale,
            };
            inner.listeners.retain(|s| s.send(snapshot.clone()).is_ok());
        }

        // FFTController
        if let Some(ctrl) = &self.fft_ctrl {
            let mut inner = ctrl.inner.lock().unwrap();
            let info = crate::controllers::FFTPanelInfo {
                shown: inner.show,
                current_size: None,
                requested_size: inner.request_set_size,
            };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }
}

/// Main application wrapper that implements eframe::App.
pub struct MainApp {
    pub main_panel: MainPanel,
    pub window_ctrl: Option<WindowController>,
    pub ui_ctrl: Option<UiActionController>,
    pub traces_ctrl: Option<TracesController>,
    pub fft_ctrl: Option<FFTController>,
}

impl MainApp {
    /// Create a new MainApp with the given data receiver.
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        Self {
            main_panel: MainPanel::new(rx),
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            fft_ctrl: None,
        }
    }

    /// Create with optional controllers.
    pub fn with_controllers(
        rx: std::sync::mpsc::Receiver<PlotCommand>,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        fft_ctrl: Option<FFTController>,
    ) -> Self {
        let mut app = Self::new(rx);
        app.window_ctrl = window_ctrl.clone();
        app.ui_ctrl = ui_ctrl.clone();
        app.traces_ctrl = traces_ctrl.clone();
        app.fft_ctrl = fft_ctrl.clone();
        app.main_panel
            .set_controllers(window_ctrl, ui_ctrl, traces_ctrl, fft_ctrl);
        app
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.main_panel.update(ui);
        });
        self.main_panel.apply_controllers_embedded(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

/// Run the liveplot application with the MainPanel architecture.
pub fn run_liveplot_modular(rx: std::sync::mpsc::Receiver<PlotCommand>) -> eframe::Result<()> {
    let app = MainApp::new(rx);
    let title = "LivePlot".to_string();
    let mut opts = eframe::NativeOptions::default();
    if let Some(icon) = load_app_icon_svg() {
        opts.viewport = egui::ViewportBuilder::default().with_icon(icon);
    }
    eframe::run_native(
        &title,
        opts,
        Box::new(|_cc| {
            Ok(Box::new(app))
        }),
    )
}

/// Run with controllers.
pub fn run_liveplot_modular_with_controllers(
    rx: std::sync::mpsc::Receiver<PlotCommand>,
    window_ctrl: Option<WindowController>,
    ui_ctrl: Option<UiActionController>,
    traces_ctrl: Option<TracesController>,
    fft_ctrl: Option<FFTController>,
) -> eframe::Result<()> {
    let app = MainApp::with_controllers(rx, window_ctrl, ui_ctrl, traces_ctrl, fft_ctrl);
    let title = "LivePlot".to_string();
    let mut opts = eframe::NativeOptions::default();
    if let Some(icon) = load_app_icon_svg() {
        opts.viewport = egui::ViewportBuilder::default().with_icon(icon);
    }
    eframe::run_native(
        &title,
        opts,
        Box::new(|_cc| {
            Ok(Box::new(app))
        }),
    )
}

fn load_app_icon_svg() -> Option<egui::IconData> {
    let svg_path = concat!(env!("CARGO_MANIFEST_DIR"), "/icon.svg");
    let data = std::fs::read(svg_path).ok()?;

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
