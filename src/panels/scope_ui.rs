use egui::{Color32, Ui};
use egui_plot::{Legend, Line, Plot, Points};

// no XDateFormat needed in this panel for now

use crate::data::scope::ScopeType;
use crate::data::data::LivePlotData;
use crate::data::scope::ScopeData;
// no panel trait needed here; overlays are provided via closure from app

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ZoomMode {
    Off,
    X,
    Y,
    Both,
}

pub struct ScopePanel {
    data: ScopeData,

    // UI state
    zoom_mode: ZoomMode,
    time_slider_dragging: bool,
    time_window_bounds: (f64, f64),
    points_bounds: (usize, usize),
}

impl Default for ScopePanel {
    fn default() -> Self {
        Self {
            data: ScopeData::default(),
            zoom_mode: ZoomMode::X,
            time_slider_dragging: false,
            time_window_bounds: (0.1, 100.0),
            points_bounds: (5000, 200000),
        }
    }
}

impl ScopePanel {

    pub fn new(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> Self {
        let mut instance = Self::default();
        instance.data.set_rx(rx);
        instance
    }

    pub fn update_data(&mut self) -> &mut LivePlotData {
        self.data.update();
        &mut self.data
    }

    pub fn get_data_mut(&mut self) -> &mut LivePlotData {
        &mut self.data
    }

    pub fn render_menu(&mut self, _ui: &mut Ui) {}

    pub fn render_panel<F>(&mut self, ui: &mut Ui, mut draw_overlays: F)
    where
        F: FnMut(&mut egui_plot::PlotUi, &LivePlotData),
    {
        self.render_controls(ui);
        ui.separator();
        self.render_plot(ui, &mut draw_overlays);
    }

    fn render_controls(&mut self, ui: &mut Ui) {
        // Main top-bar controls grouped similarly to old controls_ui

        ui.horizontal(|ui| {
            ui.label("Data Points:");
            ui.add(egui::Slider::new(
                &mut self.data.traces.max_points,
                self.points_bounds.0..=self.points_bounds.1,
            ));

            ui.separator();

            ui.strong("X-Axis");
            if self.data.scope_type == ScopeType::TimeScope {
                ui.label("Time Window:");
                let mut tw = self.data.time_window.max(1e-9);
                if !self.time_slider_dragging {
                    if tw <= self.time_window_bounds.0 {
                        self.time_window_bounds.0 /= 10.0;
                        self.time_window_bounds.1 /= 10.0;
                    } else if tw >= self.time_window_bounds.1 {
                        self.time_window_bounds.0 *= 10.0;
                        self.time_window_bounds.1 *= 10.0;
                    }
                }

                let slider = egui::Slider::new(
                    &mut tw,
                    self.time_window_bounds.0..=self.time_window_bounds.1,
                )
                .logarithmic(true)
                .smart_aim(true)
                .show_value(true)
                .clamping(egui::SliderClamping::Edits)
                .custom_formatter(|n, _| {
                    // Use the axis formatter with unit for the time window value
                    // Use the current value as step heuristic for sci thresholding
                    self.data.x_axis.format_value_with_unit(n, 4, n)
                });

                let sresp = ui.add(slider);
                if sresp.changed() {
                    self.data.time_window = tw;
                }

                self.time_slider_dragging = sresp.is_pointer_button_down_on();
            } else {
                let mut x_min_tmp = self.data.x_axis.bounds.0;
                let mut x_max_tmp = self.data.x_axis.bounds.1;
                let x_range = x_max_tmp - x_min_tmp;
                ui.label("Min:");
                let r1 = ui.add(
                    egui::DragValue::new(&mut x_min_tmp)
                        .speed(0.1)
                        .custom_formatter(|n, _| self.data.x_axis.format_value_with_unit(n, 4, x_range)),
                );
                ui.label("Max:");
                let r2 = ui.add(
                    egui::DragValue::new(&mut x_max_tmp)
                        .speed(0.1)
                        .custom_formatter(|n, _| self.data.x_axis.format_value_with_unit(n, 4, x_range)),
                );
                if (r1.changed() || r2.changed()) && x_min_tmp < x_max_tmp {
                    self.data.x_axis.bounds.0 = x_min_tmp;
                    self.data.x_axis.bounds.1 = x_max_tmp;
                    self.data.time_window = x_max_tmp - x_min_tmp;
                }
            }

            if ui
                .button("Fit X")
                .on_hover_text("Fit X to visible data")
                .clicked()
            {
                self.data.fit_x_bounds();
            }

            ui.checkbox(&mut self.data.x_axis.auto_fit, "Auto Fit X");

            ui.separator();

            // Y controls
            let mut y_min_tmp = self.data.y_axis.bounds.0;
            let mut y_max_tmp = self.data.y_axis.bounds.1;
            let y_range = y_max_tmp - y_min_tmp;
            ui.strong("Y-Axis");
            ui.label("Min:");
            let r1 = ui.add(
                egui::DragValue::new(&mut y_min_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| self.data.y_axis.format_value_with_unit(n, 4, y_range)),
            );
            ui.label("Max:");
            let r2 = ui.add(
                egui::DragValue::new(&mut y_max_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| self.data.y_axis.format_value_with_unit(n, 4, y_range)),
            );
            if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp {
                self.data.y_axis.bounds.0 = y_min_tmp;
                self.data.y_axis.bounds.1 = y_max_tmp;
            }

            if ui
                .button("Fit Y")
                .on_hover_text("Fit Y to visible data")
                .clicked()
            {
                self.data.fit_y_bounds();
            }

            ui.checkbox(&mut self.data.y_axis.auto_fit, "Auto Fit Y");

            ui.separator();

            ui.strong("Zoom:");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both");

            ui.separator();

            if ui
                .button("Fit to View")
                .on_hover_text("Fit both axes to visible data")
                .clicked()
            {
                self.data.fit_bounds();
            }

            ui.separator();

            if !self.data.is_paused() {
                if ui.button("Pause").clicked() {
                    self.data.pause();
                }
            } else {
                if ui.button("Resume").clicked() {
                    self.data.resume();
                }
            }

            if ui.button("Clear All").clicked() {
                self.data.clear_all();
            }

            // Request a screenshot of the current viewport; the handler below
            // will catch the Screenshot event and write it to disk.
            if ui.button("Save Screenshot").on_hover_text("Take a screenshot of the entire window").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }

            ui.separator();
        });
    }

