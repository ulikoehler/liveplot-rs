//! UI composition and top-level controls for `ScopeAppMulti`.
//!
//! This module contains the egui widget layout for the toolbar/panels and utility handlers
//! for dialogs, screenshots, window controller updates, and controller-driven actions.

use eframe::egui;
use image::{Rgba, RgbaImage};
use std::time::Duration;

#[cfg(feature = "fft")]
use crate::controllers::FFTPanelInfo;
use crate::controllers::{FFTDataRequest, FFTRawData, RawExportFormat, WindowInfo};
use crate::thresholds::ThresholdEvent;

use super::panel::DockPanel;
use super::ScopeAppMulti;

impl ScopeAppMulti {
    /// Return references to right-side dockable panels.
    pub(super) fn side_panels(&mut self) -> Vec<&mut dyn DockPanel> {
        vec![&mut self.traces_panel, &mut self.math_panel, &mut self.thresholds_panel]
    }

    /// Render the right-side sidebar if any attached panel is visible; includes header and body.
    pub(super) fn render_right_sidebar_panel(&mut self, ctx: &egui::Context) {
        let sidebar_visible = {
            let mut panels = self.side_panels();
            panels.iter_mut().any(|p| {
                let d = p.dock_mut();
                !d.detached && d.show_dialog
            })
        };
        if !sidebar_visible { return; }
        egui::SidePanel::right("right_tabs")
            .resizable(true)
            .default_width(350.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let mut clicked_idx: Option<usize> = None;
                    let titles_flags: Vec<(&'static str, bool)> = {
                        let mut panels = self.side_panels();
                        panels.iter_mut().map(|p| { let d = p.dock_mut(); (d.title, !d.detached && d.show_dialog) }).collect()
                    };
                    if titles_flags.len() == 1 { ui.strong(titles_flags[0].0); } else {
                        for (i, (title, active)) in titles_flags.iter().enumerate() {
                            if ui.selectable_label(*active, *title).clicked() { clicked_idx = Some(i); }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Hide").on_hover_text("Hide the sidebar").clicked() {
                            let mut panels = self.side_panels();
                            for p in panels.iter_mut() { let d = p.dock_mut(); if !d.detached { d.show_dialog = false; } }
                        }
                        let any_attached = {
                            let mut panels = self.side_panels();
                            panels.iter_mut().any(|p| { let d = p.dock_mut(); !d.detached && d.show_dialog })
                        };
                        if any_attached {
                            if ui.button("Pop out").on_hover_text("Open attached panel in a floating window").clicked() {
                                let mut panels = self.side_panels();
                                for p in panels.iter_mut() { let d = p.dock_mut(); if !d.detached && d.show_dialog { d.detached = true; d.show_dialog = true; } }
                            }
                        }
                    });
                    if let Some(i) = clicked_idx {
                        let mut panels = self.side_panels();
                        for (j, p) in panels.iter_mut().enumerate() {
                            let d = p.dock_mut();
                            if j == i { d.detached = false; d.show_dialog = true; } else if !d.detached { d.show_dialog = false; }
                        }
                    }
                });
                ui.separator();
                let active_idx = {
                    let mut panels = self.side_panels();
                    panels.iter_mut().position(|p| { let d = p.dock_mut(); !d.detached && d.show_dialog })
                };
                if let Some(i) = active_idx {
                    match i {
                        0 => { let mut panel = std::mem::take(&mut self.traces_panel); panel.panel_contents(self, ui); self.traces_panel = panel; }
                        1 => { let mut panel = std::mem::take(&mut self.math_panel); panel.panel_contents(self, ui); self.math_panel = panel; }
                        2 => { let mut panel = std::mem::take(&mut self.thresholds_panel); panel.panel_contents(self, ui); self.thresholds_panel = panel; }
                        _ => {}
                    }
                }
            });
    }

    /// Render export buttons (Save PNG screenshot and raw data export) into the given Ui.
    pub(super) fn render_export_buttons(&mut self, ui: &mut egui::Ui) {
        if ui.button("🖼 Save PNG").on_hover_text("Take a screenshot of the entire window").clicked() {
            self.request_window_shot = true;
        }
        ui.menu_button("📤 Export", |ui| {
            let hover_text_traces: &str = {
                #[cfg(feature = "parquet")] { "Export all traces as CSV or Parquet" }
                #[cfg(not(feature = "parquet"))] { "Export all traces as CSV" }
            };
            if ui.button("🗠 Traces").on_hover_text(hover_text_traces).clicked() { ui.close(); self.prompt_and_save_raw_data(); }
            if ui.button("⚠️ Threshold events").on_hover_text("Export filtered or all threshold events as CSV").clicked() { ui.close(); self.prompt_and_save_threshold_events(); }
        });
    }

    /// Show a file dialog and save raw data in the chosen format.
    pub(super) fn prompt_and_save_raw_data(&mut self) {
        let mut dlg = rfd::FileDialog::new();
        dlg = dlg.add_filter("CSV", &["csv"]);
        #[cfg(feature = "parquet")] { dlg = dlg.add_filter("Parquet", &["parquet"]); }
        if let Some(path) = dlg.set_file_name("liveplot_export.csv").save_file() {
            let fmt = {
                #[cfg(feature = "parquet")] {
                    match path.extension().and_then(|s| s.to_str()).unwrap_or("") { "parquet" => RawExportFormat::Parquet, _ => RawExportFormat::Csv }
                }
                #[cfg(not(feature = "parquet"))] { RawExportFormat::Csv }
            };
            if let Err(e) = super::export_helpers::save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) {
                eprintln!("Failed to save raw data: {e}");
            }
        }
    }

    /// Show a file dialog and export threshold events to CSV (respects current filter).
    pub(super) fn prompt_and_save_threshold_events(&mut self) {
        let evts: Vec<&ThresholdEvent> = self
            .threshold_event_log
            .iter()
            .rev()
            .filter(|e| self.thresholds_panel.events_filter.as_ref().map_or(true, |f| &e.threshold == f))
            .collect();
        if evts.is_empty() { return; }
        if let Some(path) = rfd::FileDialog::new().set_file_name("threshold_events.csv").add_filter("CSV", &["csv"]).save_file() {
            if let Err(e) = super::export_helpers::save_threshold_events_csv(&path, &evts) {
                eprintln!("Failed to export events CSV: {e}");
            }
        }
    }

    /// Handle pending screenshot request and save the resulting image to a chosen path or env path.
    pub(super) fn handle_screenshot_result(&mut self, ctx: &egui::Context) {
        if self.request_window_shot {
            self.request_window_shot = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| { if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None } })
        }) {
            self.last_viewport_capture = Some(image_arc.clone());
            if let Ok(path_str) = std::env::var("LIVEPLOT_SAVE_SCREENSHOT_TO") {
                std::env::remove_var("LIVEPLOT_SAVE_SCREENSHOT_TO");
                let path = std::path::PathBuf::from(path_str);
                let egui::ColorImage { size: [w, h], pixels, .. } = &*image_arc;
                let mut out = RgbaImage::new(*w as u32, *h as u32);
                for y in 0..*h { for x in 0..*w { let p = pixels[y * *w + x]; out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()])); } }
                if let Err(e) = out.save(&path) { eprintln!("Failed to save viewport screenshot: {e}"); } else { eprintln!("Saved viewport screenshot to {:?}", path); }
            } else {
                let default_name = format!("viewport_{:.0}.png", chrono::Local::now().timestamp_millis());
                if let Some(path) = rfd::FileDialog::new().set_file_name(&default_name).save_file() {
                    let egui::ColorImage { size: [w, h], pixels, .. } = &*image_arc;
                    let mut out = RgbaImage::new(*w as u32, *h as u32);
                    for y in 0..*h { for x in 0..*w { let p = pixels[y * *w + x]; out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()])); } }
                    if let Err(e) = out.save(&path) { eprintln!("Failed to save viewport screenshot: {e}"); } else { eprintln!("Saved viewport screenshot to {:?}", path); }
                }
            }
        }
    }

    /// Handle focus requests coming from detached panels and hide other attached panels.
    pub(super) fn process_focus_requests(&mut self) {
        let mut focus_idx: Option<usize> = None;
        {
            let mut panels = self.side_panels();
            for (i, p) in panels.iter_mut().enumerate() {
                if p.dock_mut().focus_dock { focus_idx = Some(i); break; }
            }
        }
        if let Some(i) = focus_idx {
            let mut panels = self.side_panels();
            for (j, p) in panels.iter_mut().enumerate() {
                let d = p.dock_mut();
                if j == i { d.focus_dock = false; d.detached = false; d.show_dialog = true; }
                else if !d.detached { d.show_dialog = false; }
            }
        }
    }

    /// Compose toolbar and controls; used by both main and embedded variants.
    pub(super) fn controls_ui(&mut self, ui: &mut egui::Ui, mode: super::app::ControlsMode) {
        match mode {
            super::app::ControlsMode::Main => { ui.heading("LivePlot (multi)"); ui.label("Left mouse: pan  |  Right drag: zoom box"); }
            super::app::ControlsMode::Embedded => { ui.label("LivePlot"); ui.label("Left mouse: pan  |  Right drag: zoom box"); }
        }
        ui.horizontal(|ui| {
            ui.label("X-Axis Time:");
            let mut tw = self.time_window;
            if !self.time_slider_dragging {
                if tw <= self.time_window_min { self.time_window_min = self.time_window_min / 10.0; self.time_window_max = self.time_window_max / 10.0; }
                else if tw >= self.time_window_max { self.time_window_min = self.time_window_min * 10.0; self.time_window_max = self.time_window_max * 10.0; }
            }
            let slider = egui::Slider::new(&mut tw, self.time_window_min..=self.time_window_max)
                .logarithmic(true).smart_aim(true).show_value(true).clamping(egui::SliderClamping::Edits).suffix(" s");
            let sresp = ui.add(slider);
            if sresp.changed() { self.time_window = tw; }
            self.time_slider_dragging = sresp.is_pointer_button_down_on();

            ui.label("Points:");
            ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));

            if ui.button("Fit").on_hover_text("Fit the X-axis to the visible data").clicked() { self.pending_auto_x = true; }

            ui.separator();

            let mut y_min_tmp = self.y_min;
            let mut y_max_tmp = self.y_max;
            let y_range = y_max_tmp - y_min_tmp;

            ui.label("Y-Axis Min:");
            let r1 = ui.add(egui::DragValue::new(&mut y_min_tmp).speed(0.1).custom_formatter(|n, _| {
                if let Some(unit) = &self.y_unit {
                    if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                } else {
                    if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{}", n / 10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                }
            }));
            ui.label("Max:");
            let r2 = ui.add(egui::DragValue::new(&mut y_max_tmp).speed(0.1).custom_formatter(|n, _| {
                if let Some(unit) = &self.y_unit {
                    if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                } else {
                    if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{}", n / 10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                }
            }));
            if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp { self.y_min = y_min_tmp; self.y_max = y_max_tmp; self.pending_auto_y = false; }

            if ui.button("Fit").on_hover_text("Fit the Y-axis to the visible data").clicked() { self.pending_auto_y = true; }

            ui.checkbox(&mut self.auto_zoom_y, "Auto Zoom").on_hover_text("Continuously fit the Y-axis to the currently visible data range");
            if self.auto_zoom_y { self.pending_auto_y = true; }

            ui.separator();

            // Zoom mode moved to the top-left "View" menu

            ui.separator();

            if ui.button("🕂 Fit to View").on_hover_text("Fit the view to the available data").clicked() { self.pending_auto_x = true; self.pending_auto_y = true; }

            ui.separator();

            if ui.button(if self.paused { "⏵ Resume" } else { "◼ Pause" }).clicked() {
                if self.paused { self.paused = false; for tr in self.traces.values_mut() { tr.snap = None; } }
                else { for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); } self.paused = true; }
            }

            if ui.button("Clear Measurement").clicked() { self.point_selection.clear(); }

            if ui.button("Clear All").clicked() {
                for tr in self.traces.values_mut() { tr.live.clear(); if let Some(s) = &mut tr.snap { s.clear(); } }
                self.reset_all_math_storage();
                self.point_selection.clear();
                self.clear_all_threshold_events();
            }

            ui.checkbox(&mut self.show_legend, "Legend").on_hover_text("Show legend");

            // Panel openers and export actions moved to the main menu bar (File / Functions)
        });
    }

    /// Render any open detached dialogs for right-side panels.
    pub(super) fn show_dialogs_shared(&mut self, ctx: &egui::Context) {
        {
            let mut panel = std::mem::take(&mut self.traces_panel);
            if { let d = panel.dock_mut(); d.detached && d.show_dialog } { panel.show_detached_dialog(self, ctx); }
            self.traces_panel = panel;
        }
        {
            let mut panel = std::mem::take(&mut self.math_panel);
            if { let d = panel.dock_mut(); d.detached && d.show_dialog } { panel.show_detached_dialog(self, ctx); }
            self.math_panel = panel;
        }
        {
            let mut panel = std::mem::take(&mut self.thresholds_panel);
            if { let d = panel.dock_mut(); d.detached && d.show_dialog } { panel.show_detached_dialog(self, ctx); }
            self.thresholds_panel = panel;
        }
        #[cfg(feature = "fft")]
        {
            let mut panel = std::mem::take(&mut self.fft_panel);
            if { let d = panel.dock_mut(); d.detached && d.show_dialog } { panel.show_detached_dialog(self, ctx); }
            self.fft_panel = panel;
        }
    }

    /// Bottom panels accessor (FFT etc.).
    pub(super) fn bottom_panels(&mut self) -> Vec<&mut dyn super::panel::DockPanel> {
        #[cfg(feature = "fft")] { vec![&mut self.fft_panel] }
        #[cfg(not(feature = "fft"))] { Vec::new() }
    }

    /// Update any external controllers about attached bottom-panel visibility (e.g., FFT).
    pub(super) fn update_bottom_panels_controller_visibility(&mut self) {
        #[cfg(feature = "fft")] {
            if let Some(ctrl) = &self.fft_controller {
                let d = self.fft_panel.dock.clone();
                let mut inner = ctrl.inner.lock().unwrap();
                inner.show = d.show_dialog && !d.detached;
                let info = FFTPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
                inner.listeners.retain(|s| s.send(info.clone()).is_ok());
            }
        }
    }

    /// Call a closure with the bottom panel at the given index temporarily moved out, then put it back.
    pub(super) fn with_bottom_panel_at<F>(&mut self, index: usize, mut f: F)
    where F: FnMut(&mut dyn super::panel::DockPanel, &mut Self), {
        #[cfg(feature = "fft")] {
            if index == 0 {
                let mut p = std::mem::take(&mut self.fft_panel);
                f(&mut p, self);
                self.fft_panel = p;
                return;
            }
        }
        let _ = index; let _ = &f; // no-op for unknown index or non-fft builds
    }

    /// Render the bottom panel container if any attached bottom panel is visible; includes header and body.
    pub(super) fn render_bottom_panel(&mut self, ctx: &egui::Context) {
        let visible = { let mut panels = self.bottom_panels(); panels.iter_mut().any(|p| { let d = p.dock_mut(); !d.detached && d.show_dialog }) };
        if !visible { return; }
        egui::TopBottomPanel::bottom("bottom_panels").resizable(true).default_height(300.0).min_height(120.0).show(ctx, |ui| {
            let titles_flags: Vec<(&'static str, bool)> = { let mut panels = self.bottom_panels(); panels.iter_mut().map(|p| { let d = p.dock_mut(); (d.title, !d.detached && d.show_dialog) }).collect() };
            let active_idx_current = titles_flags.iter().position(|(_, active)| *active);
            let mut clicked_idx: Option<usize> = None;
            ui.horizontal(|ui| {
                if titles_flags.len() == 1 { ui.strong(titles_flags[0].0); } else {
                    for (i, (title, active)) in titles_flags.iter().enumerate() {
                        if ui.selectable_label(*active, *title).clicked() { clicked_idx = Some(i); }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ai) = active_idx_current {
                        if ui.button("Pop out").clicked() { self.with_bottom_panel_at(ai, |p, app| { let d = p.dock_mut(); d.detached = true; d.show_dialog = true; app.update_bottom_panels_controller_visibility(); }); }
                        if ui.button("Hide").clicked() { self.with_bottom_panel_at(ai, |p, app| { let d = p.dock_mut(); d.show_dialog = false; app.update_bottom_panels_controller_visibility(); }); }
                    }
                });
            });
            if let Some(i) = clicked_idx {
                let mut panels = self.bottom_panels();
                for (j, p) in panels.iter_mut().enumerate() {
                    let d = p.dock_mut();
                    if j == i { d.show_dialog = true; d.detached = false; } else if !d.detached { d.show_dialog = false; }
                }
            }
            ui.separator();
            let active_idx: Option<usize> = { let mut panels = self.bottom_panels(); panels.iter_mut().position(|p| { let d = p.dock_mut(); !d.detached && d.show_dialog }) };
            if let Some(i) = active_idx { self.with_bottom_panel_at(i, |p, app| { let show_attached = { let d = p.dock_mut(); !d.detached && d.show_dialog }; if show_attached { p.panel_contents(app, ui); } }); }
        });
    }

    /// Apply any pending UI action controller requests (pause/resume/screenshot/raw save, FFT data).
    pub(super) fn handle_ui_action_requests(&mut self) {
        if let Some(ctrl) = &self.ui_action_controller {
            let mut inner = ctrl.inner.lock().unwrap();
            if let Some(want_pause) = inner.request_pause.take() {
                if want_pause && !self.paused { for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); } self.paused = true; }
                else if !want_pause && self.paused { self.paused = false; for tr in self.traces.values_mut() { tr.snap = None; } }
            }
            if inner.request_screenshot { inner.request_screenshot = false; self.request_window_shot = true; }
            if let Some(path) = inner.request_screenshot_to.take() {
                self.request_window_shot = true;
                drop(inner);
                std::env::set_var("LIVEPLOT_SAVE_SCREENSHOT_TO", path);
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(fmt) = inner.request_save_raw.take() {
                drop(inner);
                let mut dlg = rfd::FileDialog::new();
                dlg = dlg.add_filter("CSV", &["csv"]);
                #[cfg(feature = "parquet")] { dlg = dlg.add_filter("Parquet", &["parquet"]); }
                if let Some(path) = dlg.save_file() {
                    if let Err(e) = super::export_helpers::save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) {
                        eprintln!("Failed to save raw data: {e}");
                    }
                }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some((fmt, path)) = inner.request_save_raw_to.take() {
                drop(inner);
                if let Err(e) = super::export_helpers::save_raw_data_to_path(fmt, &path, self.paused, &self.traces, &self.trace_order) {
                    eprintln!("Failed to save raw data: {e}");
                }
                inner = ctrl.inner.lock().unwrap();
            }
            if let Some(req) = inner.fft_request.take() {
                let name_opt = match req { FFTDataRequest::CurrentTrace => self.selection_trace.clone(), FFTDataRequest::NamedTrace(s) => Some(s) };
                if let Some(name) = name_opt { if let Some(tr) = self.traces.get(&name) {
                    let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused { if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
                    let data: Vec<[f64; 2]> = iter.cloned().collect();
                    let msg = FFTRawData { trace: name.clone(), data };
                    inner.fft_listeners.retain(|s| s.send(msg.clone()).is_ok());
                } }
            }
        }
    }

    /// Publish current window info and apply any pending viewport requests from the window controller.
    pub(super) fn handle_window_controller_requests(&mut self, ctx: &egui::Context) {
        if let Some(ctrl) = &self.window_controller {
            let rect = ctx.input(|i| i.content_rect());
            let ppp = ctx.pixels_per_point();
            let mut inner = ctrl.inner.lock().unwrap();
            let size_pts = rect.size();
            inner.current_size = Some([size_pts.x * ppp, size_pts.y * ppp]);
            inner.current_pos = Some([rect.min.x * ppp, rect.min.y * ppp]);
            if let Some(size_px) = inner.request_set_size.take() {
                let size_pts = [size_px[0] / ppp, size_px[1] / ppp];
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size_pts.into()));
            }
            if let Some(pos_px) = inner.request_set_pos.take() {
                let pos_pts = [pos_px[0] / ppp, pos_px[1] / ppp];
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos_pts.into()));
            }
            let info = WindowInfo { current_size: inner.current_size, current_pos: inner.current_pos, requested_size: inner.request_set_size, requested_pos: inner.request_set_pos };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }

    /// Request a continuous repaint at ~60 FPS.
    pub(super) fn repaint_tick(ctx: &egui::Context) { ctx.request_repaint_after(Duration::from_millis(16)); }

    /// Render the LivePlot UI into an arbitrary egui container (e.g., inside an egui::Window).
    pub fn ui_embed(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.tick_non_ui();
        ui.vertical(|ui| { self.controls_ui(ui, super::app::ControlsMode::Embedded); });
        self.show_dialogs_shared(&ctx);
        let plot_response = self.plot_traces_common(ui, &ctx, "scope_plot_multi_embedded");
        self.pause_on_click(&plot_response);
        self.apply_zoom(&plot_response);
        self.handle_plot_click(&plot_response);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
