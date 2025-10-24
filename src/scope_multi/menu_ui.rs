//! Menu UI: top menu bar rendering for the multi-trace app.
//!
//! Contains the extracted menu rendering logic previously in `app.rs`.

use eframe::egui;
use crate::controllers::FFTPanelInfo;
use crate::controllers::{FFTDataRequest, FFTRawData, RawExportFormat, WindowInfo};
use super::LivePlotApp;

impl LivePlotApp {
    /// Render the top menu bar and return true if any bottom-panel visibility changed
    /// that requires notifying external controllers.
    pub(super) fn render_menu_bar(&mut self, ctx: &egui::Context) -> bool {
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
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "⊗ Off").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "⬌ X-Axis").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "⬍ Y-Axis").clicked() { ui.close(); }
                        if ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "🕂 Both").clicked() { ui.close(); }
                    });

                    ui.separator();
                    // Render the time-window control (was in the main toolbar)
                    self.render_time_window_control(ui);

                    // Points / Fit (X) / Pause controls: moved from the main toolbar into the View menu
                    ui.horizontal(|ui| {
                        ui.label("〰 Points:");
                        ui.add(egui::Slider::new(&mut self.max_points, 1000..=200_000));

                        if ui.button("Fit").on_hover_text("Fit the X-axis to the visible data").clicked() {
                            self.pending_auto_x = true;
                        }

                        ui.separator();

                        if ui.button(if self.paused { "⏵ Resume" } else { "◼ Pause" }).clicked() {
                            if self.paused {
                                self.paused = false;
                                for tr in self.traces.values_mut() { tr.snap = None; }
                            } else {
                                for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); }
                                self.paused = true;
                            }
                        }
                    });

                    ui.separator();
                    // Y-axis controls moved here (as-is from controls_ui)
                    let mut y_min_tmp = self.y_min;
                    let mut y_max_tmp = self.y_max;
                    let y_range = y_max_tmp - y_min_tmp;

                    ui.horizontal(|ui| {
                        ui.label("⬍ Y-Axis Min:");
                        let r1 = ui.add(egui::DragValue::new(&mut y_min_tmp).speed(0.1).custom_formatter(|n, _| {
                            if let Some(unit) = &self.y_unit {
                                if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                            } else {
                                if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{}", n / 10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                            }
                        }));

                        ui.separator();

                        ui.label("Max:");
                        let r2 = ui.add(egui::DragValue::new(&mut y_max_tmp).speed(0.1).custom_formatter(|n, _| {
                            if let Some(unit) = &self.y_unit {
                                if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{} {}", n / 10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                            } else {
                                if y_range.abs() < 0.001 { let exponent = y_range.log10().floor() + 1.0; format!("{:.1}e{}", n / 10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                            }
                        }));

                        if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp {
                            self.y_min = y_min_tmp;
                            self.y_max = y_max_tmp;
                            self.pending_auto_y = false;
                        }
                    });

                    if ui.button("⬍ Fit Y-Axis").on_hover_text("Fit the Y-axis to the visible data").clicked() { self.pending_auto_y = true; }

                    ui.separator();

                    let mut az = self.auto_zoom_y;
                    if ui.checkbox(&mut az, "🕂 Fit to view continously").on_hover_text("Continuously fit the Y-axis to the currently visible data range").changed() {
                        self.auto_zoom_y = az;
                        if self.auto_zoom_y { self.pending_auto_y = true; }
                    }

                    if ui.button("🕂 Fit to View").on_hover_text("Fit the view to the available data").clicked() {
                        self.pending_auto_x = true;
                        self.pending_auto_y = true;
                        ui.close();
                    }
                    ui.separator();
                    // Legend toggle moved from controls into View menu
                    if ui.checkbox(&mut self.show_legend, "Legend").on_hover_text("Show legend").changed() {
                        // no extra action required
                    }
                });
                ui.menu_button("📈 Traces", |ui| {
                    if ui.button("⊗ Clear markers").clicked() {
                        self.point_selection.clear();
                        ui.close();
                    }
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
                ui.menu_button("⚙ Settings", |ui| {
                    if ui.button("Hotkeys").on_hover_text("Configure keyboard shortcuts").clicked() {
                        self.hotkeys_dialog_open = true;
                        ui.close();
                    }
                });
                ui.menu_button("➰ Extras", |ui| {
                    if ui.button("⊗ Clear all traces").on_hover_text("Clear all trace data, math computations, point selections, and threshold events").clicked() {
                        for tr in self.traces.values_mut() { tr.live.clear(); if let Some(s) = &mut tr.snap { s.clear(); } }
                        self.reset_all_math_storage();
                        self.point_selection.clear();
                        self.clear_all_threshold_events();
                        ui.close();
                    }
                });
            });
        });
        did_toggle_bottom_panel
    }
}