    // Handle a completed screenshot event and write the PNG to disk.
    // If an environment variable LIVEPLOT_SAVE_SCREENSHOT_TO is set, save there.
    // Otherwise, prompt the user for a path.
    fn handle_screenshot_result(&mut self, ui: &mut Ui) {
        if let Some(image_arc) = ui.ctx().input(|i| {
            i.events.iter().rev().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        }) {
            // Convert ColorImage to an image::RgbaImage
            let img = &*image_arc;
            let [w, h] = img.size;
            let mut out = image::RgbaImage::new(w as u32, h as u32);
            for y in 0..h {
                for x in 0..w {
                    let p = img.pixels[y * w + x];
                    out.put_pixel(x as u32, y as u32, image::Rgba([p.r(), p.g(), p.b(), p.a()]));
                }
            }

            // Determine path: env var or file dialog
            if let Ok(path_str) = std::env::var("LIVEPLOT_SAVE_SCREENSHOT_TO") {
                std::env::remove_var("LIVEPLOT_SAVE_SCREENSHOT_TO");
                let path = std::path::PathBuf::from(path_str);
                if let Err(e) = out.save(&path) {
                    eprintln!("Failed to save viewport screenshot: {e}");
                } else {
                    eprintln!("Saved viewport screenshot to {:?}", path);
                }
            } else {
                let default_name = format!(
                    "viewport_{:.0}.png",
                    chrono::Local::now().timestamp_millis()
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&default_name)
                    .add_filter("PNG", &["png"])
                    .save_file()
                {
                    if let Err(e) = out.save(&path) {
                        eprintln!("Failed to save viewport screenshot: {e}");
                    } else {
                        eprintln!("Saved viewport screenshot to {:?}", path);
                    }
                }
            }
        }
    }

