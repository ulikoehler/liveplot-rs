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
use crate::data::data::ScreenshotRequest;
use crate::TraceRef;

use super::LivePlotPanel;

impl LivePlotPanel {
    fn screenshot_default_name(&self, multi_scope: bool) -> String {
        let prefix = if multi_scope { "scopes" } else { "scope" };
        format!(
            "{}_{:.0}.png",
            prefix,
            chrono::Local::now().timestamp_millis()
        )
    }

    fn sanitize_screenshot_name(name: &str) -> String {
        let sanitized: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        sanitized.trim_matches('_').to_string()
    }

    fn build_image_from_screenshot(image: &egui::ColorImage) -> image::RgbaImage {
        let [w, h] = image.size;
        let mut out = image::RgbaImage::new(w as u32, h as u32);
        for y in 0..h {
            for x in 0..w {
                let p = image.pixels[y * w + x];
                out.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgba([p.r(), p.g(), p.b(), p.a()]),
                );
            }
        }
        out
    }

    fn handle_completed_screenshot(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_screenshot_capture.clone() else {
            return;
        };
        let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        }) else {
            return;
        };
        self.pending_screenshot_capture = None;

        if pending.targets.is_empty() {
            return;
        }

        let viewport_image = Self::build_image_from_screenshot(&image_arc);
        let base_path = match pending.path.clone() {
            Some(path) => Some(path),
            None => rfd::FileDialog::new()
                .set_file_name(&self.screenshot_default_name(pending.targets.len() > 1))
                .add_filter("PNG", &["png"])
                .save_file(),
        };
        let Some(base_path) = base_path else {
            return;
        };

        let ext = base_path
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.is_empty())
            .unwrap_or("png")
            .to_string();
        let stem = base_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or("screenshot")
            .to_string();
        let parent = base_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        for (idx, target) in pending.targets.iter().enumerate() {
            let left = ((target.rect[0] - pending.content_origin[0]) * pending.pixels_per_point)
                .floor()
                .max(0.0) as u32;
            let top = ((target.rect[1] - pending.content_origin[1]) * pending.pixels_per_point)
                .floor()
                .max(0.0) as u32;
            let right = ((target.rect[2] - pending.content_origin[0]) * pending.pixels_per_point)
                .ceil()
                .min(viewport_image.width() as f32) as u32;
            let bottom = ((target.rect[3] - pending.content_origin[1]) * pending.pixels_per_point)
                .ceil()
                .min(viewport_image.height() as f32) as u32;
            if right <= left || bottom <= top {
                continue;
            }

            let cropped =
                image::imageops::crop_imm(&viewport_image, left, top, right - left, bottom - top)
                    .to_image();

            let output_path = if pending.targets.len() == 1 {
                base_path.clone()
            } else {
                let scope_name = Self::sanitize_screenshot_name(&target.scope_name);
                let suffix = if scope_name.is_empty() {
                    format!("scope_{}", target.scope_id + 1)
                } else {
                    scope_name
                };
                parent.join(format!("{stem}__{suffix}_{idx}.{ext}"))
            };

            match cropped.save(&output_path) {
                Ok(()) => {
                    if let Some(ctrl) = &self.event_ctrl {
                        let mut event =
                            crate::events::PlotEvent::new(crate::events::EventKind::SCREENSHOT);
                        event.export = Some(crate::events::ExportMeta {
                            format: "png".to_string(),
                            path: Some(output_path.to_string_lossy().to_string()),
                        });
                        ctrl.emit_filtered(event);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to save scope screenshot: {err}");
                }
            }
        }
    }

    fn queue_screenshot_capture(&mut self, ctx: &egui::Context, request: ScreenshotRequest) {
        if self.pending_screenshot_capture.is_some() {
            self.pending_requests.screenshot = Some(request);
            return;
        }

        let expand_scope_rect =
            |rect: [f32; 4], show_x_axis_label: bool, show_y_axis_label: bool| {
                let mut out = rect;
                let x_pad = if show_y_axis_label { 56.0 } else { 28.0 };
                let y_pad = if show_x_axis_label { 44.0 } else { 20.0 };
                out[0] -= x_pad;
                out[1] -= 10.0;
                out[2] += 16.0;
                out[3] += y_pad;
                out
            };

        let targets = match request.target {
            crate::data::data::ScreenshotTarget::CenterPanel => {
                vec![super::ScreenshotCropTarget {
                    scope_id: usize::MAX,
                    scope_name: "center_panel".to_string(),
                    rect: self.last_widget_rect,
                }]
            }
            crate::data::data::ScreenshotTarget::ScopeRect {
                scope_id,
                scope_name,
                rect,
                show_x_axis_label,
                show_y_axis_label,
            } => {
                vec![super::ScreenshotCropTarget {
                    scope_id,
                    scope_name,
                    rect: expand_scope_rect(rect, show_x_axis_label, show_y_axis_label),
                }]
            }
            _ => self.liveplot_panel.screenshot_targets(&request.target),
        };
        if targets.is_empty() {
            return;
        }

        let content_rect = ctx.input(|i| i.content_rect());
        self.pending_screenshot_capture = Some(super::PendingScreenshotCapture {
            targets,
            path: request.path,
            pixels_per_point: ctx.pixels_per_point(),
            content_origin: [content_rect.left(), content_rect.top()],
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }

    /// Main per-frame update: ingest data, render menu / side panels, then draw the plot.
    ///
    /// Call this from an egui `Ui` context each frame.  In standalone mode it is
    /// called by [`LivePlotApp::update`](super::LivePlotApp); in embedded mode the host
    /// application calls it directly (or via [`update_embedded`](Self::update_embedded)).
    pub fn update(&mut self, ui: &mut egui::Ui) {
        // Use `push_id` to ensure that all widgets created by this panel instance
        // stay isolated from other panels (critical for embedded tile dashboards).
        ui.push_id(self.panel_id, |ui| {
            let widget_rect = ui.max_rect();
            self.last_widget_rect = [
                widget_rect.left(),
                widget_rect.top(),
                widget_rect.right(),
                widget_rect.bottom(),
            ];

            // Capture the full widget size BEFORE any layout (top bar, sidebars, etc.)
            // is applied.  This is the total area available to the entire plot widget
            // and is used for responsive min-width / min-height decisions.
            self.last_plot_size = widget_rect.size();

            self.update_data();
            self.handle_completed_screenshot(ui.ctx());

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
                    let mut evt =
                        crate::events::PlotEvent::new(crate::events::EventKind::KEY_PRESSED);
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
                        // invoke optional user overlay callback after panel overlays
                        if let Some(cb) = &mut self.overlays {
                            cb(plot_ui, scope, traces);
                        }
                    };

                // Render the liveplot panel; `draw_overlays` supplies per-panel overlays.
                self.liveplot_panel.clear_rendered_flags();
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

            // Collect any pending view changes from scope panels (zoom/pan/slider/fit).
            if let Some(vc) = self.liveplot_panel.collect_view_changes() {
                self.pending_view_change = Some(vc);
            }

            let screenshot_request = self
                .liveplot_panel
                .take_scope_screenshot_request()
                .or_else(|| self.pending_requests.screenshot.take());
            if let Some(request) = screenshot_request {
                self.queue_screenshot_capture(ui.ctx(), request);
            }
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
            scope.fit_bounds(&self.traces_data, false);
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

        // Collect existing trace names only when traces were registered
        // externally (via update_background) since the last update_data call.
        // This avoids O(traces × trace_order) work every frame.
        let all_trace_names: Vec<TraceRef> = if self.traces_dirty {
            self.traces_data.keys().cloned().collect()
        } else {
            Vec::new()
        };

        self.liveplot_panel.update_data(&self.traces_data);
        let data = &mut LivePlotData {
            scope_data: self.liveplot_panel.get_data_mut(),
            traces: &mut self.traces_data,
            pending_requests: &mut self.pending_requests,
            event_ctrl: self.event_ctrl.clone(),
        };

        // Attach newly created traces to the primary (first) scope only.
        if let Some(scope) = data.primary_scope_mut() {
            for name in new_traces.into_iter().chain(all_trace_names) {
                if !scope.trace_order.iter().any(|n| n == &name) {
                    scope.trace_order.push(name);
                }
            }
        }
        self.traces_dirty = false;

        // Propagate data to every registered sub-panel.
        // Skip invisible panels to avoid unnecessary work (e.g. FFT computation).
        for p in &mut self.left_side_panels {
            if p.state().visible {
                p.update_data(data);
            }
        }
        for p in &mut self.right_side_panels {
            if p.state().visible {
                p.update_data(data);
            }
        }
        for p in &mut self.bottom_panels {
            if p.state().visible {
                p.update_data(data);
            }
        }
        for p in &mut self.detached_panels {
            if p.state().visible {
                p.update_data(data);
            }
        }
        for p in &mut self.empty_panels {
            if p.state().visible {
                p.update_data(data);
            }
        }

        // After threshold processing, forward freshly generated events to controller listeners.
        self.publish_threshold_events();
    }
}
