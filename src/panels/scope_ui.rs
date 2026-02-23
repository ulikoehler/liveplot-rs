use egui::{Color32, Ui};
use egui_plot::{Legend, Line, Plot, Points};

use crate::data::scope::ScopeData;
use crate::data::scope::ScopeType;
use crate::data::traces::TracesCollection;
use crate::events::EventController;

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
    controlls_in_toolbar: bool,
    zoom_mode: ZoomMode,
    time_slider_dragging: bool,
    time_window_bounds: (f64, f64),

    // Event controller reference for emitting events
    pub(crate) event_ctrl: Option<EventController>,

    // Responsive tick-label thresholds
    /// Hide Y-axis tick labels when the plot width (px) falls below this value.
    pub min_width_for_y_ticklabels: f32,
    /// Hide X-axis tick labels when the plot height (px) falls below this value.
    pub min_height_for_x_ticklabels: f32,
    /// Hide the legend when the total widget width (px) falls below this value. `0.0` = always show.
    pub min_width_for_legend: f32,
    /// Hide the legend when the total widget height (px) falls below this value. `0.0` = always show.
    pub min_height_for_legend: f32,

    /// Total size of the entire plot widget (including top bar, side panels, etc.).
    /// Set by the parent before rendering; used for responsive tick-label decisions.
    pub total_widget_size: egui::Vec2,
}

impl Default for ScopePanel {
    fn default() -> Self {
        Self {
            data: ScopeData::default(),
            controlls_in_toolbar: false,
            zoom_mode: ZoomMode::X,
            time_slider_dragging: false,
            time_window_bounds: (0.1, 100.0),
            event_ctrl: None,
            min_width_for_y_ticklabels: 250.0,
            min_height_for_x_ticklabels: 200.0,
            min_width_for_legend: 0.0,
            min_height_for_legend: 0.0,
            total_widget_size: egui::Vec2::new(10_000.0, 10_000.0),
        }
    }
}

impl ScopePanel {
    pub fn new(id: usize) -> Self {
        let mut pane = Self::default();
        let name = format!("Scope {}", id + 1);
        let data = pane.get_data_mut();
        data.id = id;
        data.name = name;
        pane
    }

    pub fn name(&self) -> &str {
        &self.data.name
    }

    pub fn id(&self) -> usize {
        self.data.id
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let n = name.into();
        self.data.name = n.clone();
        self.data.name = n;
    }

    /// Return whether the controls toolbar is currently visible for this scope.
    pub fn controls_in_toolbar(&self) -> bool {
        self.controlls_in_toolbar
    }

    /// Set whether the controls toolbar is visible for this scope.
    pub fn set_controls_in_toolbar(&mut self, visible: bool) {
        self.controlls_in_toolbar = visible;
    }

    pub fn update_data(&mut self, traces: &TracesCollection) {
        self.data.update(traces);
    }

    pub fn get_data_mut(&mut self) -> &mut ScopeData {
        &mut self.data
    }

    pub fn get_data(&self) -> &ScopeData {
        &self.data
    }

    /// Returns current value of the `pause_on_click` flag.
    pub fn pause_on_click(&self) -> bool {
        self.data.pause_on_click
    }

    /// Enable or disable the left-click pause/resume behaviour for this scope.
    pub fn set_pause_on_click(&mut self, enabled: bool) {
        self.data.pause_on_click = enabled;
    }

    pub fn render_menu(&mut self, ui: &mut Ui, traces: &mut TracesCollection) {
        ui.checkbox(&mut self.controlls_in_toolbar, "Controls in Toolbar");

        ui.separator();

        self.render_controls(ui, traces);

        ui.separator();

        ui.checkbox(&mut self.data.show_grid, "Show Grid")
            .on_hover_text("Show or hide the plot background grid");
        ui.checkbox(&mut self.data.show_legend, "Show Legend")
            .on_hover_text("Show or hide the plot legend");
        if !self.data.show_legend {
            self.data.show_info_in_legend = false;
        }
        ui.add_enabled_ui(self.data.show_legend, |ui| {
            ui.checkbox(&mut self.data.show_info_in_legend, "Show Info")
                .on_hover_text("Append each trace's info text to its legend label");
        });
    }