    fn render_plot<F>(&mut self, ui: &mut Ui, mut draw_overlays: F)
    where
        F: FnMut(&mut egui_plot::PlotUi, &LivePlotData),
    {
        // No extra controls in panel; top bar uses render_menu
        // Render plot directly here (for now). Later we can separate draw() if needed.
        // First, handle any completed screenshot events from the OS/windowing backend.
        self.handle_screenshot_result(ui);

        let y_log = self.data.y_axis.log_scale;
        let x_log = self.data.x_axis.log_scale;
        let plot = Plot::new("scope_plot")
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(true)
            .legend(Legend::default())
            .x_axis_formatter(|x, _range| {
                let x_value = if x_log { 10f64.powf(x.value) } else { x.value };
                self.data
                    .x_axis
                    .format_value(x_value, 4, x.step_size.abs())
            })
            .y_axis_formatter(|y, _range| {
                // Scientific ticks with optional unit, apply inverse log mapping for display
                let y_value = if y_log { 10f64.powf(y.value) } else { y.value };
                self.data
                    .y_axis
                    .format_value(y_value, 4, y.step_size.abs())
            });

    let plot_resp = plot.show(ui, |plot_ui| {
            // Handle wheel zoom around hovered point
            let resp = plot_ui.response();

            let is_zooming_rect = resp.drag_stopped_by(egui::PointerButton::Secondary);
            let is_panning =
                resp.dragged_by(egui::PointerButton::Primary) && resp.is_pointer_button_down_on();

            let scroll_data = resp.ctx.input(|i| i.raw_scroll_delta);
            let is_zooming_with_wheel =
                (scroll_data.x != 0.0 || scroll_data.y != 0.0) && resp.hovered();

            let bounds_changed = is_zooming_rect || is_panning || is_zooming_with_wheel;

            if is_zooming_with_wheel {
                let mut zoom_factor = egui::Vec2::new(1.0, 1.0);
                if scroll_data.y != 0.0
                    && (self.zoom_mode == ZoomMode::X || self.zoom_mode == ZoomMode::Both)
                {
                    zoom_factor.x = 1.0 + scroll_data.y * 0.001;
                } else if scroll_data.x != 0.0 {
                    zoom_factor.x = 1.0 - scroll_data.x * 0.001;
                }
                if self.zoom_mode == ZoomMode::Y || self.zoom_mode == ZoomMode::Both {
                    zoom_factor.y = 1.0 + scroll_data.y * 0.001;
                }

                // if !self.data.paused {
                //     let t_latest = self.data.x_axis.bounds.1;
                //     plot_ui.set_plot_bounds_x(
                //         t_latest - self.data.time_window * (2.0 - (zoom_factor.x as f64))
                //             ..=t_latest,
                //     );
                //     zoom_factor.x = 1.0;
                // }
                plot_ui.zoom_bounds_around_hovered(zoom_factor);
            }

            // Apply bounds: X follows latest time using time_window; Y respects manual limits if valid
            if !bounds_changed {
                let (x_min, x_max) = self.data.x_axis.bounds;
                let x_space = (x_max - x_min) * 0.05;
                plot_ui.set_plot_bounds_x(x_min - x_space..=x_max + x_space);

                let (y_min, y_max) = self.data.y_axis.bounds;
                let y_space = (y_max - y_min) * 0.05;
                plot_ui.set_plot_bounds_y(y_min - y_space..=y_max + y_space);
            }

            // Draw traces
            for name in self.data.trace_order.clone().into_iter() {
                if let Some(tr) = self.data.traces.get(&name) {
                    if !tr.look.visible {
                        continue;
                    }
                    let shown_pts = match self.data.get_drawn_points(&name) {
                        Some(pts) => pts,
                        None => continue,
                    };
                    let pts_vec: Vec<[f64; 2]> = shown_pts
                        .into_iter()
                        .map(|p| {
                            let y_lin = p[1] + tr.offset;
                            let y = if self.data.y_axis.log_scale {
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                y_lin
                            };
                            let x = if self.data.x_axis.log_scale {
                                if p[0] > 0.0 {
                                    p[0].log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                p[0]
                            };
                            [x, y]
                        })
                        .collect();
                    let mut color = tr.look.color;
                    let mut width: f32 = tr.look.width.max(0.1);
                    let style = tr.look.style;
                    if let Some(hov) = &self.data.hover_trace {
                        if name != *hov {
                            // Strongly dim non-hovered traces
                            color = Color32::from_rgba_unmultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                40,
                            );
                        } else {
                            // Emphasize hovered trace
                            width = (width * 1.6).max(width + 1.0);
                        }
                    }
                    let mut line = Line::new(name.clone(), pts_vec.clone())
                        .color(color)
                        .width(width)
                        .style(style);
                    let legend_label = if self.data.show_info_in_legend && !tr.info.is_empty() {
                        format!("{} — {}", name, tr.info)
                    } else {
                        name.clone()
                    };
                    line = line.name(legend_label.clone());
                    plot_ui.line(line);

                    // Optional point markers for each datapoint
                    if tr.look.show_points {
                        if !pts_vec.is_empty() {
                            let mut radius = tr.look.point_size.max(0.5);
                            if let Some(hov) = &self.data.hover_trace {
                                if name == *hov {
                                    radius = (radius * 1.25).max(radius + 0.5);
                                }
                            }
                            let points = Points::new(legend_label, pts_vec.clone())
                                .radius(radius)
                                .shape(tr.look.marker)
                                .color(color);
                            plot_ui.points(points);
                        }
                    }
                }
            }

            // Additional overlays provided by caller (e.g., thresholds, markers)
                draw_overlays(plot_ui, &self.data);

            // Detect bounds changes via zoom box
            bounds_changed
        });

        // After plot: if bounds changed, sync time_window and Y limits from actual plot bounds
        if plot_resp.inner {
            let b = plot_resp.transform.bounds();
            let xr = b.range_x();
            let (x_min, x_max) = (xr.start(), xr.end());
            let space_x = (0.05 / 1.1) * (x_max - x_min);
            if x_min.is_finite() && x_max.is_finite() && x_max > x_min {
                self.data.x_axis.bounds = (x_min + space_x, x_max - space_x);
                self.data.time_window = x_max - x_min - 2.0 * space_x;
            }
            let yr = b.range_y();
            let (y_min, y_max) = (yr.start(), yr.end());
            let space_y = (0.05 / 1.1) * (y_max - y_min);
            if y_min.is_finite() && y_max.is_finite() && y_max > y_min {
                self.data.y_axis.bounds = (y_min + space_y, y_max - space_y);
            }
        }

        self.handle_plot_click(&plot_resp);

        self.data.hover_trace = None;
    }

