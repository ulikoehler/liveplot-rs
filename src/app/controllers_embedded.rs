//! Embedded controller request handling for [`MainPanel`].
//!
//! When `MainPanel` is embedded inside a parent application (rather than
//! running standalone via [`MainApp`](super::MainApp)), the host code
//! communicates with the panel through *controllers* – thread-safe handles
//! that queue requests and receive state snapshots.
//!
//! This module implements:
//!
//! * [`apply_controllers_embedded`](MainPanel::apply_controllers_embedded) –
//!   processes all queued controller requests and publishes state snapshots.
//! * [`apply_threshold_controller_requests`](MainPanel::apply_threshold_controller_requests) –
//!   adds/removes threshold definitions from the thresholds panel.
//! * [`publish_threshold_events`](MainPanel::publish_threshold_events) –
//!   forwards newly generated threshold crossing events to listeners.

use std::collections::HashMap;

use eframe::egui;

use crate::data::data::LivePlotData;
use crate::data::export;
use crate::data::traces::TraceRef;

use super::MainPanel;

impl MainPanel {
    /// Apply controller requests and publish state, for embedded usage (no stand-alone window frame).
    ///
    /// This method handles all optional controllers:
    ///
    /// * **WindowController** – publishes viewport size/position, applies resize requests.
    /// * **UiActionController** – pause/resume, screenshot, raw data export.
    /// * **TracesController** – colour, visibility, offset, width, style changes;
    ///   publishes the current trace snapshot.
    /// * **ScopesController** – add/remove/configure scopes.
    /// * **LiveplotController** – pause all, clear all, save/load state, window commands.
    /// * **FFTController** – publishes FFT panel info.
    /// * **ThresholdController** – threshold add/remove and event publishing
    ///   (via [`apply_threshold_controller_requests`] and [`publish_threshold_events`]).
    pub fn apply_controllers_embedded(&mut self, ctx: &egui::Context) {
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

        // ── ThresholdController (add/remove definitions) ─────────────────────
        self.apply_threshold_controller_requests();
        self.publish_threshold_events();

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

        // ── TracesController ─────────────────────────────────────────────────
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

            // Apply show/detached state to the TracesPanel widget.
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

            // Publish panel-level state snapshot.
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

        // ── ScopesController ─────────────────────────────────────────────────
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

        // ── LiveplotController ───────────────────────────────────────────────
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

        // ── ScopesController (second pass – publish final state) ─────────────
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

        // ── LiveplotController (second pass – publish final state) ───────────
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

    /// Process any queued threshold add/remove requests from the [`ThresholdController`].
    ///
    /// New threshold definitions are inserted into the [`ThresholdsPanel`]; removed
    /// ones are deleted both from the panel and from the event cursor map.
    pub(crate) fn apply_threshold_controller_requests(&mut self) {
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

    /// Forward newly generated threshold crossing events to controller listeners.
    ///
    /// Each threshold definition accumulates events in its runtime state.  This
    /// method tracks a per-threshold cursor so that only events generated *since
    /// the last call* are forwarded.
    pub(crate) fn publish_threshold_events(&mut self) {
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

        // Drop cursors for thresholds no longer present (e.g., removed via UI).
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
                // Events were cleared (e.g. threshold reset); resync cursor.
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
}