    pub fn render_panel<F>(
        &mut self,
        ui: &mut Ui,
        mut draw_overlays: F,
        traces: &mut TracesCollection,
    ) where
        F: FnMut(&mut egui_plot::PlotUi, &ScopeData, &TracesCollection),
    {
        if self.controlls_in_toolbar {
            ui.horizontal_wrapped(|ui| {
                self.render_controls(ui, traces);
            });
            ui.separator();
        }
        self.render_plot(ui, &mut draw_overlays, traces);
    }

    // Extended controls with injectable prefix/suffix sections
    fn render_controls(&mut self, ui: &mut Ui, traces: &mut TracesCollection) {
        if !self.data.paused {
            if ui.button("⏸ Pause").clicked() {
                self.data.paused = true;
                traces.take_snapshot();
            }
        } else if ui.button("▶ Resume").clicked() {
            self.data.paused = false;
        }

        ui.separator();
        // X controls
        ui.strong("X-Axis");
        ui.horizontal(|ui| {
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
                .custom_formatter(|n, _| self.data.x_axis.format_value(n, 4, n));

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
                        .custom_formatter(|n, _| self.data.x_axis.format_value(n, 4, x_range)),
                );
                ui.label("Max:");
                let r2 = ui.add(
                    egui::DragValue::new(&mut x_max_tmp)
                        .speed(0.1)
                        .custom_formatter(|n, _| self.data.x_axis.format_value(n, 4, x_range)),
                );
                if (r1.changed() || r2.changed()) && x_min_tmp < x_max_tmp {
                    self.data.x_axis.bounds.0 = x_min_tmp;
                    self.data.x_axis.bounds.1 = x_max_tmp;
                    self.data.time_window = x_max_tmp - x_min_tmp;
                }
            }
        });

        ui.horizontal(|ui| {
            if ui
                .button("↔ Fit X")
                .on_hover_text("Fit X to visible data")
                .clicked()
            {
                self.data.fit_x_bounds(traces);
            }

            ui.checkbox(&mut self.data.x_axis.auto_fit, "Auto Fit X");
        });

        ui.separator();

        // Y controls
        ui.horizontal(|ui| {
            let mut y_min_tmp = self.data.y_axis.bounds.0;
            let mut y_max_tmp = self.data.y_axis.bounds.1;
            let y_range = y_max_tmp - y_min_tmp;
            ui.strong("Y-Axis");
            ui.label("Min:");
            let r1 = ui.add(
                egui::DragValue::new(&mut y_min_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| self.data.y_axis.format_value(n, 4, y_range)),
            );
            ui.label("Max:");
            let r2 = ui.add(
                egui::DragValue::new(&mut y_max_tmp)
                    .speed(0.1)
                    .custom_formatter(|n, _| self.data.y_axis.format_value(n, 4, y_range)),
            );
            if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp {
                self.data.y_axis.bounds.0 = y_min_tmp;
                self.data.y_axis.bounds.1 = y_max_tmp;
            }
        });

        ui.horizontal(|ui| {
            if ui
                .button("↕ Fit Y")
                .on_hover_text("Fit Y to visible data")
                .clicked()
            {
                self.data.fit_y_bounds(traces);
            }

            ui.checkbox(&mut self.data.y_axis.auto_fit, "Auto Fit Y");
        });

        ui.checkbox(&mut self.data.y_axis.log_scale, "Log scale")
            .on_hover_text("Use base-10 log of (value + offset). Non-positive values are omitted.");

        ui.horizontal(|ui| {
            ui.label("Unit:");
            let mut unit = self.data.y_axis.get_unit().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut unit).desired_width(80.0))
                .changed()
            {
                self.data.y_axis.set_unit(if unit.trim().is_empty() {
                    None
                } else {
                    Some(unit)
                });
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.strong("Zoom:");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y");
            ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both");
        });

        ui.separator();

        if ui
            .button("🔍 Fit to View")
            .on_hover_text("Fit both axes to visible data")
            .clicked()
        {
            self.data.fit_bounds(traces);
            self.data.auto_fit_to_view = true;
            // Emit fit-to-view event
            if let Some(ctrl) = &self.event_ctrl {
                let mut evt = crate::events::PlotEvent::new(
                    crate::events::EventKind::FIT_TO_VIEW | crate::events::EventKind::ZOOM,
                );
                evt.view_change = Some(crate::events::ViewChangeMeta {
                    x_range: Some(self.data.x_axis.bounds),
                    y_range: Some(self.data.y_axis.bounds),
                    scope_id: Some(self.data.id),
                });
                ctrl.emit_filtered(evt);
            }
        }

        ui.separator();

        // Screenshot button kept in core controls
        if ui
            .button("🖼 Save Screenshot")
            .on_hover_text("Take a screenshot of the entire window")
            .clicked()
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
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
                    out.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([p.r(), p.g(), p.b(), p.a()]),
                    );
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

    fn render_plot<F>(&mut self, ui: &mut Ui, mut draw_overlays: F, traces: &mut TracesCollection)
    where
        F: FnMut(&mut egui_plot::PlotUi, &ScopeData, &TracesCollection),
    {
        // First, handle any completed screenshot events from the OS/windowing backend.
        self.handle_screenshot_result(ui);

        // Determine whether tick labels should be suppressed based on the TOTAL
        // widget size (including top bar, sidebars, etc.) so the decision is
        // stable and doesn't jump as sub-panels are toggled.
        let hide_y_labels = self.total_widget_size.x < self.min_width_for_y_ticklabels;
        let hide_x_labels = self.total_widget_size.y < self.min_height_for_x_ticklabels;
        let hide_legend = self.data.force_hide_legend
            || (self.min_width_for_legend > 0.0
                && self.total_widget_size.x < self.min_width_for_legend)
            || (self.min_height_for_legend > 0.0
                && self.total_widget_size.y < self.min_height_for_legend);

        let y_log = self.data.y_axis.log_scale;
        let x_log = self.data.x_axis.log_scale;
        let show_grid = self.data.show_grid;
        let mut plot = Plot::new(format!("scope_plot_{}", self.data.name))
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(true)
            .show_grid(egui::Vec2b::new(show_grid, show_grid))
            // When tick labels are hidden (thresholds set above available size), also
            // suppress the egui_plot axis space reservation so the plot fills the full
            // widget width without a black gutter on the left (Y axis) or bottom (X axis).
            .show_axes(egui::Vec2b::new(!hide_x_labels, !hide_y_labels));
        if self.data.show_legend && !hide_legend {
            plot = plot.legend(Legend::default());
        }
        let plot = plot
            .x_axis_formatter(|x, _range| {
                if hide_x_labels {
                    return String::new();
                }
                let x_value = if x_log { 10f64.powf(x.value) } else { x.value };
                self.data.x_axis.format_value(x_value, 4, x.step_size.abs())
            })
            .y_axis_formatter(|y, _range| {
                if hide_y_labels {
                    return String::new();
                }
                // Scientific ticks with optional unit, apply inverse log mapping for display
                let y_value = if y_log { 10f64.powf(y.value) } else { y.value };
                self.data.y_axis.format_value(y_value, 4, y.step_size.abs())
            })
            .label_formatter(|name, value| {
                let x = if x_log { 10f64.powf(value.x) } else { value.x };
                let y = if y_log { 10f64.powf(value.y) } else { value.y };
                // For time axes this routes through TimeFormatter; for value axes numeric.
                // For XY scopes both axes are value-typed, so both format numerically.
                let x_str = self.data.x_axis.format_value(x, 6, 0.0);
                let y_str = self.data.y_axis.format_value(y, 6, 0.0);
                if name.is_empty() {
                    format!("x = {}\ny = {}", x_str, y_str)
                } else {
                    format!("{}\nx = {}\ny = {}", name, x_str, y_str)
                }
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
            if self.data.scope_type == ScopeType::XYScope && !self.data.xy_pairs.is_empty() {
                let tol = 1e-9_f64;
                for (x_name, y_name, pair_look) in self.data.xy_pairs.clone().into_iter() {
                    let (Some(x_name), Some(y_name)) = (x_name, y_name) else {
                        continue;
                    };
                    let (Some(x_tr), Some(y_tr)) =
                        (traces.get_trace(&x_name), traces.get_trace(&y_name))
                    else {
                        continue;
                    };
                    if !pair_look.visible || !x_tr.look.visible || !y_tr.look.visible {
                        continue;
                    }

                    let x_pts = traces.get_points(&x_name, self.data.paused);
                    let y_pts = traces.get_points(&y_name, self.data.paused);
                    let (Some(x_pts), Some(y_pts)) = (x_pts, y_pts) else {
                        continue;
                    };

                    let mut derived: Vec<[f64; 2]> = Vec::new();
                    let mut i = 0usize;
                    let mut j = 0usize;
                    while i < x_pts.len() && j < y_pts.len() {
                        let tx = x_pts[i][0];
                        let ty = y_pts[j][0];
                        let dt = tx - ty;
                        if dt.abs() <= tol {
                            let x_lin = x_pts[i][1] + x_tr.offset;
                            let y_lin = y_pts[j][1] + y_tr.offset;
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
                                if x_lin > 0.0 {
                                    x_lin.log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                x_lin
                            };
                            derived.push([x, y]);
                            i += 1;
                            j += 1;
                        } else if dt < 0.0 {
                            i += 1;
                        } else {
                            j += 1;
                        }
                    }

                    if derived.is_empty() {
                        continue;
                    }

                    // Use per-pair style.
                    let mut color = pair_look.color;
                    let mut width: f32 = pair_look.width.max(0.1);
                    let style = pair_look.style;

                    let legend_label = if self.data.show_info_in_legend && !y_tr.info.is_empty() {
                        format!("{} vs {} — {}", y_name, x_name, y_tr.info)
                    } else {
                        format!("{} vs {}", y_name, x_name)
                    };

                    if let Some(hov) = &traces.hover_trace {
                        // Hover on either trace highlights this pair.
                        let is_pair_hover = *hov == x_name || *hov == y_name;
                        if !is_pair_hover {
                            color = Color32::from_rgba_unmultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                40,
                            );
                        } else {
                            width = (width * 1.6).max(width + 1.0);
                        }
                    }

                    plot_ui.line(
                        Line::new(legend_label.clone(), derived.clone())
                            .name(legend_label.clone())
                            .color(color)
                            .width(width)
                            .style(style),
                    );

                    let highlight_newest = pair_look.highlight_newest_point;

                    if pair_look.show_points {
                        let radius = pair_look.point_size.max(0.5);
                        plot_ui.points(
                            Points::new(legend_label.clone(), derived.clone())
                                .radius(radius)
                                .shape(pair_look.marker)
                                .color(color),
                        );
                    }

                    if highlight_newest {
                        // Add a second pass for the newest point with increased size.
                        let last = *derived.last().unwrap();
                        let radius = (pair_look.point_size.max(0.5) * 2.0).max(2.0);
                        plot_ui.points(
                            Points::new(format!("{legend_label} (last)"), vec![last])
                                .radius(radius)
                                .shape(pair_look.marker)
                                .color(color),
                        );
                    }
                }
            } else {
                for name in self.data.trace_order.clone().into_iter() {
                    if let Some(tr) = traces.get_trace(&name) {
                        if !tr.look.visible {
                            continue;
                        }
                        let shown_pts = match self.data.get_drawn_points(&name, traces) {
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
                        if let Some(hov) = &traces.hover_trace {
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
                            name.0.clone()
                        };
                        line = line.name(legend_label.clone());
                        plot_ui.line(line);

                        // Optional point markers for each datapoint
                        if tr.look.show_points {
                            if !pts_vec.is_empty() {
                                let mut radius = tr.look.point_size.max(0.5);
                                if let Some(hov) = &traces.hover_trace {
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
            }

            // Additional overlays provided by caller (e.g., thresholds, markers)
            draw_overlays(plot_ui, &self.data, traces);

            // Detect bounds changes via zoom box
            bounds_changed
        });

        // After plot: if bounds changed, sync time_window and Y limits from actual plot bounds
        if plot_resp.inner {
            // Disable auto-fit-to-view on manual user interaction
            self.data.auto_fit_to_view = false;

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

            // Emit zoom/pan event
            if let Some(ctrl) = &self.event_ctrl {
                let mut evt = crate::events::PlotEvent::new(
                    crate::events::EventKind::ZOOM | crate::events::EventKind::PAN,
                );
                evt.view_change = Some(crate::events::ViewChangeMeta {
                    x_range: Some(self.data.x_axis.bounds),
                    y_range: Some(self.data.y_axis.bounds),
                    scope_id: Some(self.data.id),
                });
                ctrl.emit_filtered(evt);
            }
        }

        self.handle_plot_click(&plot_resp, traces);

        // Handle drag-drop of traces from the traces list onto the scope plot.
        self.handle_trace_drop(ui, &plot_resp.response);
    }

    /// Accept trace drops from the main traces table drag.
    fn handle_trace_drop(&mut self, ui: &mut Ui, plot_response: &egui::Response) {
        use super::scope_settings_ui::DragPayload;

        let drag_payload: Option<DragPayload> = ui
            .ctx()
            .data(|d| d.get_temp(egui::Id::new("liveplot_active_trace_drag")));

        if let Some(payload) = drag_payload {
            // Check if the pointer was released over this plot area
            let released = ui.ctx().input(|i| i.pointer.any_released());
            let hovered = plot_response.rect.contains(
                ui.ctx()
                    .pointer_latest_pos()
                    .unwrap_or(egui::Pos2::new(-1.0, -1.0)),
            );

            if released && hovered {
                let trace = payload.trace;
                // Add the trace if not already in this scope
                if !self.data.trace_order.iter().any(|t| t == &trace) {
                    self.data.trace_order.push(trace.clone());
                }
                // If dragged from another scope, remove from origin
                if let Some(origin_id) = payload.origin_scope_id {
                    if origin_id != self.data.id {
                        // We can't directly remove from the origin scope here since
                        // we don't have mutable access to it. Instead mark this as
                        // a move so the caller can handle the removal.
                        // For now, just add to this scope; the origin removal is
                        // handled by scope_settings_ui's existing drop logic.
                    }
                }
                // Clear the drag payload so other scopes don't also consume it
                ui.ctx().data_mut(|d| {
                    d.remove::<DragPayload>(egui::Id::new("liveplot_active_trace_drag"));
                });
            } else if hovered && !released {
                // Visual feedback: highlight the plot border while dragging over it
                ui.painter().rect_stroke(
                    plot_response.rect,
                    4.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 160, 255)),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }

    /// Handle click selection on the plot using nearest point logic.
    fn handle_plot_click(
        &mut self,
        plot_response: &egui_plot::PlotResponse<bool>,
        traces: &mut TracesCollection,
    ) {
        self.data.clicked_point = None;
        if plot_response.response.double_clicked() {
            self.data.fit_bounds(traces);
            self.data.auto_fit_to_view = true;
            // Emit double-click + fit-to-view events
            if let Some(ctrl) = &self.event_ctrl {
                let kinds =
                    crate::events::EventKind::DOUBLE_CLICK | crate::events::EventKind::FIT_TO_VIEW;
                let mut evt = crate::events::PlotEvent::new(kinds);
                // Try to get click position for metadata
                if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                    let transform = plot_response.transform;
                    let plot_pos = transform.value_from_position(screen_pos);
                    evt.click = Some(crate::events::ClickMeta {
                        screen_pos: Some(crate::events::ScreenPos {
                            x: screen_pos.x,
                            y: screen_pos.y,
                        }),
                        plot_pos: Some(crate::events::PlotPos {
                            x: plot_pos.x,
                            y: plot_pos.y,
                        }),
                        trace: None,
                        scope_id: Some(self.data.id),
                    });
                }
                let (xmin, xmax) = self.data.x_axis.bounds;
                let (ymin, ymax) = self.data.y_axis.bounds;
                evt.view_change = Some(crate::events::ViewChangeMeta {
                    x_range: Some((xmin, xmax)),
                    y_range: Some((ymin, ymax)),
                    scope_id: Some(self.data.id),
                });
                ctrl.emit_filtered(evt);
            }
        } else if plot_response.response.clicked() {
            // optional feature flag – allow callers to turn off pause/resume-on-click
            if !self.data.pause_on_click {
                // Even with pausing disabled we still want measurement clicks when
                // already paused, and we always emit a plain click event.
                if self.data.paused && self.data.measurement_active {
                    if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                        let transform = plot_response.transform;
                        let plot_pos = transform.value_from_position(screen_pos);

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
                if let Some(ctrl) = &self.event_ctrl {
                    let mut evt = crate::events::PlotEvent::new(crate::events::EventKind::CLICK);
                    if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                        let transform = plot_response.transform;
                        let plot_pos = transform.value_from_position(screen_pos);
                        evt.click = Some(crate::events::ClickMeta {
                            screen_pos: Some(crate::events::ScreenPos {
                                x: screen_pos.x,
                                y: screen_pos.y,
                            }),
                            plot_pos: Some(crate::events::PlotPos {
                                x: plot_pos.x,
                                y: plot_pos.y,
                            }),
                            trace: None,
                            scope_id: Some(self.data.id),
                        });
                    }
                    ctrl.emit_filtered(evt);
                }
                return;
            }

            if self.data.paused {
                if self.data.measurement_active {
                    // Measurement is active – set clicked point without resuming
                    // so the measurement panel can pick up the new point.
                    if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                        let transform = plot_response.transform;
                        let plot_pos = transform.value_from_position(screen_pos);

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

                        // Emit click event (measurement point will be emitted by measurement panel)
                        if let Some(ctrl) = &self.event_ctrl {
                            let mut evt =
                                crate::events::PlotEvent::new(crate::events::EventKind::CLICK);
                            evt.click = Some(crate::events::ClickMeta {
                                screen_pos: Some(crate::events::ScreenPos {
                                    x: screen_pos.x,
                                    y: screen_pos.y,
                                }),
                                plot_pos: Some(crate::events::PlotPos {
                                    x: x_plot,
                                    y: y_plot,
                                }),
                                trace: None,
                                scope_id: Some(self.data.id),
                            });
                            ctrl.emit_filtered(evt);
                        }
                    }
                } else {
                    // No measurement active – resume on click.
                    self.data.paused = false;
                    if let Some(ctrl) = &self.event_ctrl {
                        let mut evt = crate::events::PlotEvent::new(
                            crate::events::EventKind::CLICK | crate::events::EventKind::RESUME,
                        );
                        evt.pause = Some(crate::events::PauseMeta {
                            scope_id: Some(self.data.id),
                        });
                        if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                            let transform = plot_response.transform;
                            let plot_pos = transform.value_from_position(screen_pos);
                            evt.click = Some(crate::events::ClickMeta {
                                screen_pos: Some(crate::events::ScreenPos {
                                    x: screen_pos.x,
                                    y: screen_pos.y,
                                }),
                                plot_pos: Some(crate::events::PlotPos {
                                    x: plot_pos.x,
                                    y: plot_pos.y,
                                }),
                                trace: None,
                                scope_id: Some(self.data.id),
                            });
                        }
                        ctrl.emit_filtered(evt);
                    }
                }
            } else {
                self.data.paused = true;
                traces.take_snapshot();

                if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                    let transform = plot_response.transform;
                    let plot_pos = transform.value_from_position(screen_pos);

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

                    // Emit click + pause events
                    if let Some(ctrl) = &self.event_ctrl {
                        let mut evt = crate::events::PlotEvent::new(
                            crate::events::EventKind::CLICK | crate::events::EventKind::PAUSE,
                        );
                        evt.click = Some(crate::events::ClickMeta {
                            screen_pos: Some(crate::events::ScreenPos {
                                x: screen_pos.x,
                                y: screen_pos.y,
                            }),
                            plot_pos: Some(crate::events::PlotPos {
                                x: x_plot,
                                y: y_plot,
                            }),
                            trace: None,
                            scope_id: Some(self.data.id),
                        });
                        evt.pause = Some(crate::events::PauseMeta {
                            scope_id: Some(self.data.id),
                        });
                        ctrl.emit_filtered(evt);
                    }
                }
            }
        }
    }

    pub fn fit_all(&mut self, traces: &TracesCollection) {
        self.data.fit_bounds(traces);
    }
}
