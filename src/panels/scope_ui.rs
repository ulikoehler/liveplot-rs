use egui::Ui;
use egui_plot::{Legend, Line, Plot, Points};

// no XDateFormat needed in this panel for now

use super::panel_trait::{Panel, PanelState};
use crate::data::DataContext;
use chrono::Local;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ZoomMode {
    Off,
    X,
    Y,
    Both,
}

pub struct ScopePanel {
    pub state: PanelState,
    // Zoom/fit state
    zoom_mode: ZoomMode,
    pending_auto_x: bool,
    pending_auto_y: bool,
    auto_zoom_y: bool,
    y_min: f64,
    y_max: f64,
    show_thresholds: bool,
    show_threshold_events: bool,
}
impl Default for ScopePanel {
    fn default() -> Self {
        Self {
            state: PanelState {
                visible: true,
                detached: false,
            },
            zoom_mode: ZoomMode::X,
            pending_auto_x: false,
            pending_auto_y: false,
            auto_zoom_y: false,
            y_min: 0.0,
            y_max: 1.0,
            show_thresholds: true,
            show_threshold_events: false,
        }
    }
}

impl Panel for ScopePanel {
    fn name(&self) -> &'static str { "Scope" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_menu(&mut self, ui: &mut Ui, data: &mut DataContext) {
        // Main top-bar controls grouped similarly to old controls_ui
        ui.label("X-Axis Time:");
        let mut tw = data.traces.time_window.max(1e-9);
        static mut TW_MIN: f64 = 0.1;
        static mut TW_MAX: f64 = 100.0;
        let (mut tw_min, mut tw_max) = unsafe { (TW_MIN, TW_MAX) };
        if tw <= tw_min { tw_min /= 10.0; }
        if tw >= tw_max { tw_min *= 10.0; tw_max *= 10.0; }
        let slider = egui::Slider::new(&mut tw, tw_min..=tw_max)
            .logarithmic(true)
            .smart_aim(true)
            .show_value(true)
            .suffix(" s");
        if ui.add(slider).changed() { data.traces.time_window = tw; }

        ui.separator();
        ui.label("Points:");
        ui.add(egui::Slider::new(&mut data.traces.max_points, 5_000..=200_000));

        if ui.button("Fit X").on_hover_text("Fit X to visible data").clicked() { self.pending_auto_x = true; }

        ui.separator();

