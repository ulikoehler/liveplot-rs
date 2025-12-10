use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::measurement::Measurement;
use egui::{Align2, Color32};
use egui_plot::{Line, PlotPoint, Points, Text};

pub struct MeasurementPanel {
    state: PanelState,
    measurements: Vec<Measurement>,
    selected_measurement: Option<usize>,
    selected_point_index: Option<usize>,
    last_clicked_point: Option<[f64; 2]>,
    hovered_measurement: Option<usize>,
}

impl Default for MeasurementPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Measurement", "📏"),
            measurements: vec![Measurement::new("M1")],
            selected_measurement: Some(0),
            selected_point_index: None,
            last_clicked_point: None,
            hovered_measurement: None,
        }
    }
}

impl Panel for MeasurementPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn clear_all(&mut self) {
        self.measurements.clear();
        self.selected_measurement = None;
        self.selected_point_index = None;
        self.last_clicked_point = None;
        self.hovered_measurement = None;
    }

    fn render_menu(&mut self, ui: &mut egui::Ui, data: &mut LivePlotData<'_>) {
        ui.menu_button("📏 Measurements", |ui| {
            if ui.button("New measurement").clicked() {
                let idx = self.measurements.len() + 1;
                self.measurements
                    .push(Measurement::new(&format!("M{}", idx)));
                // Focus this panel
                let st = self.state_mut();
                st.visible = true;
                st.detached = false;
                st.request_docket = true;
                ui.close();
            }
            if ui.button("X Clear All").clicked() {
                for m in &mut self.measurements {
                    m.clear();
                }
                data.scope_data.clicked_point = None;
                ui.close();
            }
            if ui.button("Take P1 at click").clicked() {
                self.selected_point_index = Some(0);
                ui.close();
            }
            if ui.button("Take P2 at click").clicked() {
                self.selected_point_index = Some(1);
                ui.close();
            }
        });
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        if let Some(point) = data.scope_data.clicked_point {
            if self.last_clicked_point == Some(point) {
                return;
            }
            self.last_clicked_point = Some(point);

            if self.measurements.is_empty() {
                self.measurements.push(Measurement::default());
            }

            // Choose target measurement index safely to avoid borrow conflicts
            let target_idx = if let Some(idx) = self.selected_measurement {
                if idx < self.measurements.len() {
                    idx
                } else {
                    self.selected_measurement = None;
                    0
                }
            } else {
                0
            };
            let measurement = self.measurements.get_mut(target_idx).unwrap();

            let sel_data_points: Option<Vec<[f64; 2]>> =
                if let Some(name) = &measurement.catch_trace {
                    data.traces
                        .get_points(name, data.scope_data.paused)
                        .map(|v| v.into_iter().collect())
                } else {
                    None
                };

            let point = match (&measurement.catch_trace, &sel_data_points) {
                (Some(name), Some(data_points)) if !data_points.is_empty() => {
                    let off = data.traces.get_trace(name).map(|t| t.offset).unwrap_or(0.0);
                    let mut best_i = None;
                    let mut best_d2 = f64::INFINITY;
                    for (i, p) in data_points.iter().enumerate() {
                        let x_plot = p[0];
                        let y_plot = p[1] + off;
                        let dx = x_plot - point[0];
                        let dy = y_plot - point[1];
                        let d2 = dx * dx + dy * dy;
                        if d2 < best_d2 {
                            best_d2 = d2;
                            best_i = Some(i);
                        }
                    }
                    if let Some(i) = best_i {
                        data_points[i]
                    } else {
                        point
                    }
                }
                _ => point,
            };

            if let Some(point_idx) = self.selected_point_index {
                match point_idx {
                    0 => measurement.set_point1(point),
                    1 => measurement.set_point2(point),
                    _ => measurement.set_point(point),
                }
                self.selected_point_index = None;
            } else {
                measurement.set_point(point);
            }
        }
    }

    fn draw(
        &mut self,
        plot_ui: &mut egui_plot::PlotUi,
        scope: &crate::data::scope::ScopeData,
        _traces: &crate::data::traces::TracesCollection,
    ) {
        // Measurement overlays
        let base_body = plot_ui.ctx().style().text_styles[&egui::TextStyle::Body].size;
        let marker_font_size = base_body * 1.5;

        let hovered_idx = self.hovered_measurement;
        for (mi, measurement) in self.measurements.iter().enumerate() {
            let name = measurement.name.clone();
            let (p1_opt, p2_opt) = measurement.get_points();
            let dimmed = if let Some(h) = hovered_idx {
                h != mi
            } else {
                false
            };
            let (c_p1, c_p2, c_line) = if dimmed {
                let dim = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 60);
                (
                    dim(Color32::YELLOW),
                    dim(Color32::LIGHT_BLUE),
                    dim(Color32::LIGHT_GREEN),
                )
            } else {
                (Color32::YELLOW, Color32::LIGHT_BLUE, Color32::LIGHT_GREEN)
            };

            let (x_min_lin, x_max_lin) = scope.x_axis.bounds;
            let (y_min_lin, y_max_lin) = scope.y_axis.bounds;
            let x_min_plot = if scope.x_axis.log_scale && x_min_lin > 0.0 {
                x_min_lin.log10()
            } else {
                x_min_lin
            };
            let x_max_plot = if scope.x_axis.log_scale && x_max_lin > 0.0 {
                x_max_lin.log10()
            } else {
                x_max_lin
            };
            let y_min_plot = if scope.y_axis.log_scale && y_min_lin > 0.0 {
                y_min_lin.log10()
            } else {
                y_min_lin
            };
            let y_max_plot = if scope.y_axis.log_scale && y_max_lin > 0.0 {
                y_max_lin.log10()
            } else {
                y_max_lin
            };
            let ox = 0.01 * (x_max_plot - x_min_plot);
            let oy = 0.01 * (y_max_plot - y_min_plot);

            let (dx, dy) = if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                (p2[0] - p1[0], p2[1] - p1[1])
            } else {
                (0.0, 0.0)
            };

            let label_pos = |dx: f64,
                             dy: f64,
                             p: &[f64; 2],
                             ox: f64,
                             oy: f64|
             -> (Align2, egui::Align, PlotPoint) {
                let slope = if dx != 0.0 || oy != 0.0 || ox != 0.0 {
                    (dy / oy) / (dx / ox)
                } else {
                    0.0
                };
                if dx <= 0.0 || slope.abs() > 8.0 {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (
                            Align2::LEFT_TOP,
                            egui::Align::LEFT,
                            PlotPoint::new(p[0] + ox, p[1] - oy),
                        )
                    } else {
                        (
                            Align2::LEFT_BOTTOM,
                            egui::Align::LEFT,
                            PlotPoint::new(p[0] + ox, p[1] + oy),
                        )
                    }
                } else {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (
                            Align2::RIGHT_TOP,
                            egui::Align::RIGHT,
                            PlotPoint::new(p[0] - ox, p[1] - oy),
                        )
                    } else {
                        (
                            Align2::RIGHT_BOTTOM,
                            egui::Align::RIGHT,
                            PlotPoint::new(p[0] - ox, p[1] + oy),
                        )
                    }
                }
            };

            if let Some(p) = p1_opt {
                plot_ui.points(Points::new(&name, vec![p]).radius(5.0).color(c_p1));
                let (halign_anchor, text_align, base) = label_pos(dx, dy, &p, ox, oy);
                let x_lin = if scope.x_axis.log_scale {
                    10f64.powf(p[0])
                } else {
                    p[0]
                };
                let y_lin = if scope.y_axis.log_scale {
                    10f64.powf(p[1])
                } else {
                    p[1]
                };
                let x_range = (x_max_lin - x_min_lin).abs();
                let y_range = (y_max_lin - y_min_lin).abs();
                let x_txt = scope.x_axis.format_value(x_lin, 6, x_range);
                let y_txt = scope.y_axis.format_value_with_unit(y_lin, 6, y_range);
                let txt = format!("P1\nx = {}\ny = {}", x_txt, y_txt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(c_p1)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new(&name, base, job).anchor(halign_anchor));
            }
            if let Some(p) = p2_opt {
                plot_ui.points(Points::new(&name, vec![p]).radius(5.0).color(c_p2));
                let (halign_anchor, text_align, base) = label_pos(-dx, -dy, &p, ox, oy);
                let x_lin = if scope.x_axis.log_scale {
                    10f64.powf(p[0])
                } else {
                    p[0]
                };
                let y_lin = if scope.y_axis.log_scale {
                    10f64.powf(p[1])
                } else {
                    p[1]
                };
                let x_range = (x_max_lin - x_min_lin).abs();
                let y_range = (y_max_lin - y_min_lin).abs();
                let x_txt = scope.x_axis.format_value(x_lin, 6, x_range);
                let y_txt = scope.y_axis.format_value_with_unit(y_lin, 6, y_range);
                let txt = format!("P2\nx = {}\ny = {}", x_txt, y_txt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(c_p2)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new(&name, base, job).anchor(halign_anchor));
            }
            if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                plot_ui.line(Line::new(&name, vec![p1, p2]).color(c_line));
                let x1_lin = if scope.x_axis.log_scale {
                    10f64.powf(p1[0])
                } else {
                    p1[0]
                };
                let x2_lin = if scope.x_axis.log_scale {
                    10f64.powf(p2[0])
                } else {
                    p2[0]
                };
                let y1_lin = if scope.y_axis.log_scale {
                    10f64.powf(p1[1])
                } else {
                    p1[1]
                };
                let y2_lin = if scope.y_axis.log_scale {
                    10f64.powf(p2[1])
                } else {
                    p2[1]
                };
                let dx_lin = x2_lin - x1_lin;
                let dy_lin = y2_lin - y1_lin;
                let slope = if dx_lin.abs() > 1e-12 {
                    dy_lin / dx_lin
                } else {
                    f64::INFINITY
                };
                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                let y_range = (y_max_lin - y_min_lin).abs();
                let dy_txt = scope.y_axis.format_value_with_unit(dy_lin, 6, y_range);
                let txt = if slope.is_finite() {
                    format!(
                        "{}:\nΔx={:.6}\nΔy={}\nslope={:.4}",
                        name, dx_lin, dy_txt, slope
                    )
                } else {
                    format!("Δx=0\nΔy={}\nslope=∞", dy_txt)
                };
                let slope_plot = if dx != 0.0 || oy != 0.0 || ox != 0.0 {
                    (dy / oy) / (dx / ox)
                } else {
                    0.0
                };
                let (halign_anchor, base) = if slope_plot.abs() > 8.0 {
                    (Align2::RIGHT_CENTER, PlotPoint::new(mid[0] - ox, mid[1]))
                } else if slope_plot.abs() < 0.2 {
                    (Align2::CENTER_BOTTOM, PlotPoint::new(mid[0], mid[1] + oy))
                } else if slope_plot >= 0.0 {
                    (Align2::LEFT_TOP, PlotPoint::new(mid[0] + ox, mid[1] - oy))
                } else {
                    (
                        Align2::LEFT_BOTTOM,
                        PlotPoint::new(mid[0] + ox, mid[1] + oy),
                    )
                };
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(c_line)
                    .append_to(
                        &mut job,
                        &style,
                        egui::FontSelection::Default,
                        egui::Align::LEFT,
                    );
                plot_ui.text(Text::new(&name, base, job).anchor(halign_anchor));
            }
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui, data: &mut LivePlotData<'_>) {
        ui.label("Pick points on the plot and compute deltas.");
        ui.horizontal(|ui| {
            if ui.button("➕ Add").clicked() {
                let idx = self.measurements.len() + 1;
                self.measurements
                    .push(Measurement::new(&format!("M{}", idx)));
                self.selected_measurement = Some(self.measurements.len() - 1);
                self.selected_point_index = None;
            }
            if ui.button("X Clear All").clicked() {
                for m in &mut self.measurements {
                    m.clear();
                }
            }
        });
        ui.add_space(6.0);
        self.hovered_measurement = None;

        for i in 0..self.measurements.len() {
            // Use a scope so mutable borrows do not conflict
            ui.separator();
            let mut remove_this = false;
            ui.horizontal(|ui| {
                let selected = self.selected_measurement == Some(i);
                let label = ui.selectable_label(selected, format!("#{}", i + 1));
                if label.clicked() {
                    self.selected_measurement = Some(i);
                }

                let m = &mut self.measurements[i];
                let name_edit = ui.text_edit_singleline(&mut m.name);
                if name_edit.clicked() {
                    self.selected_measurement = Some(i);
                }

                let catch_trace_names: Vec<String> =
                    data.traces.keys().map(|name| name.0.clone()).collect();
                let mut selected_trace_name = m.catch_trace.clone();
                let old_selected = m.catch_trace.clone();
                let selected_text = match &selected_trace_name {
                    Some(t) => t.0.clone(),
                    None => "None".to_string(),
                };
                egui::ComboBox::from_id_salt(format!("catch_trace_{}", i))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected_trace_name, None, "None");
                        for name in &catch_trace_names {
                            ui.selectable_value(
                                &mut selected_trace_name,
                                Some(crate::TraceRef(name.clone())),
                                name.clone(),
                            );
                        }
                    });
                // The combo response.changed() may not reflect a selection change. Compare the
                // previous selection with the new selection to detect changes reliably.
                if selected_trace_name != old_selected {
                    m.catch_trace = selected_trace_name.clone();
                }

                let clear_btn = ui.button("X Clear");
                if clear_btn.clicked() {
                    m.clear();
                }

                let rm_btn = ui.button("🗑 Remove");
                if rm_btn.clicked() {
                    remove_this = true;
                }
                if label.hovered() || name_edit.hovered() || clear_btn.hovered() || rm_btn.hovered()
                {
                    self.hovered_measurement = Some(i);
                };
            });

            // Show values for P1/P2 and delta if available
            let (p1, p2) = self.measurements[i].get_points();
            let x_range = (data.scope_data.x_axis.bounds.1 - data.scope_data.x_axis.bounds.0).abs();
            let y_range = (data.scope_data.y_axis.bounds.1 - data.scope_data.y_axis.bounds.0).abs();
            ui.horizontal(|ui| {
                let mut p1_label = if let Some(p) = p1 {
                    let x_lin = if data.scope_data.x_axis.log_scale {
                        10f64.powf(p[0])
                    } else {
                        p[0]
                    };
                    let y_lin = if data.scope_data.y_axis.log_scale {
                        10f64.powf(p[1])
                    } else {
                        p[1]
                    };
                    let p1_text = format!(
                        "P1: x={}  y={}",
                        data.scope_data.x_axis.format_value(x_lin, 6, x_range),
                        data.scope_data
                            .y_axis
                            .format_value_with_unit(y_lin, 6, y_range)
                    );
                    let resp = ui.colored_label(Color32::YELLOW, p1_text.clone());
                    if resp.double_clicked() {
                        ui.ctx().copy_text(p1_text);
                    }
                    resp
                } else {
                    ui.label("P1: –")
                };
                p1_label = p1_label.on_hover_text("Click to reassign P1");
                if p1_label.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = Some(0);
                }
                // double-click handled above when value exists

                let mut p2_label = if let Some(p) = p2 {
                    let x_lin = if data.scope_data.x_axis.log_scale {
                        10f64.powf(p[0])
                    } else {
                        p[0]
                    };
                    let y_lin = if data.scope_data.y_axis.log_scale {
                        10f64.powf(p[1])
                    } else {
                        p[1]
                    };
                    let p2_text = format!(
                        "P2: x={}  y={}",
                        data.scope_data.x_axis.format_value(x_lin, 6, x_range),
                        data.scope_data
                            .y_axis
                            .format_value_with_unit(y_lin, 6, y_range)
                    );
                    let resp = ui.colored_label(Color32::LIGHT_BLUE, p2_text.clone());
                    if resp.double_clicked() {
                        ui.ctx().copy_text(p2_text);
                    }
                    resp
                } else {
                    ui.label("P2: –")
                };
                p2_label = p2_label.on_hover_text("Click to reassign P2");
                if p2_label.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = Some(1);
                }
                // double-click handled above when value exists

                if p1_label.hovered() || p2_label.hovered() {
                    self.hovered_measurement = Some(i);
                };
            });
            if let (Some(p1), Some(p2)) = (p1, p2) {
                let x1 = if data.scope_data.x_axis.log_scale {
                    10f64.powf(p1[0])
                } else {
                    p1[0]
                };
                let x2 = if data.scope_data.x_axis.log_scale {
                    10f64.powf(p2[0])
                } else {
                    p2[0]
                };
                let y1 = if data.scope_data.y_axis.log_scale {
                    10f64.powf(p1[1])
                } else {
                    p1[1]
                };
                let y2 = if data.scope_data.y_axis.log_scale {
                    10f64.powf(p2[1])
                } else {
                    p2[1]
                };
                let dx = x2 - x1;
                let dy = y2 - y1;
                let slope = if dx.abs() > 1e-12 {
                    dy / dx
                } else {
                    f64::INFINITY
                };
                let diff_txt = if slope.is_finite() {
                    format!(
                        "Δx={:.6}  Δy={}  slope={:.4}",
                        dx,
                        data.scope_data
                            .y_axis
                            .format_value_with_unit(dy, 6, y_range),
                        slope
                    )
                } else {
                    format!(
                        "Δx=0  Δy={}",
                        data.scope_data
                            .y_axis
                            .format_value_with_unit(dy, 6, y_range)
                    )
                };
                let mut diff_label = ui.colored_label(Color32::LIGHT_GREEN, diff_txt.clone());
                diff_label = diff_label.on_hover_text("Delta between P1 and P2");
                if diff_label.hovered() {
                    self.hovered_measurement = Some(i);
                };
                if diff_label.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = None;
                }
                if diff_label.double_clicked() {
                    ui.ctx().copy_text(diff_txt.clone());
                }
            }

            if remove_this {
                if self.selected_measurement == Some(i) {
                    self.selected_measurement = None;
                    self.selected_point_index = None;
                }
                self.measurements.remove(i);
                break; // restart loop due to changed indices
            }
        }
    }
}

impl MeasurementPanel {
    pub fn clear_all(&mut self) {
        for m in &mut self.measurements {
            m.clear();
        }
    }
}