    /// Handle click selection on the plot using nearest point logic.
     fn handle_plot_click(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.response.clicked() {
            if !self.data.is_paused() {
                self.data.pause();
            }

            if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                let transform = plot_response.transform;
                let plot_pos = transform.value_from_position(screen_pos);
                let selected_trace_name = self.data.selection_trace.clone();
                let sel_data_points: Option<Vec<[f64; 2]>> =
                    if let Some(name) = &selected_trace_name {
                        self.data.traces.get(name).map(|tr| {
                            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.data.is_paused() {
                                if let Some(snap) = &tr.snap {
                                    Box::new(snap.iter())
                                } else {
                                    Box::new(tr.live.iter())
                                }
                            } else {
                                Box::new(tr.live.iter())
                            };
                            iter.cloned().collect()
                        })
                    } else {
                        None
                    };
                match (&selected_trace_name, &sel_data_points) {
                    (Some(name), Some(data_points)) if !data_points.is_empty() => {
                        let off = self.data.traces.get(name).map(|t| t.offset).unwrap_or(0.0);
                        let mut best_i = None;
                        let mut best_d2 = f64::INFINITY;
                        for (i, p) in data_points.iter().enumerate() {
                            let x_lin = p[0];
                            let x_plot = if self.data.x_axis.log_scale {
                                if x_lin > 0.0 {
                                    x_lin.log10()
                                } else {
                                    continue;
                                }
                            } else {
                                x_lin
                            };
                            let y_lin = p[1] + off;
                            let y_plot = if self.data.y_axis.log_scale {
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    continue;
                                }
                            } else {
                                y_lin
                            };
                            let dx = x_plot - plot_pos.x;
                            let dy = y_plot - plot_pos.y;
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_d2 {
                                best_d2 = d2;
                                best_i = Some(i);
                            }
                        }
                        if let Some(i) = best_i {
                            let p = data_points[i];
                            let x_lin = p[0];
                            let x_plot = if self.data.x_axis.log_scale { x_lin.log10() } else { x_lin };
                            let y_lin = p[1] + off;
                            let y_plot = if self.data.y_axis.log_scale { y_lin.log10() } else { y_lin };
                            self.data.clicked_point = Some([x_plot, y_plot]);
                        }
                    }
                    _ => {
                        let x_plot = if self.data.x_axis.log_scale {
                            if plot_pos.x > 0.0 {
                                plot_pos.x.log10()
                            } else {
                                plot_pos.x
                            }
                        } else {
                            plot_pos.x
                        };
                        let y_plot = if self.data.y_axis.log_scale {
                            if plot_pos.y > 0.0 {
                                plot_pos.y.log10()
                            } else {
                                plot_pos.y
                            }
                        } else {
                            plot_pos.y
                        };
                        self.data.clicked_point = Some([x_plot, y_plot]);
                    }
                }
            }
        }
    }
}
