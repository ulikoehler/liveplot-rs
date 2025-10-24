//! Plotting logic for `LivePlotApp`.
//!
//! This module encapsulates the central plot rendering and related interactions:
//! - drawing traces, thresholds, and measurement overlays
//! - handling zoom/pan and auto-fit behavior
//! - click selection with nearest-point snapping

use chrono::Local;
use egui::{Align2, Color32};
use egui_plot::{HLine, Line, Plot, PlotPoint, Points, Text, VLine};

use super::LivePlotApp;

impl LivePlotApp {
    /// Render the central plot inside the default central panel and apply interactions.
    pub(super) fn render_central_plot_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let plot_response = self.plot_traces_common(ui, ctx, "scope_plot_multi");
            self.pause_on_click(&plot_response);
            self.apply_zoom(&plot_response);
            self.handle_plot_click(&plot_response);
        });
    }

    /// Shared plot for both embedded and main variants. Returns (x_width, zoomed) and full response.
    pub(super) fn plot_traces_common(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        plot_id: &str,
    ) -> egui_plot::PlotResponse<bool> {
        let mut plot = Plot::new(plot_id)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(true)
            .x_axis_formatter(|x, _range| {
                let val = x.value;
                let secs = val as i64;
                let nsecs = ((val - secs as f64) * 1e9) as u32;
                let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
            })
            .y_axis_formatter(|y, _range| {
                let v = y.value;
                let step = y.step_size;
                let label_val = if self.y_log { 10f64.powf(v) } else { v };
                if let Some(unit) = &self.y_unit {
                    if step.abs() < 0.001 {
                        let exponent = step.log10().floor() + 1.0;
                        format!(
                            "{:.1}e{} {}",
                            label_val / 10f64.powf(exponent),
                            exponent,
                            unit
                        )
                    } else {
                        format!("{:.3} {}", label_val, unit)
                    }
                } else {
                    if step.abs() < 0.001 {
                        let exponent = step.log10().floor() + 1.0;
                        format!("{:.1}e{}", label_val / 10f64.powf(exponent), exponent)
                    } else {
                        format!("{:.3}", label_val)
                    }
                }
            });
        // Determine desired x-bounds for follow
        let t_latest = self.latest_time_overall().unwrap_or(0.0);

        if self.show_legend {
            plot = plot.legend(egui_plot::Legend::default());
        }
        let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
        let marker_font_size = base_body * 1.5;
        let plot_resp = plot.show(ui, |plot_ui| {
            // Handle zooming/panning/auto-zooming
            let resp = plot_ui.response();

            let is_zooming_rect = resp.drag_stopped_by(egui::PointerButton::Secondary);
            let is_panning =
                resp.dragged_by(egui::PointerButton::Primary) && resp.is_pointer_button_down_on();

            let scroll_data = resp.ctx.input(|i| i.raw_scroll_delta);
            let is_zooming_with_wheel =
                (scroll_data.x != 0.0 || scroll_data.y != 0.0) && resp.hovered();

            let bounds_changed =
                is_zooming_rect || is_panning || is_zooming_with_wheel || self.pending_auto_x;

            if is_zooming_with_wheel {
                let mut zoom_factor = egui::Vec2::new(1.0, 1.0);
                if scroll_data.y != 0.0
                    && (self.zoom_mode == super::app::ZoomMode::X || self.zoom_mode == super::app::ZoomMode::Both)
                {
                    zoom_factor.x = 1.0 + scroll_data.y * 0.001;
                } else if scroll_data.x != 0.0 {
                    zoom_factor.x = 1.0 - scroll_data.x * 0.001;
                }
                if self.zoom_mode == super::app::ZoomMode::Y || self.zoom_mode == super::app::ZoomMode::Both {
                    zoom_factor.y = 1.0 + scroll_data.y * 0.001;
                }

                if !self.paused {
                    plot_ui.set_plot_bounds_x(
                        t_latest - self.time_window * (2.0 - (zoom_factor.x as f64))..=t_latest,
                    );
                    zoom_factor.x = 1.0;
                }
                plot_ui.zoom_bounds_around_hovered(zoom_factor);
            } else if self.pending_auto_x {
                let mut xmin = f64::INFINITY;
                let mut xmax = f64::NEG_INFINITY;

                for tr in self.traces.values() {
                    if !tr.look.visible {
                        continue;
                    }

                    if self.paused {
                        if let Some(snap) = &tr.snap {
                            if let (Some(&[t_first, _]), Some(&[t_last, _])) =
                                (snap.front(), snap.back())
                            {
                                if t_first < xmin {
                                    xmin = t_first;
                                }
                                if t_last > xmax {
                                    xmax = t_last;
                                }
                            }
                        }
                    } else if let (Some(&[t_first, _]), Some(&[t_last, _])) =
                        (tr.live.front(), tr.live.back())
                    {
                        if t_first < xmin {
                            xmin = t_first;
                        }
                        if t_last > xmax {
                            xmax = t_last;
                        }
                    }
                }

                if xmin.is_finite() && xmax.is_finite() && xmin < xmax {
                    if !self.paused {
                        plot_ui.set_plot_bounds_x(t_latest - (xmax - xmin)..=t_latest);
                    } else {
                        plot_ui.set_plot_bounds_x(xmin..=xmax);
                    }
                }
                self.pending_auto_x = false;
            } else {
                if self.y_min.is_finite() && self.y_max.is_finite() && self.y_min < self.y_max {
                    let space = (self.y_max - self.y_min) * 0.05;
                    plot_ui.set_plot_bounds_y(self.y_min - space..=self.y_max + space);
                }
                if !self.paused {
                    plot_ui.set_plot_bounds_x(t_latest - self.time_window..=t_latest);
                } else {
                    let act_bounds = plot_ui.plot_bounds();
                    let xmax = act_bounds.range_x().end()
                        - (act_bounds.range_x().end()
                            - act_bounds.range_x().start()
                            - self.time_window)
                            / 2.0;
                    let xmin = xmax - self.time_window;
                    plot_ui.set_plot_bounds_x(xmin..=xmax);
                }
            }

            // Lines
            for name in self.trace_order.clone().into_iter() {
                if let Some(tr) = self.traces.get(&name) {
                    if !tr.look.visible { continue; }
                    let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                        if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                    } else { Box::new(tr.live.iter()) };
                    let pts_vec: Vec<[f64; 2]> = iter
                        .map(|p| {
                            let y_lin = p[1] + tr.offset;
                            let y = if self.y_log { if y_lin > 0.0 { y_lin.log10() } else { f64::NAN } } else { y_lin };
                            [p[0], y]
                        })
                        .collect();
                    let mut color = tr.look.color;
                    let mut width: f32 = tr.look.width.max(0.1);
                    let style = tr.look.style;
                    if let Some(hov) = &self.hover_trace {
                        if &tr.name != hov {
                            color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40);
                        } else {
                            width = (width * 1.6).max(width + 1.0);
                        }
                    }
                    let mut line = Line::new(&tr.name, pts_vec.clone())
                        .color(color)
                        .width(width)
                        .style(style);
                    let legend_label = if self.show_info_in_legend && !tr.info.is_empty() {
                        format!("{} — {}", tr.name, tr.info)
                    } else { tr.name.clone() };
                    line = line.name(legend_label);
                    plot_ui.line(line);

                    if tr.look.show_points && !pts_vec.is_empty() {
                        let mut radius = tr.look.point_size.max(0.5);
                        if let Some(hov) = &self.hover_trace {
                            if &tr.name == hov {
                                radius = (radius * 1.25).max(radius + 0.5);
                            }
                        }
                        let points = Points::new("", pts_vec.clone())
                            .radius(radius)
                            .shape(tr.look.marker)
                            .color(color);
                        plot_ui.points(points);
                    }
                }
            }

            // Threshold overlays
            if !self.threshold_defs.is_empty() {
                let bounds = plot_ui.plot_bounds();
                let xr = bounds.range_x();
                let xmin = *xr.start();
                let xmax = *xr.end();
                for def in &self.threshold_defs {
                    if let Some(tr) = self.traces.get(&def.target.0) {
                        if !tr.look.visible { continue; }
                        let thr_look = self.thresholds_panel.looks.get(&def.name).cloned().unwrap_or_else(|| {
                            let mut l = super::trace_look::TraceLook::default();
                            if let Some(rgb) = def.color_hint { l.color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]); } else { l.color = tr.look.color; }
                            l.width = 1.5; l
                        });
                        let ev_start_look = self.thresholds_panel.start_looks.get(&def.name).cloned().unwrap_or_else(|| {
                            let mut l = super::trace_look::TraceLook::default();
                            l.color = thr_look.color; l.width = 2.0; l
                        });
                        let ev_stop_look = self.thresholds_panel.stop_looks.get(&def.name).cloned().unwrap_or_else(|| {
                            let mut l = super::trace_look::TraceLook::default();
                            l.color = thr_look.color; l.width = 2.0; l
                        });
                        let mut thr_color = thr_look.color;
                        let mut thr_width = thr_look.width.max(0.1);
                        if let Some(hov_thr) = &self.hover_threshold {
                            if &def.name != hov_thr {
                                thr_color = Color32::from_rgba_unmultiplied(thr_color.r(), thr_color.g(), thr_color.b(), 60);
                            } else {
                                thr_width = (thr_width * 1.6).max(thr_width + 1.0);
                            }
                        }
                        let ev_base = thr_look.color;
                        let ev_color = if let Some(hov_thr) = &self.hover_threshold {
                            if &def.name != hov_thr { Color32::from_rgba_unmultiplied(ev_base.r(), ev_base.g(), ev_base.b(), 60) } else { ev_base }
                        } else { ev_base };

                        let mut draw_hline = |id: &str, label: Option<String>, y_world: f64| {
                            let y_lin = y_world + tr.offset;
                            let y_plot = if self.y_log { if y_lin > 0.0 { y_lin.log10() } else { f64::NAN } } else { y_lin };
                            if y_plot.is_finite() {
                                let mut h = HLine::new(id.to_string(), y_plot).color(thr_color).width(thr_width).style(thr_look.style);
                                if let Some(lbl) = &label { h = h.name(lbl.clone()); } else { h = h.name(""); }
                                plot_ui.hline(h);
                            }
                        };

                        let expr = match &def.kind {
                            crate::thresholds::ThresholdKind::GreaterThan { value } => {
                                if let Some(u) = &self.y_unit { format!("> {:.3} {}", value, u) } else { format!("> {:.3}", value) }
                            }
                            crate::thresholds::ThresholdKind::LessThan { value } => {
                                if let Some(u) = &self.y_unit { format!("< {:.3} {}", value, u) } else { format!("< {:.3}", value) }
                            }
                            crate::thresholds::ThresholdKind::InRange { low, high } => {
                                if let Some(u) = &self.y_unit { format!("[{:.3}, {:.3}] {}", low, high, u) } else { format!("[{:.3}, {:.3}]", low, high) }
                            }
                        };
                        let thr_info = format!("{} {}", def.target.0, expr);
                        let legend_label = if self.show_info_in_legend { format!("{} — {}", def.name, thr_info) } else { def.name.clone() };

                        match def.kind {
                            crate::thresholds::ThresholdKind::GreaterThan { value } => {
                                let id = format!("thr:{}", def.name);
                                draw_hline(&id, Some(legend_label), value);
                            }
                            crate::thresholds::ThresholdKind::LessThan { value } => {
                                let id = format!("thr:{}", def.name);
                                draw_hline(&id, Some(legend_label), value);
                            }
                            crate::thresholds::ThresholdKind::InRange { low, high } => {
                                let id_low = format!("thr:{}:low", def.name);
                                let id_high = format!("thr:{}:high", def.name);
                                draw_hline(&id_low, Some(legend_label), low);
                                draw_hline(&id_high, None, high);
                            }
                        }

                        if let Some(state) = self.threshold_states.get(&def.name) {
                            if ev_start_look.show_points || ev_stop_look.show_points {
                                let marker_y_world = match def.kind {
                                    crate::thresholds::ThresholdKind::GreaterThan { value } => value,
                                    crate::thresholds::ThresholdKind::LessThan { value } => value,
                                    crate::thresholds::ThresholdKind::InRange { low, high } => (low + high) * 0.5,
                                };
                                let y_lin = marker_y_world + tr.offset;
                                let marker_y_plot = if self.y_log { if y_lin > 0.0 { y_lin.log10() } else { f64::NAN } } else { y_lin };
                                if marker_y_plot.is_finite() {
                                    for ev in state.events.iter() {
                                        if ev.end_t < xmin || ev.start_t > xmax { continue; }
                                        if ev_start_look.show_points {
                                            let p = Points::new("", vec![[ev.start_t, marker_y_plot]])
                                                .radius(ev_start_look.point_size.max(0.5))
                                                .shape(ev_start_look.marker)
                                                .color(ev_color);
                                            plot_ui.points(p);
                                        } else {
                                            let s = VLine::new("", ev.start_t).color(ev_color).width(ev_start_look.width.max(0.1)).style(ev_start_look.style).name("");
                                            plot_ui.vline(s);
                                        }
                                        if ev_stop_look.show_points {
                                            let p = Points::new("", vec![[ev.end_t, marker_y_plot]])
                                                .radius(ev_stop_look.point_size.max(0.5))
                                                .shape(ev_stop_look.marker)
                                                .color(ev_color);
                                            plot_ui.points(p);
                                        } else {
                                            let e = VLine::new("", ev.end_t).color(ev_color).width(ev_stop_look.width.max(0.1)).style(ev_stop_look.style).name("");
                                            plot_ui.vline(e);
                                        }
                                    }
                                    if state.active {
                                        let start_t = state.start_t;
                                        let end_t = state.last_t.unwrap_or(start_t);
                                        if !(end_t < xmin || start_t > xmax) {
                                            if ev_start_look.show_points {
                                                let p = Points::new("", vec![[start_t, marker_y_plot]])
                                                    .radius(ev_start_look.point_size.max(0.5))
                                                    .shape(ev_start_look.marker)
                                                    .color(ev_color);
                                                plot_ui.points(p);
                                            } else {
                                                let s = VLine::new("", start_t).color(ev_color).width(ev_start_look.width.max(0.1)).style(ev_start_look.style).name("");
                                                plot_ui.vline(s);
                                            }
                                            if ev_stop_look.show_points {
                                                let p = Points::new("", vec![[end_t, marker_y_plot]])
                                                    .radius(ev_stop_look.point_size.max(0.5))
                                                    .shape(ev_stop_look.marker)
                                                    .color(ev_color);
                                                plot_ui.points(p);
                                            } else {
                                                let e = VLine::new("", end_t).color(ev_color).width(ev_stop_look.width.max(0.1)).style(ev_stop_look.style).name("");
                                                plot_ui.vline(e);
                                            }
                                        }
                                    }
                                }
                            } else {
                                for ev in state.events.iter() {
                                    if ev.end_t < xmin || ev.start_t > xmax { continue; }
                                    let ls = VLine::new("", ev.start_t).color(ev_color).width(ev_start_look.width.max(0.1)).style(ev_start_look.style).name("");
                                    plot_ui.vline(ls);
                                    let le = VLine::new("", ev.end_t).color(ev_color).width(ev_stop_look.width.max(0.1)).style(ev_stop_look.style).name("");
                                    plot_ui.vline(le);
                                }
                                if state.active {
                                    let start_t = state.start_t;
                                    let end_t = state.last_t.unwrap_or(start_t);
                                    if !(end_t < xmin || start_t > xmax) {
                                        let s = VLine::new("", start_t).color(ev_color).width(ev_start_look.width.max(0.1)).style(ev_start_look.style).name("");
                                        plot_ui.vline(s);
                                        let e = VLine::new("", end_t).color(ev_color).width(ev_stop_look.width.max(0.1)).style(ev_stop_look.style).name("");
                                        plot_ui.vline(e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Measurement overlays
            let p1_opt = self.point_selection.selected_p1;
            let p2_opt = self.point_selection.selected_p2;

            let ox = 0.01 * self.time_window;
            let oy = 0.01 * (self.y_max - self.y_min);

            let (dx, dy) = if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) { (p2[0] - p1[0], p2[1] - p1[1]) } else { (0.0, 0.0) };

            let label_pos = |dx: f64, dy: f64, p: &[f64; 2], ox: f64, oy: f64| -> (Align2, egui::Align, PlotPoint) {
                let slope = if dx != 0.0 || oy != 0.0 || ox != 0.0 { (dy / oy) / (dx / ox) } else { 0.0 };
                if dx <= 0.0 || slope.abs() > 8.0 {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (Align2::LEFT_TOP, egui::Align::LEFT, PlotPoint::new(p[0] + ox, p[1] - oy))
                    } else {
                        (Align2::LEFT_BOTTOM, egui::Align::LEFT, PlotPoint::new(p[0] + ox, p[1] + oy))
                    }
                } else {
                    if dy >= 0.0 || slope.abs() < 0.2 {
                        (Align2::RIGHT_TOP, egui::Align::RIGHT, PlotPoint::new(p[0] - ox, p[1] - oy))
                    } else {
                        (Align2::RIGHT_BOTTOM, egui::Align::RIGHT, PlotPoint::new(p[0] - ox, p[1] + oy))
                    }
                }
            };

            if let Some(p) = p1_opt {
                plot_ui.points(Points::new("Measurement", vec![p]).radius(5.0).color(Color32::YELLOW));
                let (halign_anchor, text_align, base) = label_pos(dx, dy, &p, ox, oy);
                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit { format!("{:.6} {}", y_lin, u) } else { format!("{:.6}", y_lin) };
                let txt = format!("P1\nx = {}\ny = {}", self.x_date_format.format_value(p[0]), ytxt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::YELLOW)
                    .append_to(&mut job, &style, egui::FontSelection::Default, text_align);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let Some(p) = p2_opt {
                plot_ui.points(Points::new("Measurement", vec![p]).radius(5.0).color(Color32::LIGHT_BLUE));
                let (halign_anchor, text_align, base) = label_pos(-dx, -dy, &p, ox, oy);
                let y_lin = if self.y_log { 10f64.powf(p[1]) } else { p[1] };
                let ytxt = if let Some(u) = &self.y_unit { format!("{:.6} {}", y_lin, u) } else { format!("{:.6}", y_lin) };
                let txt = format!("P2\nx = {}\ny = {}", self.x_date_format.format_value(p[0]), ytxt);
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_BLUE)
                    .append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::LEFT);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }
            if let (Some(p1), Some(p2)) = (p1_opt, p2_opt) {
                plot_ui.line(Line::new("Measurement", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                let dx = p2[0] - p1[0];
                let y1 = if self.y_log { 10f64.powf(p1[1]) } else { p1[1] };
                let y2 = if self.y_log { 10f64.powf(p2[1]) } else { p2[1] };
                let dy_lin = y2 - y1;
                let slope = if dx.abs() > 1e-12 { dy_lin / dx } else { f64::INFINITY };
                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                let dy_txt = if let Some(u) = &self.y_unit { format!("{:.6} {}", dy_lin, u) } else { format!("{:.6}", dy_lin) };
                let txt = if slope.is_finite() { format!("Δx={:.6}\nΔy={}\nslope={:.4}", dx, dy_txt, slope) } else { format!("Δx=0\nΔy={}\nslope=∞", dy_txt) };
                let slope_plot = if dx != 0.0 || oy != 0.0 || ox != 0.0 { (dy / oy) / (dx / ox) } else { 0.0 };
                let (halign_anchor, base) = if slope_plot.abs() > 8.0 {
                    (Align2::RIGHT_CENTER, PlotPoint::new(mid[0] - ox, mid[1]))
                } else if slope_plot.abs() < 0.2 {
                    (Align2::CENTER_BOTTOM, PlotPoint::new(mid[0], mid[1] + oy))
                } else if slope_plot >= 0.0 {
                    (Align2::LEFT_TOP, PlotPoint::new(mid[0] + ox, mid[1] - oy))
                } else {
                    (Align2::LEFT_BOTTOM, PlotPoint::new(mid[0] + ox, mid[1] + oy))
                };
                let style = egui::Style::default();
                let mut job = egui::text::LayoutJob::default();
                egui::RichText::new(txt)
                    .size(marker_font_size)
                    .color(Color32::LIGHT_GREEN)
                    .append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::LEFT);
                plot_ui.text(Text::new("Measurement", base, job).anchor(halign_anchor));
            }

            bounds_changed
        });
        plot_resp
    }

    pub(super) fn pause_on_click(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.response.clicked()
            || plot_response.response.dragged_by(egui::PointerButton::Secondary)
        {
            if !self.paused {
                self.paused = true;
                for tr in self.traces.values_mut() {
                    tr.snap = Some(tr.live.clone());
                }
            }
        }
    }

    /// Update zoom and pan state from plot response.
    pub(super) fn apply_zoom(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.inner {
            let bounds = plot_response.transform.bounds();
            let w = {
                let r = bounds.range_x();
                let (a, b) = (*r.start(), *r.end());
                (b - a).abs()
            };
            if w.is_finite() && w > 0.0 && (w - self.time_window).abs() / self.time_window.max(1e-6) > 0.02 {
                self.time_window = w;
            }
            let r = bounds.range_y();
            let ymin = *r.start();
            let ymax = *r.end();
            if ymin.is_finite() && ymax.is_finite() && ymin < ymax {
                let space = (0.05 / 1.1) * (ymax - ymin);
                self.y_min = ymin + space;
                self.y_max = ymax - space;
            }
        } else if self.pending_auto_y {
            let act_bounds = plot_response.transform.bounds();
            let mut ymin = f64::INFINITY;
            let mut ymax = f64::NEG_INFINITY;
            let rx = act_bounds.range_x();
            let (xmin, xmax) = (*rx.start(), *rx.end());
            for tr in self.traces.values() {
                if !tr.look.visible { continue; }
                let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                    if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                } else { Box::new(tr.live.iter()) };
                for p in iter {
                    let x = p[0];
                    if !(x >= xmin && x <= xmax) { continue; }
                    let y_lin = p[1] + tr.offset;
                    let y = if self.y_log { if y_lin > 0.0 { y_lin.log10() } else { continue; } } else { y_lin };
                    if y < ymin { ymin = y; }
                    if y > ymax { ymax = y; }
                }
            }
            self.y_min = ymin;
            self.y_max = ymax;
            self.pending_auto_y = false;
        }
    }

    /// Handle click selection on the plot using nearest point logic.
    pub(super) fn handle_plot_click(&mut self, plot_response: &egui_plot::PlotResponse<bool>) {
        if plot_response.response.clicked() {
            if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                let transform = plot_response.transform;
                let plot_pos = transform.value_from_position(screen_pos);
                let selected_trace_name = self.selection_trace.clone();
                let sel_data_points: Option<Vec<[f64; 2]>> = if let Some(name) = &selected_trace_name {
                    self.traces.get(name).map(|tr| {
                        let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                        } else { Box::new(tr.live.iter()) };
                        iter.cloned().collect()
                    })
                } else { None };
                match (&selected_trace_name, &sel_data_points) {
                    (Some(name), Some(data_points)) if !data_points.is_empty() => {
                        let off = self.traces.get(name).map(|t| t.offset).unwrap_or(0.0);
                        let mut best_i = None;
                        let mut best_d2 = f64::INFINITY;
                        for (i, p) in data_points.iter().enumerate() {
                            let x = p[0];
                            let y_lin = p[1] + off;
                            let y_plot = if self.y_log { if y_lin > 0.0 { y_lin.log10() } else { continue; } } else { y_lin };
                            let dx = x - plot_pos.x;
                            let dy = y_plot - plot_pos.y;
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_d2 { best_d2 = d2; best_i = Some(i); }
                        }
                        if let Some(i) = best_i {
                            let p = data_points[i];
                            let y_lin = p[1] + off;
                            let y_plot = if self.y_log { y_lin.log10() } else { y_lin };
                            self.point_selection.handle_click_point([p[0], y_plot]);
                        }
                    }
                    _ => {
                        self.point_selection.handle_click_point([plot_pos.x, plot_pos.y]);
                    }
                }
            }
        }
    }
}
