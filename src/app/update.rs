//! Per-frame update logic for [`LivePlotPanel`].
//!
//! This module contains the methods that drive each frame of the LivePlot UI:
//!
//! * **[`update`](LivePlotPanel::update)** – the top-level entry point called every
//!   frame.  It ingests new data, renders the menu bar and side panels, and
//!   finally draws the central plot area with panel overlays.
//! * **[`update_embedded`](LivePlotPanel::update_embedded)** – convenience wrapper
//!   that additionally applies embedded controllers after the normal update.
//! * **[`update_data`](LivePlotPanel::update_data)** – the data-only pass that
//!   processes incoming [`PlotCommand`](crate::PlotCommand)s, refreshes every
//!   sub-panel, and evaluates threshold/trigger logic.
//! * **[`fit_all_bounds`](LivePlotPanel::fit_all_bounds)** – utility to reset all
//!   scope axes to fit the current data.

use eframe::egui;

use crate::data::data::LivePlotData;

use super::LivePlotPanel;

impl LivePlotPanel {
    /// Main per-frame update: ingest data, render menu / side panels, then draw the plot.
    ///
    /// Call this from an egui `Ui` context each frame.  In standalone mode it is
    /// called by [`LivePlotApp::update`](super::LivePlotApp); in embedded mode the host
    /// application calls it directly (or via [`update_embedded`](Self::update_embedded)).
    pub fn update(&mut self, ui: &mut egui::Ui) {
        // Capture the full widget size BEFORE any layout (top bar, sidebars, etc.)
        // is applied.  This is the total area available to the entire plot widget
        // and is used for responsive min-width / min-height decisions.
        self.last_plot_size = ui.max_rect().size();

        self.update_data();

        // Propagate the event controller to scope panels (handles new scopes too).
        self.liveplot_panel
            .set_event_controller(self.event_ctrl.clone());

        // Propagate the total widget size to every scope panel so their tick-label
        // hide decisions also use the complete widget dimensions.
        self.liveplot_panel
            .set_total_widget_size(self.last_plot_size);

        // ── Emit resize event if size changed ─────────────────────────────
        if let Some(ctrl) = &self.event_ctrl {
            let cur_size = [self.last_plot_size.x, self.last_plot_size.y];
            let size_changed = {
                let inner = ctrl.inner.lock().unwrap();
                inner.last_size.map_or(true, |prev| {
                    (prev[0] - cur_size[0]).abs() > 0.5 || (prev[1] - cur_size[1]).abs() > 0.5
                })
            };
            if size_changed {
                {
                    let mut inner = ctrl.inner.lock().unwrap();
                    inner.last_size = Some(cur_size);
                }
                let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::RESIZE);
                evt.resize = Some(crate::events::ResizeMeta {
                    width: cur_size[0],
                    height: cur_size[1],
                });
                ctrl.emit_filtered(evt);
            }
        }

        // ── Emit key-press events ─────────────────────────────────────────
        if let Some(ctrl) = &self.event_ctrl {
            let keys: Vec<(String, crate::events::KeyModifiers)> = ui.ctx().input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => Some((
                            format!("{:?}", key),
                            crate::events::KeyModifiers {
                                ctrl: modifiers.ctrl,
                                alt: modifiers.alt,
                                shift: modifiers.shift,
                                command: modifiers.command,
                            },
                        )),
                        _ => None,
                    })
                    .collect()
            });
            for (key, mods) in keys {
                let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::KEY_PRESSED);
                evt.key_press = Some(crate::events::KeyPressMeta {
                    key,
                    modifiers: mods,
                });
                ctrl.emit_filtered(evt);
            }
        }

        // In compact mode, skip all chrome (menu bar, sidebars, bottom panels)
        // so the plot fills the entire allocated area.  This avoids collapsed
        // panel stubs stealing space from very small embedded cells.
        if !self.compact {
            self.render_menu(ui);
            self.render_panels(ui);
        }

        // Render the central plot area with overlay support from sub-panels.
        let central_panel = egui::CentralPanel::default();
        let central_panel = if self.compact {
            central_panel.frame(egui::Frame::NONE)
        } else {
            central_panel
        };
        central_panel.show_inside(ui, |ui| {
            use std::cell::RefCell;
            // Temporarily take panel lists to build a local overlay drawer
            // without borrowing `self` mutably (needed because the liveplot
            // render callback borrows traces_data through self).
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

            // Return panel lists back to self.
            self.left_side_panels = left.into_inner();
            self.right_side_panels = right.into_inner();
            self.bottom_panels = bottom.into_inner();
            self.detached_panels = detached.into_inner();
            self.empty_panels = empty.into_inner();

            self.traces_data.hover_trace = None;
        });
    }

    /// Update and render the panel when embedded in a parent app, then apply controllers.
    ///
    /// This is the convenience entry point for embedded use: it calls
    /// [`update`](Self::update) followed by
    /// [`apply_controllers_embedded`](Self::apply_controllers_embedded).
    pub fn update_embedded(&mut self, ui: &mut egui::Ui) {
        self.update(ui);
        self.apply_controllers_embedded(ui.ctx());
    }

    /// Programmatically trigger "Fit to View" (both X and Y axes) on every scope.
    ///
    /// Call this e.g. after a window resize to ensure all plots fill their bounds.
    pub fn fit_all_bounds(&mut self) {
        for scope in self.liveplot_panel.get_data_mut() {
            scope.fit_bounds(&self.traces_data);
        }
    }

    /// Ingest new data from the command channel, refresh all sub-panels, and
    /// evaluate threshold/trigger logic.
    ///
    /// Called at the start of every frame before any rendering.
    pub(crate) fn update_data(&mut self) {
        // Process incoming plot commands; collect any newly created traces.
        let new_traces = self.traces_data.update();

        // ── Emit data-update event when new traces arrive ─────────────────
        if !new_traces.is_empty() {
            if let Some(ctrl) = &self.event_ctrl {
                let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::DATA_UPDATED);
                evt.data_update = Some(crate::events::DataUpdateMeta {
                    traces: new_traces.clone(),
                    new_point_count: 0,
                });
                ctrl.emit_filtered(evt);
            }
        }

        // Apply any queued threshold add/remove requests before processing data so new defs
        // participate in this frame's evaluation.
        self.apply_threshold_controller_requests();

        self.liveplot_panel.update_data(&self.traces_data);
        let data = &mut LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
            pending_requests: &mut self.pending_requests,
            event_ctrl: self.event_ctrl.clone(),
        };

        // Attach newly created traces to the primary (first) scope only.
        if let Some(scope) = data.primary_scope_mut() {
            for name in new_traces {
                if !scope.trace_order.iter().any(|n| n == &name) {
                    scope.trace_order.push(name);
                }
            }
        }

        // Propagate data to every registered sub-panel.
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
}
