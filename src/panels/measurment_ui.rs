use super::panel_trait::{Panel, PanelState};
use crate::data::measurement::Measurement;
use crate::data::scope::ScopeData;
use egui::{Align2, Color32};
use egui_plot::{Line, PlotPoint, Points, Text};

pub struct MeasurementPanel {
    state: PanelState,
    measurements: Vec<Measurement>,
    selected_measurement: Option<usize>,
    selected_point_index: Option<usize>,
    last_clicked_point: Option<[f64; 2]>,
}

impl Default for MeasurementPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Measurement"),
            measurements: vec![Measurement::default()],
            selected_measurement: None,
            selected_point_index: None,
            last_clicked_point: None,
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

    fn update_data(&mut self, _data: &mut ScopeData) {
        if let Some(point) = _data.clicked_point {
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

    fn draw(&mut self, plot_ui: &mut egui_plot::PlotUi, _data: &ScopeData) {
        // Draw the measurement UI components here
        // Measurement overlays
        let base_body = plot_ui.ctx().style().text_styles[&egui::TextStyle::Body].size;
        let marker_font_size = base_body * 1.5;

        for measurement in &self.measurements {
            let (p1_opt, p2_opt) = measurement.get_points();
            // Compute small offsets in PLOT coordinates (respecting log axes)
            let (x_min_lin, x_max_lin) = _data.x_axis.bounds;
            let (y_min_lin, y_max_lin) = _data.y_axis.bounds;
            let x_min_plot = if _data.x_axis.log_scale && x_min_lin > 0.0 {
                x_min_lin.log10()
            } else {
                x_min_lin
            };
            let x_max_plot = if _data.x_axis.log_scale && x_max_lin > 0.0 {
                x_max_lin.log10()
            } else {
                x_max_lin
            };
            let y_min_plot = if _data.y_axis.log_scale && y_min_lin > 0.0 {
                y_min_lin.log10()
            } else {
                y_min_lin
            };
            let y_max_plot = if _data.y_axis.log_scale && y_max_lin > 0.0 {
                y_max_lin.log10()
            } else {
                y_max_lin
            };
            let ox = 0.01 * (x_max_plot - x_min_plot);
            let oy = 0.01 * (y_max_plot - y_min_plot);

            let (dx, dy) = if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                (p2[0] - p1[0], p2[1] - p1[1]) // plot-space differences
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
                plot_ui.points(
                    Points::new(measurement.name.clone(), vec![p])
                        .radius(5.0)
                        .color(Color32::YELLOW),
                );
                let (halign_anchor, text_align, base) = label_pos(dx, dy, &p, ox, oy);
                let x_lin = if _data.x_axis.log_scale {
                    10f64.powf(p[0])
                } else {
                    p[0]
                };
                let y_lin = if _data.y_axis.log_scale {
                    10f64.powf(p[1])
                } else {
                    p[1]
                };
                let x_range = (x_max_lin - x_min_lin).abs();
                let y_range = (y_max_lin - y_min_lin).abs();
                let x_txt = _data.x_axis.format_value(x_lin, 6, x_range);
                let y_txt = _data.y_axis.format_value_with_unit(y_lin, 6, y_range);
                let txt = format!("P1\nx = {}\ny = {}", x_txt, y_txt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::YELLOW)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let Some(p) = p2_opt {
                plot_ui.points(
                    Points::new("Measurement", vec![p])
                        .radius(5.0)
                        .color(Color32::LIGHT_BLUE),
                );
                let (halign_anchor, text_align, base) = label_pos(-dx, -dy, &p, ox, oy);
                let x_lin = if _data.x_axis.log_scale {
                    10f64.powf(p[0])
                } else {
                    p[0]
                };
                let y_lin = if _data.y_axis.log_scale {
                    10f64.powf(p[1])
                } else {
                    p[1]
                };
                let x_range = (x_max_lin - x_min_lin).abs();
                let y_range = (y_max_lin - y_min_lin).abs();
                let x_txt = _data.x_axis.format_value(x_lin, 6, x_range);
                let y_txt = _data.y_axis.format_value_with_unit(y_lin, 6, y_range);
                let txt = format!("P2\nx = {}\ny = {}", x_txt, y_txt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_BLUE)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                plot_ui.line(Line::new("Measurement", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                // Compute slope and delta in LINEAR units for readability
                let x1_lin = if _data.x_axis.log_scale {
                    10f64.powf(p1[0])
                } else {
                    p1[0]
                };
                let x2_lin = if _data.x_axis.log_scale {
                    10f64.powf(p2[0])
                } else {
                    p2[0]
                };
                let y1_lin = if _data.y_axis.log_scale {
                    10f64.powf(p1[1])
                } else {
                    p1[1]
                };
                let y2_lin = if _data.y_axis.log_scale {
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
                let dy_txt = _data.y_axis.format_value_with_unit(dy_lin, 6, y_range);
                let txt = if slope.is_finite() {
                    format!("Δx={:.6}\nΔy={}\nslope={:.4}", dx_lin, dy_txt, slope)
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
                    .color(Color32::LIGHT_GREEN)
                    .append_to(
                        &mut job,
                        &style,
                        egui::FontSelection::Default,
                        egui::Align::LEFT,
                    );
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui, data: &mut ScopeData) {
        ui.label("Pick points on the plot and compute deltas.");
        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                let idx = self.measurements.len() + 1;
                self.measurements
                    .push(Measurement::new(&format!("M{}", idx)));
            }
            if ui.button("Clear All").clicked() {
                for m in &mut self.measurements {
                    m.clear();
                }
                self.last_clicked_point = None;
            }
        });
        ui.add_space(6.0);

        for i in 0..self.measurements.len() {
            // Use a scope so mutable borrows do not conflict
            ui.separator();
            let mut remove_this = false;
            ui.horizontal(|ui| {
                let selected = self.selected_measurement == Some(i);
                if ui
                    .selectable_label(selected, format!("#{}", i + 1))
                    .clicked()
                {
                    self.selected_measurement = Some(i);
                }
                let m = &mut self.measurements[i];
                ui.text_edit_singleline(&mut m.name);

                if ui
                    .button("Pick")
                    .on_hover_text("Set next click to auto-advance P1/P2")
                    .clicked()
                {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = None;
                }
                if ui.button("P1").clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = Some(0);
                }
                if ui.button("P2").clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = Some(1);
                }
                if ui.button("Clear").clicked() {
                    m.clear();
                    self.last_clicked_point = None;
                }
                if ui.button("Remove").clicked() {
                    remove_this = true;
                }
            });

            // Show values for P1/P2 and delta if available
            let (p1, p2) = self.measurements[i].get_points();
            let x_range = (data.x_axis.bounds.1 - data.x_axis.bounds.0).abs();
            let y_range = (data.y_axis.bounds.1 - data.y_axis.bounds.0).abs();
            ui.horizontal(|ui| {
                if let Some(p) = p1 {
                    let x_lin = if data.x_axis.log_scale {
                        10f64.powf(p[0])
                    } else {
                        p[0]
                    };
                    let y_lin = if data.y_axis.log_scale {
                        10f64.powf(p[1])
                    } else {
                        p[1]
                    };
                    ui.colored_label(
                        Color32::YELLOW,
                        format!(
                            "P1: x={}  y={}",
                            data.x_axis.format_value(x_lin, 6, x_range),
                            data.y_axis.format_value_with_unit(y_lin, 6, y_range)
                        ),
                    );
                } else {
                    ui.label("P1: –");
                }
                if let Some(p) = p2 {
                    let x_lin = if data.x_axis.log_scale {
                        10f64.powf(p[0])
                    } else {
                        p[0]
                    };
                    let y_lin = if data.y_axis.log_scale {
                        10f64.powf(p[1])
                    } else {
                        p[1]
                    };
                    ui.colored_label(
                        Color32::LIGHT_BLUE,
                        format!(
                            "P2: x={}  y={}",
                            data.x_axis.format_value(x_lin, 6, x_range),
                            data.y_axis.format_value_with_unit(y_lin, 6, y_range)
                        ),
                    );
                } else {
                    ui.label("P2: –");
                }
            });
            if let (Some(p1), Some(p2)) = (p1, p2) {
                let x1 = if data.x_axis.log_scale {
                    10f64.powf(p1[0])
                } else {
                    p1[0]
                };
                let x2 = if data.x_axis.log_scale {
                    10f64.powf(p2[0])
                } else {
                    p2[0]
                };
                let y1 = if data.y_axis.log_scale {
                    10f64.powf(p1[1])
                } else {
                    p1[1]
                };
                let y2 = if data.y_axis.log_scale {
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
                ui.colored_label(
                    Color32::LIGHT_GREEN,
                    if slope.is_finite() {
                        format!(
                            "Δx={:.6}  Δy={}  slope={:.4}",
                            dx,
                            data.y_axis.format_value_with_unit(dy, 6, y_range),
                            slope
                        )
                    } else {
                        format!(
                            "Δx=0  Δy={}",
                            data.y_axis.format_value_with_unit(dy, 6, y_range)
                        )
                    },
                );
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
