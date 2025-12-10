use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use eframe::egui;

use crate::controllers::{
    FFTController, ThresholdController, TracesController, UiActionController, WindowController,
};
use crate::data::export;
use crate::data::hotkeys as hotkey_helpers;
use crate::data::hotkeys::{HotkeyAction, Hotkeys};
use crate::data::traces::{TraceRef, TracesCollection};

use crate::data::data::LivePlotData;
use crate::panels::liveplot_ui::LiveplotPanel;
use crate::panels::panel_trait::Panel;
use crate::PlotCommand;

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
    pub(crate) fft_ctrl: Option<FFTController>,
    pub(crate) threshold_ctrl: Option<ThresholdController>,
    pub(crate) threshold_event_cursors: HashMap<String, usize>,
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
            //vec![Box::new(TracesPanel::default()), Box::new(MathPanel::default()), Box::new(ThresholdsPanel::default()), Box::new(TriggersPanel::default()), Box::new(ExportPanel::default())],
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
            fft_ctrl: None,
            threshold_ctrl: None,
            threshold_event_cursors: HashMap::new(),
        }
    }

    /// Attach controllers for embedded usage. These mirror the controllers used by MainApp.
    pub fn set_controllers(
        &mut self,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        fft_ctrl: Option<FFTController>,
        threshold_ctrl: Option<ThresholdController>,
    ) {
        self.window_ctrl = window_ctrl;
        self.ui_ctrl = ui_ctrl;
        self.traces_ctrl = traces_ctrl;
        self.fft_ctrl = fft_ctrl;
        self.threshold_ctrl = threshold_ctrl;
    }

    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.update_data();

        // Render UI
        self.render_menu(ui);
        self.render_panels(ui);

        // Draw additional overlay objects from other panels (e.g., thresholds)
        egui::CentralPanel::default().show_inside(ui, |ui| {
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

            // Global Clear All across tabs (rendered as a suffix-like element beneath the plot)
            let scope = self.liveplot_panel.get_data_mut();
            if ui
                .button("X Clear All")
                .on_hover_text("Clear all traces and per-panel buffers")
                .clicked()
            {
                self.traces_data.clear_all();
                // Also clear any last clicked point used by measurements/markers
                scope.clicked_point = None;
                // Broadcast clear_all to all panels (left/right/bottom/detached/empty)
                for p in right
                    .borrow_mut()
                    .iter_mut()
                    .chain(left.borrow_mut().iter_mut())
                    .chain(bottom.borrow_mut().iter_mut())
                    .chain(detached.borrow_mut().iter_mut())
                    .chain(empty.borrow_mut().iter_mut())
                {
                    p.clear_all();
                }
            }

            // Return panel lists back to self
            self.left_side_panels = left.into_inner();
            self.right_side_panels = right.into_inner();
            self.bottom_panels = bottom.into_inner();
            self.detached_panels = detached.into_inner();
            self.empty_panels = empty.into_inner();
        });
    }

    /// Update and render the panel when embedded in a parent app, and also apply controllers.
    pub fn update_embedded(&mut self, ui: &mut egui::Ui) {
        self.update(ui);
        self.apply_controllers_embedded(ui.ctx());
    }

    fn update_data(&mut self) {
        self.traces_data.update();

        // Apply any queued threshold add/remove requests before processing data so new defs
        // participate in this frame's evaluation.
        self.apply_threshold_controller_requests();

        self.liveplot_panel.update_data(&self.traces_data);
        let data = &mut LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
            request_save_state: None,
            request_load_state: None,
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

            let data = self.liveplot_panel.get_data_mut();
            if let Some(p) = take_actions.0 {
                let mut lp = LivePlotData {
                    scope_data: data,
                    traces: &mut self.traces_data,
                    request_save_state: None,
                    request_load_state: None,
                };
                if p {
                    lp.pause();
                } else {
                    lp.resume();
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
                let order = data.trace_order.clone();
                let series = order
                    .iter()
                    .filter_map(|name| {
                        data.get_drawn_points(name, &self.traces_data)
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
                // Placeholder for FFT data requests in embedded mode
            }
        }

        // TracesController: apply queued changes and publish snapshot info
        if let Some(ctrl) = &self.traces_ctrl {
            let mut inner = ctrl.inner.lock().unwrap();
            let data = self.liveplot_panel.get_data_mut();
            let traces = &mut self.traces_data;
            for (name, rgb) in inner.color_requests.drain(..) {
                let tref = TraceRef(name.clone());
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            }
            for (name, vis) in inner.visible_requests.drain(..) {
                let tref = TraceRef(name.clone());
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.visible = vis;
                }
            }
            for (name, off) in inner.offset_requests.drain(..) {
                let tref = TraceRef(name.clone());
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
            let y_unit = data.y_axis.unit.clone();
            let y_log = data.y_axis.log_scale;
            let snapshot = crate::controllers::TracesInfo {
                traces: infos,
                y_unit,
                y_log,
            };
            inner.listeners.retain(|s| s.send(snapshot.clone()).is_ok());
        }

        // FFT controller: publish basic info
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

    fn toggle_panel_visibility<T: 'static + Panel>(&mut self) -> bool {
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

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        // Render Menu

        egui::MenuBar::new().ui(ui, |ui| {
            self.liveplot_panel.render_menu(ui);

            let (save_req, load_req) = {
                let scope_data = self.liveplot_panel.get_data_mut();
                let mut data = LivePlotData {
                    scope_data,
                    traces: &mut self.traces_data,
                    request_save_state: None,
                    request_load_state: None,
                };

                for p in &mut self.left_side_panels {
                    p.render_menu(ui, &mut data);
                }
                for p in &mut self.right_side_panels {
                    p.render_menu(ui, &mut data);
                }
                for p in &mut self.bottom_panels {
                    p.render_menu(ui, &mut data);
                }
                for p in &mut self.detached_panels {
                    p.render_menu(ui, &mut data);
                }
                for p in &mut self.empty_panels {
                    p.render_menu(ui, &mut data);
                }

                (
                    data.request_save_state.take(),
                    data.request_load_state.take(),
                )
            };

            if let Some(path) = save_req {
                // Save state: build a serializable AppStateSerde and write it
                let ctx = ui.ctx();
                let rect = ctx.input(|i| i.content_rect());
                let win_size = Some([rect.width(), rect.height()]);
                let win_pos = Some([rect.left(), rect.top()]);
                let scope_data = self.liveplot_panel.get_data_mut();
                let scope_state = crate::persistence::ScopeStateSerde::from(&*scope_data);

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

                // Trace styles: snapshot & convert to serializable form to avoid long-lived borrows
                let order = scope_data.trace_order.clone();
                let trace_styles: Vec<crate::persistence::TraceStyleSerde> = {
                    let mut snapshot: Vec<(String, crate::data::trace_look::TraceLook, f64)> =
                        Vec::new();
                    for name in order.iter() {
                        if let Some(tr) = self.traces_data.get_trace(name) {
                            snapshot.push((name.0.clone(), tr.look.clone(), tr.offset));
                        }
                    }
                    let mut out: Vec<crate::persistence::TraceStyleSerde> = Vec::new();
                    for (n, look, off) in snapshot.into_iter() {
                        out.push(crate::persistence::TraceStyleSerde {
                            name: n,
                            look: crate::persistence::TraceLookSerde::from(&look),
                            offset: off,
                        });
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
                    scope: scope_state,
                    panels: panels_state,
                    traces_style: trace_styles,
                    thresholds: thresholds_ser,
                    triggers: triggers_ser,
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
                    // Scope: apply to scope data
                    let scope_data = self.liveplot_panel.get_data_mut();
                    loaded.scope.clone().apply_to(scope_data);

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

                    // Apply traces styles
                    crate::persistence::apply_trace_styles(
                        &loaded.traces_style,
                        |name, look, off| {
                            let tref = TraceRef(name.to_string());
                            if let Some(tr) = self.traces_data.get_trace_mut(&tref) {
                                tr.look = look;
                                tr.offset = off;
                            }
                        },
                    );

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
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(280.0)
                .min_width(160.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Min);
                });
            self.left_side_panels = list;
        } else if !self.left_side_panels.is_empty() {
            let mut list = std::mem::take(&mut self.left_side_panels);
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(30.0)
                .min_width(30.0)
                .show_inside(ui, |ui| {
                    let mut clicked: Option<usize> = None;
                    ui.vertical(|ui| {
                        // Compute max width for compact icons so buttons have consistent size
                        let button_font = egui::TextStyle::Button.resolve(ui.style());
                        let mut max_w = 0.0_f32;

                        ui.fonts_mut(|f| {
                            for p in list.iter() {
                                let label = p.icon_only().unwrap_or(p.title()).to_string();
                                let w = f
                                    .layout_no_wrap(
                                        label,
                                        button_font.clone(),
                                        egui::Color32::WHITE,
                                    )
                                    .rect
                                    .width();
                                if w > max_w {
                                    max_w = w;
                                }
                            }
                        });

                        for (i, p) in list.iter_mut().enumerate() {
                            let active = p.state().visible && !p.state().detached;
                            let label = p.icon_only().unwrap_or(p.title()).to_string();
                            if ui.selectable_label(active, label).clicked() {
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
            self.left_side_panels = list;
        }
        if show_right {
            let mut list = std::mem::take(&mut self.right_side_panels);
            egui::SidePanel::right("right_sidebar")
                .resizable(true)
                .default_width(320.0)
                .min_width(200.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Max);
                });
            self.right_side_panels = list;
        } else if !self.right_side_panels.is_empty() {
            let mut list = std::mem::take(&mut self.right_side_panels);
            egui::SidePanel::right("right_sidebar")
                .resizable(true)
                .default_width(30.0)
                .min_width(30.0)
                .show_inside(ui, |ui| {
                    let mut clicked: Option<usize> = None;
                    ui.vertical(|ui| {
                        // Compute max width for compact icons so buttons have consistent size
                        let button_font = egui::TextStyle::Button.resolve(ui.style());
                        let mut max_w = 0.0_f32;

                        ui.fonts_mut(|f| {
                            for p in list.iter() {
                                let label = p.icon_only().unwrap_or(p.title()).to_string();
                                let w = f
                                    .layout_no_wrap(
                                        label,
                                        button_font.clone(),
                                        egui::Color32::WHITE,
                                    )
                                    .rect
                                    .width();
                                if w > max_w {
                                    max_w = w;
                                }
                            }
                        });

                        for (i, p) in list.iter_mut().enumerate() {
                            let active = p.state().visible && !p.state().detached;
                            let label = p.icon_only().unwrap_or(p.title()).to_string();
                            if ui.selectable_label(active, label).clicked() {
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
            egui::TopBottomPanel::bottom("bottom_bar")
                .resizable(true)
                .default_height(220.0)
                .min_height(120.0)
                .show_inside(ui, |ui| {
                    self.render_tabs(ui, &mut list, egui::Align::Max);
                });
            self.bottom_panels = list;
        } else if !self.bottom_panels.is_empty() {
            let mut list = std::mem::take(&mut self.bottom_panels);
            egui::TopBottomPanel::bottom("bottom_bar")
                .resizable(false)
                .default_height(24.0)
                .min_height(24.0)
                .show_inside(ui, |ui| {
                    let mut clicked: Option<usize> = None;
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        for (i, p) in list.iter_mut().enumerate() {
                            let label = p.title_and_icon();
                            if ui.button(label).clicked() {
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

        // Detached windows
        // Detached left windows via panel trait helper
        for p in &mut self.left_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        request_save_state: None,
                        request_load_state: None,
                    },
                );
            }
        }

        // Detached right windows via panel trait helper
        for p in &mut self.right_side_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        request_save_state: None,
                        request_load_state: None,
                    },
                );
            }
        }

        // Detached bottom windows via panel trait helper
        for p in &mut self.bottom_panels {
            if p.state().visible && p.state().detached {
                p.show_detached_dialog(
                    ui.ctx(),
                    &mut LivePlotData {
                        scope_data: self.liveplot_panel.get_data_mut(),
                        traces: &mut self.traces_data,
                        request_save_state: None,
                        request_load_state: None,
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
                        request_save_state: None,
                        request_load_state: None,
                    },
                );
            }
        }
    }

    fn render_tabs(
        &mut self,
        ui: &mut egui::Ui,
        list: &mut Vec<Box<dyn Panel>>,
        _align: egui::Align,
    ) {
        let count = list.len();

        let mut clicked: Option<usize> = None;

        let scope_data = self.liveplot_panel.get_data_mut();
        let data = &mut LivePlotData {
            scope_data,
            traces: &mut self.traces_data,
            request_save_state: None,
            request_load_state: None,
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
            // Decide if actions fit on the same row; if not, render them on a new row.
            let actions_need_row_below = {
                let available = ui.available_width();
                // Estimate width of tabs/labels
                let button_font = egui::TextStyle::Button.resolve(ui.style());
                let txt_width = |text: &str, ui: &egui::Ui| -> f32 {
                    ui.fonts_mut(|f| {
                        f.layout_no_wrap(text.to_owned(), button_font.clone(), egui::Color32::WHITE)
                            .rect
                            .width()
                    })
                };
                let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
                // Measure combined label width (title + optional icon)
                let tabs_w: f32 = match count {
                    0 => 0.0,
                    1 => txt_width(&list[0].title_and_icon(), ui) + pad,
                    _ => list
                        .iter()
                        .map(|p| txt_width(&p.title_and_icon(), ui) + pad)
                        .sum(),
                };
                let actions_w = txt_width("Pop out", ui) + pad + txt_width("Hide", ui) + pad;
                tabs_w + actions_w > available
            };

            ui.horizontal(|ui| {
                if count > 1 {
                    for (i, p) in list.iter_mut().enumerate() {
                        let active = p.state().visible && !p.state().detached;
                        if ui.selectable_label(active, p.title_and_icon()).clicked() {
                            clicked = Some(i);
                        }
                    }
                } else {
                    let p = &mut list[0];
                    ui.label(p.title_and_icon());
                    clicked = Some(0);
                }

                if !actions_need_row_below {
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
                                    p.state_mut().request_docket = false;
                                    p.state_mut().visible = true;
                                    p.state_mut().request_focus = true;
                                }
                            }
                        }
                    });
                }
            });

            if actions_need_row_below {
                ui.horizontal(|ui| {
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
                                    p.state_mut().request_docket = false;
                                    p.state_mut().visible = true;
                                    p.state_mut().request_focus = true;
                                }
                            }
                        }
                    });
                });
            }

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
    }
}

pub struct MainApp {
    pub main_panel: MainPanel,
    // Optional external controllers
    pub window_ctrl: Option<WindowController>,
    pub ui_ctrl: Option<UiActionController>,
    pub traces_ctrl: Option<TracesController>,
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
        fft_ctrl: Option<FFTController>,
        threshold_ctrl: Option<ThresholdController>,
    ) -> Self {
        let mut main_panel = MainPanel::new(rx);
        main_panel.set_controllers(
            window_ctrl.clone(),
            ui_ctrl.clone(),
            traces_ctrl.clone(),
            fft_ctrl.clone(),
            threshold_ctrl.clone(),
        );
        Self {
            main_panel,
            window_ctrl,
            ui_ctrl,
            traces_ctrl,
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
            scope.time_window = cfg.time_window_secs;
            scope.y_axis.unit = cfg.y_unit.clone();
            scope.y_axis.log_scale = cfg.y_log;
            scope.x_axis.format = Some(match cfg.x_date_format {
                crate::config::XDateFormat::Iso8601WithDate => "%Y-%m-%d %H:%M:%S".to_string(),
                crate::config::XDateFormat::Iso8601Time => "%H:%M:%S".to_string(),
            });
        }

        // Trace storage limits
        self.main_panel.traces_data.max_points = cfg.max_points;

        // Show legend
        self.main_panel.liveplot_panel.get_data_mut().show_legend = cfg.show_legend;

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

            let data = self.main_panel.liveplot_panel.get_data_mut();

            // pause/resume
            if let Some(p) = take_actions.0 {
                let mut lp = LivePlotData {
                    scope_data: data,
                    traces: &mut self.main_panel.traces_data,
                    request_save_state: None,
                    request_load_state: None,
                };
                if p {
                    lp.pause();
                } else {
                    lp.resume();
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
                let order = data.trace_order.clone();
                let series = order
                    .iter()
                    .filter_map(|name| {
                        data.get_drawn_points(name, &self.main_panel.traces_data)
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

        // TracesController: apply queued changes and publish snapshot info
        if let Some(ctrl) = &self.traces_ctrl {
            let mut inner = ctrl.inner.lock().unwrap();
            let data = self.main_panel.liveplot_panel.get_data_mut();
            let traces = &mut self.main_panel.traces_data;
            for (name, rgb) in inner.color_requests.drain(..) {
                let tref = TraceRef(name.clone());
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            }
            for (name, vis) in inner.visible_requests.drain(..) {
                let tref = TraceRef(name.clone());
                if let Some(tr) = traces.get_trace_mut(&tref) {
                    tr.look.visible = vis;
                }
            }
            for (name, off) in inner.offset_requests.drain(..) {
                let tref = TraceRef(name.clone());
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

            // Publish current traces snapshot
            let mut infos: Vec<crate::controllers::TraceInfo> = Vec::new();
            for name in data.trace_order.iter() {
                if let Some(tr) = self.main_panel.traces_data.get_trace(name) {
                    infos.push(crate::controllers::TraceInfo {
                        name: name.0.clone(),
                        color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                        visible: tr.look.visible,
                        is_math: false, // no math differentiation here
                        offset: tr.offset,
                    });
                }
            }
            let y_unit = data.y_axis.unit.clone();
            let y_log = data.y_axis.log_scale;
            let snapshot = crate::controllers::TracesInfo {
                traces: infos,
                y_unit,
                y_log,
            };
            inner.listeners.retain(|s| s.send(snapshot.clone()).is_ok());
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
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }

    // Hotkey helpers moved to `data/hotkeys.rs` (re-exported as `craft::hotkeys`).

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        let hk = self.main_panel.hotkeys.borrow().clone();
        let actions = hotkey_helpers::detect_hotkey_actions(&hk, ctx);
        for act in actions {
            match act {
                HotkeyAction::Pause => {
                    let data = self.main_panel.liveplot_panel.get_data_mut();
                    let mut lp = LivePlotData {
                        scope_data: data,
                        traces: &mut self.main_panel.traces_data,
                        request_save_state: None,
                        request_load_state: None,
                    };
                    if lp.is_paused() {
                        lp.resume();
                        self.main_panel.traces_data.clear_snapshot();
                    } else {
                        lp.pause();
                    }
                }
                HotkeyAction::FitView => {
                    let data = self.main_panel.liveplot_panel.get_data_mut();
                    data.fit_bounds(&self.main_panel.traces_data);
                }
                HotkeyAction::FitViewCont => {
                    let data = self.main_panel.liveplot_panel.get_data_mut();
                    data.y_axis.auto_fit = !data.y_axis.auto_fit;
                    if data.y_axis.auto_fit {
                        data.fit_y_bounds(&self.main_panel.traces_data);
                    }
                }
                HotkeyAction::ResetMarkers => {
                    let data = self.main_panel.liveplot_panel.get_data_mut();
                    data.clicked_point = None;
                }
                HotkeyAction::ToggleTraces => {
                    self.main_panel.toggle_panel_visibility::<TracesPanel>();
                    // If we just toggled traces via hotkey, hide hotkeys panel
                    self.main_panel.hide_hotkeys_panel();
                }
                HotkeyAction::ToggleMath => {
                    self.main_panel.toggle_panel_visibility::<MathPanel>();
                    self.main_panel.hide_hotkeys_panel();
                }
                HotkeyAction::ToggleThresholds => {
                    self.main_panel.toggle_panel_visibility::<ThresholdsPanel>();
                    self.main_panel.hide_hotkeys_panel();
                }
                HotkeyAction::ToggleExport => {
                    self.main_panel.toggle_panel_visibility::<ExportPanel>();
                    self.main_panel.hide_hotkeys_panel();
                }
                HotkeyAction::ToggleFft => {
                    #[cfg(feature = "fft")]
                    {
                        self.main_panel.toggle_panel_visibility::<FftPanel>();
                        self.main_panel.hide_hotkeys_panel();
                    }
                }
                HotkeyAction::SavePng => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            }
        }
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_hotkeys(ctx);

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
    let fft_ctrl = cfg.fft_controller.take();
    let threshold_ctrl = cfg.threshold_controller.take();
    let mut app = MainApp::with_controllers(
        rx,
        window_ctrl,
        ui_ctrl,
        traces_ctrl,
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