        // Y controls
        let mut y_min_tmp = self.y_min;
        let mut y_max_tmp = self.y_max;
        let y_range = y_max_tmp - y_min_tmp;
        ui.label("Y Min:");
        let r1 = ui.add(
            egui::DragValue::new(&mut y_min_tmp).speed(0.1).custom_formatter(|n,_|{
                if let Some(unit) = &data.traces.y_unit {
                    if y_range.abs() < 0.001 { let exponent = y_range.abs().max(1e-12).log10().floor() + 1.0; format!("{:.1}e{} {}", n/10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                } else {
                    if y_range.abs() < 0.001 { let exponent = y_range.abs().max(1e-12).log10().floor() + 1.0; format!("{:.1}e{}", n/10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                }
            })
        );
        ui.label("Max:");
        let r2 = ui.add(
            egui::DragValue::new(&mut y_max_tmp).speed(0.1).custom_formatter(|n,_|{
                if let Some(unit) = &data.traces.y_unit {
                    if y_range.abs() < 0.001 { let exponent = y_range.abs().max(1e-12).log10().floor() + 1.0; format!("{:.1}e{} {}", n/10f64.powf(exponent), exponent, unit) } else { format!("{:.3} {}", n, unit) }
                } else {
                    if y_range.abs() < 0.001 { let exponent = y_range.abs().max(1e-12).log10().floor() + 1.0; format!("{:.1}e{}", n/10f64.powf(exponent), exponent) } else { format!("{:.3}", n) }
                }
            })
        );
        if (r1.changed() || r2.changed()) && y_min_tmp < y_max_tmp { self.y_min = y_min_tmp; self.y_max = y_max_tmp; self.pending_auto_y = false; }
        if ui.button("Fit Y").on_hover_text("Fit Y to visible data").clicked() { self.pending_auto_y = true; }
        ui.checkbox(&mut self.auto_zoom_y, "Auto Zoom Y");
        if self.auto_zoom_y { self.pending_auto_y = true; }

        ui.separator();
        ui.label("Zoom:");
        ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off");
        ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X");
        ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y");
        ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both");

        ui.separator();
        if ui.button(if data.traces.y_log { "Y: Log10" } else { "Y: Linear" }).clicked() { data.traces.y_log = !data.traces.y_log; }
        ui.checkbox(&mut data.traces.show_info_in_legend, "Legend info");

    ui.separator();
    ui.checkbox(&mut self.show_thresholds, "Threshold lines");
    ui.checkbox(&mut self.show_threshold_events, "Threshold events");

        // Additional grouped menu for quick actions if needed
        ui.menu_button("More", |ui| {
            if ui.button("Fit X").on_hover_text("Fit the X-axis to visible data").clicked() {
                self.pending_auto_x = true;
                ui.close();
            }
            if ui.button("Fit Y").on_hover_text("Fit the Y-axis to visible data").clicked() {
                self.pending_auto_y = true;
                ui.close();
            }
            if ui.button("Fit to View").on_hover_text("Fit both axes to visible data").clicked() {
                self.pending_auto_x = true;
                self.pending_auto_y = true;
                ui.close();
            }
            ui.separator();
            ui.checkbox(&mut self.auto_zoom_y, "Auto Zoom Y")
                .on_hover_text("Continuously fit Y to visible data");
            if self.auto_zoom_y { self.pending_auto_y = true; }
            ui.separator();
            ui.label("Wheel zoom mode:");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Off, "Off");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::X, "X");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Y, "Y");
                ui.selectable_value(&mut self.zoom_mode, ZoomMode::Both, "Both");
            });
            ui.separator();
            // Basic Y settings
            ui.label("Y unit and scale:");
            if ui.button(if data.traces.y_log { "Y: Log10" } else { "Y: Linear" }).clicked() {
                data.traces.y_log = !data.traces.y_log;
            }
        });
    }
    fn render_panel(&mut self, ui: &mut Ui, data: &mut DataContext) {
        // No extra controls in panel; top bar uses render_menu
        // Render plot directly here (for now). Later we can separate draw() if needed.
    let y_unit = data.traces.y_unit.clone();
    let y_log = data.traces.y_log;
    let plot = Plot::new("scope_plot")
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(true)
            .legend(Legend::default())
            .x_axis_formatter(|x, _range| {
                let val = x.value;
                let secs = val as i64;
                let nsecs = ((val - secs as f64) * 1e9) as u32;
                let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
            })
            .y_axis_formatter(move |y, _range| {
                // Scientific ticks with optional unit, apply inverse log mapping for display
                let v_plot = y.value;
                let step = y.step_size.abs();
                let label_val = if y_log { 10f64.powf(v_plot) } else { v_plot };
                let sci = step < 1e-3 || step >= 1e4;
                match &y_unit {
                    Some(unit) => {
                        if sci {
                            // determine exponent from step if possible
                            let exp = if step > 0.0 { step.log10().floor() as i32 } else { 0 };
                            let base = 10f64.powi(exp);
                            format!("{:.3}e{} {}", label_val / base, exp, unit)
                        } else {
                            format!("{:.3} {}", label_val, unit)
                        }
                    }
                    None => {
                        if sci {
                            let exp = if step > 0.0 { step.log10().floor() as i32 } else { 0 };
                            let base = 10f64.powi(exp);
                            format!("{:.3}e{}", label_val / base, exp)
                        } else {
                            format!("{:.3}", label_val)
                        }
                    }
                }
            });

        let plot_resp = plot.show(ui, |plot_ui| {
            // Determine latest time for auto-follow
            let mut t_latest = f64::NEG_INFINITY;
            for name in data.traces.trace_order.iter() {
                if let Some(tr) = data.traces.traces.get(name) {
                    if let Some(&[t, _]) = tr.live.back() {
                        if t > t_latest {
                            t_latest = t;
                        }
                    }
                }
            }

            // Handle wheel zoom around hovered point
            let resp = plot_ui.response();
            let scroll = resp.ctx.input(|i| i.raw_scroll_delta);
            let hovered = resp.hovered();
            let mut bounds_changed = false;
            if hovered && (scroll.x != 0.0 || scroll.y != 0.0) {
                let mut zx = 1.0f32;
                let mut zy = 1.0f32;
                if matches!(self.zoom_mode, ZoomMode::X | ZoomMode::Both) {
                    zx = 1.0 + scroll.y * 0.001;
                }
                if matches!(self.zoom_mode, ZoomMode::Y | ZoomMode::Both) {
                    zy = 1.0 + scroll.y * 0.001;
                }
                // Keep within sane bounds
                zx = zx.clamp(0.2, 5.0);
                zy = zy.clamp(0.2, 5.0);
                // Apply zoom around hovered
                plot_ui.zoom_bounds_around_hovered(egui::vec2(zx, zy));
                bounds_changed = true;
            }

            // Auto-fit requests
            if self.pending_auto_x {
                let mut xmin = f64::INFINITY;
                let mut xmax = f64::NEG_INFINITY;
                for tr in data.traces.traces.values() {
                    if !tr.look.visible { continue; }
                    if let (Some(&[t0, _]), Some(&[t1, _])) = (tr.live.front(), tr.live.back()) {
                        if t0 < xmin { xmin = t0; }
                        if t1 > xmax { xmax = t1; }
                    }
                }
                if xmin.is_finite() && xmax.is_finite() && xmax > xmin {
                    data.traces.time_window = (xmax - xmin).max(1e-9);
                }
                self.pending_auto_x = false;
            }
            if self.pending_auto_y || self.auto_zoom_y {
                // Fit Y to visible X range
                let bounds = plot_ui.plot_bounds();
                let xr = bounds.range_x();
                let xmin = *xr.start();
                let xmax = *xr.end();
                let mut ymin = f64::INFINITY;
                let mut ymax = f64::NEG_INFINITY;
                for tr in data.traces.traces.values() {
                    if !tr.look.visible { continue; }
                    for p in tr.live.iter() {
                        let x = p[0];
                        if x < xmin || x > xmax { continue; }
                        let y_lin = p[1] + tr.offset;
                        let y = if y_log { if y_lin > 0.0 { y_lin.log10() } else { continue; } } else { y_lin };
                        if y < ymin { ymin = y; }
                        if y > ymax { ymax = y; }
                    }
                }
                if ymin.is_finite() && ymax.is_finite() && ymax > ymin {
                    let pad = (ymax - ymin) * 0.05;
                    self.y_min = ymin - pad;
                    self.y_max = ymax + pad;
                }
                self.pending_auto_y = false;
            }

            // Apply bounds: X follows latest time using time_window; Y respects manual limits if valid
            if t_latest.is_finite() {
                plot_ui.set_plot_bounds_x(t_latest - data.traces.time_window..=t_latest);
            }
            if self.y_min.is_finite() && self.y_max.is_finite() && self.y_max > self.y_min {
                plot_ui.set_plot_bounds_y(self.y_min..=self.y_max);
            }

            // Draw traces
            for name in data.traces.trace_order.clone().into_iter() {
                if let Some(tr) = data.traces.traces.get(&name) {
                    if !tr.look.visible { continue; }
                    let pts: Vec<[f64; 2]> = tr
                        .live
                        .iter()
                        .map(|p| {
                            let y_lin = p[1] + tr.offset;
                            let y = if y_log {
                                if y_lin > 0.0 {
                                    y_lin.log10()
                                } else {
                                    f64::NAN
                                }
                            } else {
                                y_lin
                            };
                            [p[0], y]
                        })
                        .collect();
                    let mut line = Line::new(name.clone(), pts.clone())
                        .color(tr.look.color)
                        .width(tr.look.width)
                        .style(tr.look.style);
                    let legend_label = if data.traces.show_info_in_legend && !tr.info.is_empty() {
                        format!("{} — {}", name, tr.info)
                    } else {
                        name.clone()
                    };
                    line = line.name(legend_label);
                    plot_ui.line(line);

                    if tr.look.show_points && !pts.is_empty() {
                        let points = Points::new(format!("{}_pts", name), pts)
                            .radius(tr.look.point_size.max(0.5))
                            .shape(tr.look.marker)
                            .color(tr.look.color);
                        plot_ui.points(points);
                    }
                }
            }

            // Overlays: thresholds and events
            if self.show_thresholds || self.show_threshold_events {
                // Determine visible X for event culling
                let bounds = plot_ui.plot_bounds();
                let xr = bounds.range_x();
                let xmin = *xr.start();
                let xmax = *xr.end();
                let yr = bounds.range_y();
                let ymin = *yr.start();
                let ymax = *yr.end();

                for def in data.thresholds.defs.iter() {
                    let color = def
                        .color_hint
                        .map(|rgb| egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 200))
                        .unwrap_or(egui::Color32::from_rgba_unmultiplied(200, 200, 200, 200));
                    if self.show_thresholds {
                        match &def.kind {
                            crate::data::thresholds::ThresholdKind::GreaterThan { value }
                            | crate::data::thresholds::ThresholdKind::LessThan { value } => {
                                if !data.traces.y_log || *value > 0.0 {
                                    let y = if data.traces.y_log { value.log10() } else { *value };
                                    let pts = vec![[xmin, y], [xmax, y]];
                                    let line = Line::new(format!("thr:{}", def.name), pts)
                                        .color(color)
                                        .width(1.0)
                                        .style(egui_plot::LineStyle::Dashed { length: 6.0 });
                                    plot_ui.line(line);
                                }
                            }
                            crate::data::thresholds::ThresholdKind::InRange { low, high } => {
                                if !data.traces.y_log || *low > 0.0 {
                                    let y = if data.traces.y_log { low.log10() } else { *low };
                                    let pts = vec![[xmin, y], [xmax, y]];
                                    let line = Line::new(format!("thr:{}:low", def.name), pts)
                                        .color(color)
                                        .width(1.0)
                                        .style(egui_plot::LineStyle::Dashed { length: 6.0 });
                                    plot_ui.line(line);
                                }
                                if !data.traces.y_log || *high > 0.0 {
                                    let y = if data.traces.y_log { high.log10() } else { *high };
                                    let pts = vec![[xmin, y], [xmax, y]];
                                    let line = Line::new(format!("thr:{}:high", def.name), pts)
                                        .color(color)
                                        .width(1.0)
                                        .style(egui_plot::LineStyle::Dashed { length: 6.0 });
                                    plot_ui.line(line);
                                }
                            }
                        }
                    }
                    if self.show_threshold_events {
                        // render start/end as vertical lines from the global log
                        let ev_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120);
                        for e in data.thresholds.event_log.iter().filter(|e| e.threshold == def.name) {
                            if e.end_t < xmin || e.start_t > xmax { continue; }
                            let pts_s = vec![[e.start_t, ymin], [e.start_t, ymax]];
                            let pts_e = vec![[e.end_t, ymin], [e.end_t, ymax]];
                            let line_s = Line::new(format!("thr_evt_start:{}:{:.6}", def.name, e.start_t), pts_s)
                                .color(ev_color)
                                .width(0.75)
                                .style(egui_plot::LineStyle::Dotted { spacing: 3.0 });
                            let line_e = Line::new(format!("thr_evt_end:{}:{:.6}", def.name, e.end_t), pts_e)
                                .color(ev_color)
                                .width(0.75)
                                .style(egui_plot::LineStyle::Dotted { spacing: 3.0 });
                            plot_ui.line(line_s);
                            plot_ui.line(line_e);
                        }
                    }
                }
            }

            // Detect bounds changes via zoom box
            bounds_changed
        });

        // After plot: if bounds changed, sync time_window and Y limits from actual plot bounds
        if plot_resp.inner {
            let b = plot_resp.transform.bounds();
            let xr = b.range_x();
            let xw = (*xr.end() - *xr.start()).abs();
            if xw.is_finite() && xw > 0.0 {
                data.traces.time_window = xw;
            }
            let yr = b.range_y();
            let ymin = *yr.start();
            let ymax = *yr.end();
            if ymin.is_finite() && ymax.is_finite() && ymax > ymin {
                self.y_min = ymin;
                self.y_max = ymax;
            }
        }
    }
}
