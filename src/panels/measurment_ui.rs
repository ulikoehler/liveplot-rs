use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::marker::Marker;
use crate::data::measurement::Measurement;
use crate::data::scope::{AxisSettings, ScopeData};
use egui::{Align2, Color32};
use egui_phosphor_icons::icons::{
    ASTERISK, BROOM, CIRCLE, CROSS, CROSSHAIR, DIAMOND, DOT, EYE, EYE_SLASH, LINE_VERTICAL, MINUS,
    PLUS, RULER, SQUARE,
};
use egui_plot::{HLine, Line, MarkerShape, PlotPoint, Points, Text, VLine};

pub struct MeasurementPanel {
    state: PanelState,
    measurements: Vec<Measurement>,
    selected_measurement: Option<usize>,
    selected_point_index: Option<usize>,
    last_clicked_point: Option<[f64; 2]>,
    hovered_measurement: Option<usize>,
    markers: Vec<Marker>,
    selected_marker: Option<usize>,
    hovered_marker: Option<usize>,
    /// Set when the marker list changed since the last sync collection so the
    /// host application can pick up the full list via
    /// [`LivePlotPanel::take_marker_change`](crate::LivePlotPanel::take_marker_change).
    markers_dirty: bool,
}

impl Default for MeasurementPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Measurement", RULER.as_str()),
            measurements: Vec::new(),
            selected_measurement: None,
            selected_point_index: None,
            last_clicked_point: None,
            hovered_measurement: None,
            markers: Vec::new(),
            selected_marker: None,
            hovered_marker: None,
            markers_dirty: false,
        }
    }
}

impl MeasurementPanel {
    fn trace_point_to_plot_coords(
        scope: &ScopeData,
        point: [f64; 2],
        offset: f64,
    ) -> Option<[f64; 2]> {
        let x_plot = if scope.x_axis.log_scale {
            (point[0] > 0.0).then(|| point[0].log10())?
        } else {
            point[0]
        };
        let y_linear = point[1] + offset;
        let y_plot = if scope.y_axis.log_scale {
            (y_linear > 0.0).then(|| y_linear.log10())?
        } else {
            y_linear
        };
        Some([x_plot, y_plot])
    }

    fn plot_point_to_screen(scope: &ScopeData, point: [f64; 2]) -> Option<[f64; 2]> {
        let ([x_min, x_max], [y_min, y_max]) = scope.last_plot_bounds?;
        let [left, top, right, bottom] = scope.last_plot_screen_rect?;
        let width = f64::from(right - left);
        let height = f64::from(bottom - top);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let x_span = x_max - x_min;
        let y_span = y_max - y_min;
        if x_span.abs() <= f64::EPSILON || y_span.abs() <= f64::EPSILON {
            return None;
        }

        let x = f64::from(left) + ((point[0] - x_min) / x_span) * width;
        let y = f64::from(bottom) - ((point[1] - y_min) / y_span) * height;
        Some([x, y])
    }

    /// Snap a clicked plot point to the nearest point of `catch_trace` (if any).
    ///
    /// Prefers screen-space distance when the click position is known, falls
    /// back to plot-space distance otherwise.  Returns the clicked point
    /// unchanged when no catch trace is set or snapping is not possible.
    fn snap_to_trace(
        scope: &ScopeData,
        traces: &crate::data::traces::TracesCollection,
        catch_trace: &Option<crate::TraceRef>,
        point: [f64; 2],
    ) -> [f64; 2] {
        let sel_data_points: Option<Vec<[f64; 2]>> = if let Some(name) = catch_trace {
            traces
                .get_points_ref(name, scope.paused)
                .map(|v| v.iter().copied().collect())
        } else {
            None
        };

        match (catch_trace, &sel_data_points) {
            (Some(name), Some(data_points)) if !data_points.is_empty() => {
                let off = traces.get_trace(name).map(|t| t.offset).unwrap_or(0.0);
                let clicked_screen = scope
                    .clicked_screen_pos
                    .map(|screen| [f64::from(screen[0]), f64::from(screen[1])]);
                let mut best_point = None;
                let mut best_d2 = f64::INFINITY;
                for p in data_points.iter() {
                    let Some(candidate_plot) = Self::trace_point_to_plot_coords(scope, *p, off)
                    else {
                        continue;
                    };
                    let (dx, dy) = if let (Some(clicked), Some(candidate_screen)) = (
                        clicked_screen,
                        Self::plot_point_to_screen(scope, candidate_plot),
                    ) {
                        (
                            candidate_screen[0] - clicked[0],
                            candidate_screen[1] - clicked[1],
                        )
                    } else {
                        (candidate_plot[0] - point[0], candidate_plot[1] - point[1])
                    };
                    let d2 = dx * dx + dy * dy;
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_point = Some(candidate_plot);
                    }
                }
                best_point.unwrap_or(point)
            }
            _ => point,
        }
    }

    /// Emit a marker event via the event controller (if attached).
    fn emit_marker_event(
        ctrl: &Option<crate::events::EventController>,
        kind: crate::events::EventKind,
        marker: &Marker,
    ) {
        if let Some(ctrl) = ctrl {
            let mut evt = crate::events::PlotEvent::new(kind);
            evt.marker = Some(crate::events::MarkerMeta {
                name: marker.name.clone(),
                point: marker.point,
                scope_id: marker.scope_id,
                trace: marker.catch_trace.clone(),
                color_rgba: Some([
                    marker.color.r(),
                    marker.color.g(),
                    marker.color.b(),
                    marker.color.a(),
                ]),
                visible: Some(marker.visible),
            });
            ctrl.emit_filtered(evt);
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
        for m in &mut self.measurements {
            m.clear();
        }
        for m in &mut self.markers {
            m.clear();
        }
        self.markers_dirty = true;
        self.selected_measurement = None;
        self.selected_point_index = None;
        self.last_clicked_point = None;
        self.hovered_measurement = None;
        self.selected_marker = None;
        self.hovered_marker = None;
    }

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::Measurements)
    }

    fn render_menu(
        &mut self,
        ui: &mut egui::Ui,
        data: &mut LivePlotData<'_>,
        collapsed: bool,
        tooltip: &str,
    ) {
        let label = if collapsed {
            self.icon_only()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.title().to_string())
        } else {
            self.title_and_icon()
        };
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let mr = egui::containers::menu::MenuButton::new(label)
            .config(menu_cfg)
            .ui(ui, |ui| {
                if ui.button("Show Measurements").clicked() {
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }

                ui.separator();

                if ui.button(format!("{} New", PLUS.as_str())).clicked() {
                    let idx = self.measurements.len() + 1;
                    self.measurements
                        .push(Measurement::new(&format!("M{}", idx)));
                    // Focus this panel
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }
                if ui
                    .button(format!("{} Clear All", BROOM.as_str()))
                    .on_hover_text("Clear measurement markers across all scopes")
                    .clicked()
                {
                    self.clear_all();
                    for scope in data.scope_data.iter_mut() {
                        let scope = &mut **scope;
                        scope.clicked_point = None;
                    }
                    ui.close();
                }
                if ui
                    .button(format!("{} Take P1 at click", CROSSHAIR.as_str()))
                    .clicked()
                {
                    self.selected_point_index = Some(0);
                    self.selected_marker = None;
                    ui.close();
                }
                if ui
                    .button(format!("{} Take P2 at click", CROSSHAIR.as_str()))
                    .clicked()
                {
                    self.selected_point_index = Some(1);
                    self.selected_marker = None;
                    ui.close();
                }

                ui.separator();

                if ui.button(format!("{} New Marker", PLUS.as_str())).clicked() {
                    let idx = self.markers.len() + 1;
                    self.markers
                        .push(Marker::new(&format!("K{}", idx), self.markers.len()));
                    self.selected_marker = Some(self.markers.len() - 1);
                    self.selected_measurement = None;
                    self.selected_point_index = None;
                    self.markers_dirty = true;
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }
                if ui
                    .button(format!("{} Clear All Markers", BROOM.as_str()))
                    .on_hover_text("Clear marker points across all scopes")
                    .clicked()
                {
                    for m in &mut self.markers {
                        m.clear();
                    }
                    self.selected_marker = None;
                    self.hovered_marker = None;
                    self.markers_dirty = true;
                    ui.close();
                }
            });
        if !tooltip.is_empty() {
            mr.0.on_hover_text(tooltip);
        }
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        if data.pending_requests.clear_measurements {
            self.clear_all();
            data.pending_requests.clear_measurements = false;

            // ── Emit MEASUREMENT_CLEARED event ──────────────────────────
            if let Some(ctrl) = &data.event_ctrl {
                let evt =
                    crate::events::PlotEvent::new(crate::events::EventKind::MEASUREMENT_CLEARED);
                ctrl.emit_filtered(evt);
            }
        }

        if data.pending_requests.clear_markers {
            for m in &mut self.markers {
                m.clear();
            }
            self.selected_marker = None;
            self.hovered_marker = None;
            self.markers_dirty = true;
            data.pending_requests.clear_markers = false;

            // ── Emit MARKER_CLEARED event ───────────────────────────────
            if let Some(ctrl) = &data.event_ctrl {
                let evt = crate::events::PlotEvent::new(crate::events::EventKind::MARKER_CLEARED);
                ctrl.emit_filtered(evt);
            }
        }

        // Tell each scope whether a measurement or marker is active so
        // clicking while paused sets a clicked_point instead of resuming.
        let has_measurements = !self.measurements.is_empty();
        let has_markers = !self.markers.is_empty();
        for scope in data.scope_data.iter_mut() {
            scope.measurement_active = has_measurements || has_markers;

            // Compute the X-coordinate range of all measurement/marker points
            // on this scope so that live_update can extend x_axis.bounds to
            // keep the markers visible after the scope is resumed.
            let scope_id = scope.id;
            let mut x_min = f64::INFINITY;
            let mut x_max = f64::NEG_INFINITY;
            let mut found = false;
            for m in &self.measurements {
                if m.scope_id == Some(scope_id) {
                    if let Some(p) = m.p1 {
                        x_min = x_min.min(p[0]);
                        x_max = x_max.max(p[0]);
                        found = true;
                    }
                    if let Some(p) = m.p2 {
                        x_min = x_min.min(p[0]);
                        x_max = x_max.max(p[0]);
                        found = true;
                    }
                }
            }
            // Markers are drawn on every scope (internal sync), so include
            // their x positions in every scope's range.
            for m in &self.markers {
                if let Some(p) = m.point {
                    x_min = x_min.min(p[0]);
                    x_max = x_max.max(p[0]);
                    found = true;
                }
            }
            scope.measurement_x_range = if found { Some((x_min, x_max)) } else { None };
        }

        for scope in data.scope_data.iter_mut() {
            let scope = &mut **scope;
            if let Some(point) = scope.clicked_point {
                if self.last_clicked_point == Some(point) {
                    continue;
                }
                self.last_clicked_point = Some(point);

                // Only set points when at least one measurement or marker
                // exists. If none exist, the click just toggles pause (handled
                // by scope_ui) and we skip measurement processing.
                if self.measurements.is_empty() && self.markers.is_empty() {
                    continue;
                }

                // A selected marker receives the click (exclusive selection).
                if self
                    .selected_marker
                    .is_some_and(|i| i >= self.markers.len())
                {
                    self.selected_marker = None;
                }
                if let Some(marker_idx) = self.selected_marker {
                    let catch = self.markers[marker_idx].catch_trace.clone();
                    let point = Self::snap_to_trace(scope, data.traces, &catch, point);
                    let marker = &mut self.markers[marker_idx];
                    marker.point = Some(point);
                    marker.scope_id = Some(scope.id);
                    let marker_snapshot = marker.clone();
                    self.markers_dirty = true;
                    Self::emit_marker_event(
                        &data.event_ctrl,
                        crate::events::EventKind::MARKER_SET,
                        &marker_snapshot,
                    );
                    continue;
                }

                if self.measurements.is_empty() {
                    continue;
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

                let point =
                    Self::snap_to_trace(scope, data.traces, &measurement.catch_trace, point);

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

                measurement.scope_id = Some(scope.id);

                // ── Emit MEASUREMENT_POINT event ──────────────────────────
                if let Some(ctrl) = &data.event_ctrl {
                    let (p1, p2) = measurement.get_points();
                    let (slope, distance, delta_x, delta_y) = if let (Some(a), Some(b)) = (p1, p2) {
                        let dx = b[0] - a[0];
                        let dy = b[1] - a[1];
                        let slope = if dx.abs() > 1e-12 {
                            Some(dy / dx)
                        } else {
                            None
                        };
                        let dist = (dx * dx + dy * dy).sqrt();
                        (slope, Some(dist), Some(dx), Some(dy))
                    } else {
                        (None, None, None, None)
                    };
                    let kind = if p1.is_some() && p2.is_some() {
                        crate::events::EventKind::MEASUREMENT_POINT
                            | crate::events::EventKind::MEASUREMENT_COMPLETE
                    } else {
                        crate::events::EventKind::MEASUREMENT_POINT
                    };
                    let point_index =
                        self.selected_point_index
                            .unwrap_or(if p2.is_some() { 1 } else { 0 });
                    let mut evt = crate::events::PlotEvent::new(kind);
                    evt.measurement = Some(crate::events::MeasurementMeta {
                        point_index,
                        point,
                        measurement_name: Some(measurement.name.clone()),
                        p1,
                        p2,
                        delta_x,
                        delta_y,
                        slope,
                        distance,
                        trace: measurement.catch_trace.clone(),
                    });
                    ctrl.emit_filtered(evt);
                }
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
        let base_body = plot_ui.ctx().global_style().text_styles[&egui::TextStyle::Body].size;
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
                plot_ui.points(Points::new(&name, vec![p]).radius(5.0_f32).color(c_p1));
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
                let x_txt = scope.x_axis.format_value(x_lin, Some(x_range));
                let y_txt = scope.y_axis.format_value(y_lin, Some(y_range));
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
                plot_ui.points(Points::new(&name, vec![p]).radius(5.0_f32).color(c_p2));
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
                let x_txt = scope.x_axis.format_value(x_lin, Some(x_range));
                let y_txt = scope.y_axis.format_value(y_lin, Some(y_range));
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
                let txt = format!(
                    "{}:\n{}",
                    name,
                    self.format_delta_summary(
                        scope,
                        DeltaSummaryArgs {
                            dx_lin,
                            dy_lin,
                            slope,
                            x_range: x_max_lin - x_min_lin,
                            y_range,
                            multiline: true,
                        },
                    )
                );
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

        // Marker overlays (drawn on every scope so markers stay in sync
        // across all scopes of this panel).
        let hovered_marker = self.hovered_marker;
        for (ki, marker) in self.markers.iter().enumerate() {
            if !marker.visible {
                continue;
            }
            let Some(p) = marker.point else {
                continue;
            };
            let name = marker.name.clone();
            let dimmed = matches!(hovered_marker, Some(h) if h != ki);
            let c = if dimmed {
                Color32::from_rgba_unmultiplied(
                    marker.color.r(),
                    marker.color.g(),
                    marker.color.b(),
                    60,
                )
            } else {
                marker.color
            };

            if marker.show_vline {
                plot_ui.vline(VLine::new(&name, p[0]).color(c));
            }
            if marker.show_hline {
                plot_ui.hline(HLine::new(&name, p[1]).color(c));
            }
            if marker.show_point {
                plot_ui.points(
                    Points::new(&name, vec![p])
                        .radius(5.0_f32)
                        .shape(marker.shape)
                        .color(c),
                );
            }

            // Name + value label next to the point.
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
            let x_txt = scope.x_axis.format_value(x_lin, Some(x_range));
            let y_txt = scope.y_axis.format_value(y_lin, Some(y_range));
            let txt = format!("{}\nx = {}\ny = {}", name, x_txt, y_txt);
            let style = egui::Style::default();
            let mut job = egui::text::LayoutJob::default();
            egui::RichText::new(txt)
                .size(marker_font_size)
                .color(c)
                .append_to(
                    &mut job,
                    &style,
                    egui::FontSelection::Default,
                    egui::Align::LEFT,
                );
            let base = PlotPoint::new(p[0] + ox, p[1] - oy);
            plot_ui.text(Text::new(&name, base, job).anchor(Align2::LEFT_TOP));
        }
    }

    fn render_panel(&mut self, ui: &mut egui::Ui, data: &mut LivePlotData<'_>) {
        ui.label("Pick points on the plot and compute deltas.");
        ui.horizontal(|ui| {
            if ui.button(format!("{} Add", PLUS.as_str())).clicked() {
                let idx = self.measurements.len() + 1;
                self.measurements
                    .push(Measurement::new(&format!("M{}", idx)));
                self.selected_measurement = Some(self.measurements.len() - 1);
                self.selected_point_index = None;
            }
            if ui.button(format!("{} Clear All", BROOM.as_str())).clicked() {
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
            ui.horizontal_wrapped(|ui| {
                let selected = self.selected_measurement == Some(i);
                let label = ui.selectable_label(selected, format!("#{}", i + 1));
                if label.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_marker = None;
                }

                let m = &mut self.measurements[i];
                let name_edit = ui.add_sized(
                    [120.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut m.name),
                );
                if name_edit.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_marker = None;
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

                let clear_btn = ui
                    .button(egui_phosphor_icons::icons::BROOM)
                    .on_hover_text("Clear");
                if clear_btn.clicked() {
                    m.clear();
                }

                let rm_btn = ui
                    .button(egui_phosphor_icons::icons::TRASH)
                    .on_hover_text("Remove");
                if rm_btn.clicked() {
                    remove_this = true;
                }
                if label.hovered() || name_edit.hovered() || clear_btn.hovered() || rm_btn.hovered()
                {
                    self.hovered_measurement = Some(i);
                };
            });

            let scope: &ScopeData = if let Some(scope_id) = self.measurements[i].scope_id {
                if let Some(scope) = data.scope_by_id(scope_id) {
                    scope
                } else {
                    self.measurements[i].clear();
                    continue;
                }
            } else if let Some(name) = &self.measurements[i].catch_trace {
                if let Some(scope) = data.scope_containing_trace(name) {
                    scope
                } else {
                    self.measurements[i].clear();
                    continue;
                }
            } else {
                self.measurements[i].clear();
                continue;
            };

            // Show values for P1/P2 and delta if available
            let (p1, p2) = self.measurements[i].get_points();
            let x_range = (scope.x_axis.bounds.1 - scope.x_axis.bounds.0).abs();
            let y_range = (scope.y_axis.bounds.1 - scope.y_axis.bounds.0).abs();
            let to_axis_value = |axis: &AxisSettings, v_plot: f64| -> f64 {
                if axis.log_scale && v_plot > 0.0 {
                    10f64.powf(v_plot)
                } else {
                    v_plot
                }
            };
            ui.horizontal_wrapped(|ui| {
                let mut p1_label = if let Some(p) = p1 {
                    let x_lin = to_axis_value(&scope.x_axis, p[0]);
                    let y_lin = to_axis_value(&scope.y_axis, p[1]);
                    let p1_text = format!(
                        "P1: x={}  y={}",
                        scope.x_axis.format_value(x_lin, Some(x_range)),
                        scope.y_axis.format_value(y_lin, Some(y_range))
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
                    self.selected_marker = None;
                }
                // double-click handled above when value exists

                let mut p2_label = if let Some(p) = p2 {
                    let x_lin = to_axis_value(&scope.x_axis, p[0]);
                    let y_lin = to_axis_value(&scope.y_axis, p[1]);
                    let p2_text = format!(
                        "P2: x={}  y={}",
                        scope.x_axis.format_value(x_lin, Some(x_range)),
                        scope.y_axis.format_value(y_lin, Some(y_range))
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
                    self.selected_marker = None;
                }
                // double-click handled above when value exists

                if p1_label.hovered() || p2_label.hovered() {
                    self.hovered_measurement = Some(i);
                };
            });
            if let (Some(p1), Some(p2)) = (p1, p2) {
                let x1_lin: f64 = to_axis_value(&scope.x_axis, p1[0]);
                let x2_lin = to_axis_value(&scope.x_axis, p2[0]);
                let y1_lin = to_axis_value(&scope.y_axis, p1[1]);
                let y2_lin = to_axis_value(&scope.y_axis, p2[1]);
                let dx_lin = x2_lin - x1_lin;
                let dy_lin = y2_lin - y1_lin;
                let slope_lin = if dx_lin.abs() > 1e-12 {
                    dy_lin / dx_lin
                } else {
                    f64::INFINITY
                };
                let diff_txt = self.format_delta_summary(
                    scope,
                    DeltaSummaryArgs {
                        dx_lin,
                        dy_lin,
                        slope: slope_lin,
                        x_range,
                        y_range,
                        multiline: false,
                    },
                );
                let mut diff_label = ui.colored_label(Color32::LIGHT_GREEN, diff_txt.clone());
                diff_label = diff_label.on_hover_text("Delta between P1 and P2");
                if diff_label.hovered() {
                    self.hovered_measurement = Some(i);
                };
                if diff_label.clicked() {
                    self.selected_measurement = Some(i);
                    self.selected_point_index = None;
                    self.selected_marker = None;
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

        // ── Markers section ─────────────────────────────────────────────
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Markers").strong());
            if ui.button(format!("{} Add", PLUS.as_str())).clicked() {
                let idx = self.markers.len();
                self.markers
                    .push(Marker::new(&format!("K{}", idx + 1), idx));
                self.selected_marker = Some(self.markers.len() - 1);
                self.selected_measurement = None;
                self.selected_point_index = None;
                self.markers_dirty = true;
                if let Some(m) = self.markers.last() {
                    Self::emit_marker_event(
                        &data.event_ctrl,
                        crate::events::EventKind::MARKER_ADDED,
                        m,
                    );
                }
            }
            if ui
                .button(format!("{} Clear All", BROOM.as_str()))
                .on_hover_text("Clear all marker points")
                .clicked()
            {
                for m in &mut self.markers {
                    m.clear();
                }
                self.markers_dirty = true;
            }
        });
        ui.add_space(6.0);
        self.hovered_marker = None;

        for i in 0..self.markers.len() {
            ui.separator();
            let mut remove_this = false;
            ui.horizontal_wrapped(|ui| {
                let selected = self.selected_marker == Some(i);
                let label = ui.selectable_label(selected, format!("#{}", i + 1));
                if label.clicked() {
                    self.selected_marker = Some(i);
                    self.selected_measurement = None;
                    self.selected_point_index = None;
                }

                let m = &mut self.markers[i];
                let name_edit = ui.add_sized(
                    [90.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut m.name),
                );
                if name_edit.clicked() {
                    self.selected_marker = Some(i);
                    self.selected_measurement = None;
                    self.selected_point_index = None;
                }
                if name_edit.changed() {
                    self.markers_dirty = true;
                }

                if ui.color_edit_button_srgba(&mut m.color).changed() {
                    self.markers_dirty = true;
                }

                let catch_trace_names: Vec<String> =
                    data.traces.keys().map(|name| name.0.clone()).collect();
                let mut selected_trace_name = m.catch_trace.clone();
                let old_selected = m.catch_trace.clone();
                let selected_text = match &selected_trace_name {
                    Some(t) => t.0.clone(),
                    None => "None".to_string(),
                };
                egui::ComboBox::from_id_salt(format!("marker_catch_trace_{}", i))
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
                if selected_trace_name != old_selected {
                    m.catch_trace = selected_trace_name.clone();
                    self.markers_dirty = true;
                }

                // Display-mode toggles (independent; can combine into a cross).
                if ui
                    .selectable_label(m.show_hline, MINUS.as_str())
                    .on_hover_text("Horizontal line")
                    .clicked()
                {
                    m.show_hline = !m.show_hline;
                    self.markers_dirty = true;
                }
                if ui
                    .selectable_label(m.show_vline, LINE_VERTICAL.as_str())
                    .on_hover_text("Vertical line")
                    .clicked()
                {
                    m.show_vline = !m.show_vline;
                    self.markers_dirty = true;
                }
                if ui
                    .selectable_label(m.show_point, DOT.as_str())
                    .on_hover_text("Point")
                    .clicked()
                {
                    m.show_point = !m.show_point;
                    self.markers_dirty = true;
                }

                let eye = if m.visible { EYE } else { EYE_SLASH };
                if ui
                    .button(eye)
                    .on_hover_text("Toggle marker visibility")
                    .clicked()
                {
                    m.visible = !m.visible;
                    self.markers_dirty = true;
                    Self::emit_marker_event(
                        &data.event_ctrl,
                        crate::events::EventKind::MARKER_SET,
                        m,
                    );
                }

                let clear_btn = ui
                    .button(egui_phosphor_icons::icons::BROOM)
                    .on_hover_text("Clear point");
                if clear_btn.clicked() {
                    m.clear();
                    self.markers_dirty = true;
                }

                let rm_btn = ui
                    .button(egui_phosphor_icons::icons::TRASH)
                    .on_hover_text("Remove");
                if rm_btn.clicked() {
                    remove_this = true;
                }
                if label.hovered() || name_edit.hovered() || clear_btn.hovered() || rm_btn.hovered()
                {
                    self.hovered_marker = Some(i);
                };
            });

            // Dot style selector (enabled only when the point display is on).
            ui.horizontal_wrapped(|ui| {
                ui.label("Style:");
                let show_point = self.markers[i].show_point;
                ui.add_enabled_ui(show_point, |ui| {
                    let shapes: [(MarkerShape, &str, &str); 6] = [
                        (MarkerShape::Circle, "Circle", CIRCLE.as_str()),
                        (MarkerShape::Diamond, "Diamond", DIAMOND.as_str()),
                        (MarkerShape::Square, "Square", SQUARE.as_str()),
                        (MarkerShape::Cross, "Cross", CROSS.as_str()),
                        (MarkerShape::Plus, "Plus", PLUS.as_str()),
                        (MarkerShape::Asterisk, "Asterisk", ASTERISK.as_str()),
                    ];
                    for (shape, name, icon) in shapes {
                        let sel = self.markers[i].shape == shape;
                        if ui.selectable_label(sel, icon).on_hover_text(name).clicked() {
                            self.markers[i].shape = shape;
                            self.markers_dirty = true;
                        }
                    }
                });
            });

            // Value row: show the marker position formatted with the axes of
            // the scope it was placed on (or the catch-trace's scope).
            let scope: Option<&ScopeData> = if let Some(scope_id) = self.markers[i].scope_id {
                data.scope_by_id(scope_id)
            } else if let Some(name) = &self.markers[i].catch_trace {
                data.scope_containing_trace(name)
            } else {
                data.primary_scope()
            };
            ui.horizontal_wrapped(|ui| {
                if let (Some(scope), Some(p)) = (scope, self.markers[i].point) {
                    let to_axis_value = |axis: &AxisSettings, v_plot: f64| -> f64 {
                        if axis.log_scale && v_plot > 0.0 {
                            10f64.powf(v_plot)
                        } else {
                            v_plot
                        }
                    };
                    let x_range = (scope.x_axis.bounds.1 - scope.x_axis.bounds.0).abs();
                    let y_range = (scope.y_axis.bounds.1 - scope.y_axis.bounds.0).abs();
                    let x_lin = to_axis_value(&scope.x_axis, p[0]);
                    let y_lin = to_axis_value(&scope.y_axis, p[1]);
                    let txt = format!(
                        "x={}  y={}",
                        scope.x_axis.format_value(x_lin, Some(x_range)),
                        scope.y_axis.format_value(y_lin, Some(y_range))
                    );
                    let marker_color = self.markers[i].color;
                    let mut lbl = ui.colored_label(marker_color, txt.clone());
                    lbl = lbl.on_hover_text("Click to reassign the marker point");
                    if lbl.clicked() {
                        self.selected_marker = Some(i);
                        self.selected_measurement = None;
                        self.selected_point_index = None;
                    }
                    if lbl.double_clicked() {
                        ui.ctx().copy_text(txt);
                    }
                    if lbl.hovered() {
                        self.hovered_marker = Some(i);
                    }
                } else {
                    ui.label("x=–  y=–  (select this marker, then click the plot)");
                }
            });

            if remove_this {
                if self.selected_marker == Some(i) {
                    self.selected_marker = None;
                }
                let removed = self.markers.remove(i);
                self.markers_dirty = true;
                Self::emit_marker_event(
                    &data.event_ctrl,
                    crate::events::EventKind::MARKER_REMOVED,
                    &removed,
                );
                break; // restart loop due to changed indices
            }
        }
    }

    fn settings_snapshot(&self, _data: &LivePlotData<'_>) -> Option<String> {
        let snap = crate::persistence::MeasurementPanelStateSerde::from_panel(self);
        serde_json::to_string(&snap).ok()
    }
}

struct DeltaSummaryArgs {
    dx_lin: f64,
    dy_lin: f64,
    slope: f64,
    x_range: f64,
    y_range: f64,
    multiline: bool,
}

impl MeasurementPanel {
    pub fn clear_all(&mut self) {
        for m in &mut self.measurements {
            m.clear();
        }
    }

    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    pub fn selected_measurement_index(&self) -> Option<usize> {
        self.selected_measurement
    }

    pub fn restore_measurements(
        &mut self,
        measurements: Vec<Measurement>,
        selected_measurement: Option<usize>,
    ) {
        self.measurements = measurements;
        self.selected_measurement =
            selected_measurement.filter(|idx| *idx < self.measurements.len());
        self.selected_point_index = None;
        self.last_clicked_point = None;
        self.hovered_measurement = None;
    }

    /// All markers currently managed by this panel.
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// Index of the marker that receives the next plot click, if any.
    pub fn selected_marker_index(&self) -> Option<usize> {
        self.selected_marker
    }

    /// Replace the marker list without marking it dirty.
    ///
    /// Used by persistence restore and by
    /// [`LivePlotPanel::set_markers`](crate::LivePlotPanel::set_markers) when an
    /// externally synchronized marker list is applied (loop suppression is
    /// handled by the caller).
    pub fn set_markers(&mut self, markers: Vec<Marker>) {
        self.markers = markers;
        if self
            .selected_marker
            .is_some_and(|idx| idx >= self.markers.len())
        {
            self.selected_marker = None;
        }
        self.hovered_marker = None;
    }

    /// Restore markers together with the selection (used by persistence).
    pub fn restore_markers(&mut self, markers: Vec<Marker>, selected_marker: Option<usize>) {
        self.set_markers(markers);
        self.selected_marker = selected_marker.filter(|idx| *idx < self.markers.len());
    }

    /// Consume the "markers changed" flag.
    ///
    /// Returns `true` exactly once after any marker mutation (add, remove,
    /// edit, point set, visibility toggle, clear) so the host can collect the
    /// full list for external synchronization.
    pub fn take_markers_dirty(&mut self) -> bool {
        std::mem::take(&mut self.markers_dirty)
    }

    /// Format Δx/Δy and slope consistently for UI and plot overlays.
    fn choose_time_unit_and_scale(delta_secs: f64) -> (&'static str, f64, usize) {
        // Return (unit_label, scale_multiplier, decimals)
        let a = delta_secs.abs();
        if a >= 1.0 {
            ("s", 1.0, 6)
        } else if a >= 1e-3 {
            ("ms", 1e3, 3)
        } else if a >= 1e-6 {
            ("us", 1e6, 0)
        } else {
            ("ns", 1e9, 0)
        }
    }

    fn format_delta_summary(&self, scope: &ScopeData, args: DeltaSummaryArgs) -> String {
        let DeltaSummaryArgs {
            dx_lin,
            dy_lin,
            slope,
            x_range,
            y_range,
            multiline,
        } = args;
        // Δx formatting: if x axis is time, show a duration using s/ms/us/ns; otherwise use axis formatting
        let (dx_txt, dx_unit_opt, x_scale) = match scope.x_axis.axis_type {
            crate::data::scope::AxisType::Time(_) => {
                let (u, scale, dec) = Self::choose_time_unit_and_scale(dx_lin);
                let val = dx_lin * scale;
                let s = if dec == 0 {
                    format!("{}", val.round() as i128)
                } else {
                    format!("{:.*}", dec, val)
                };
                (s + " " + u, Some(u.to_string()), scale)
            }
            _ => (
                scope.x_axis.format_value(dx_lin, Some(x_range)),
                scope.x_axis.get_unit(),
                1.0,
            ),
        };

        // Δy formatting
        let (dy_txt, dy_unit_opt, y_scale) = match scope.y_axis.axis_type {
            crate::data::scope::AxisType::Time(_) => {
                let (u, scale, dec) = Self::choose_time_unit_and_scale(dy_lin);
                let val = dy_lin * scale;
                let s = if dec == 0 {
                    format!("{}", val.round() as i128)
                } else {
                    format!("{:.*}", dec, val)
                };
                (s + " " + u, Some(u.to_string()), scale)
            }
            _ => (
                scope.y_axis.format_value(dy_lin, Some(y_range)),
                scope.y_axis.get_unit(),
                1.0,
            ),
        };

        if slope.is_finite() {
            // Compute displayed slope adjusting for unit scales: slope_display = slope * (y_scale / x_scale)
            let slope_disp = slope * (y_scale / x_scale);
            let num = scope.y_axis.format_value(slope_disp, None);

            // Build unit string from chosen units
            let unit_str = match (dy_unit_opt.as_deref(), dx_unit_opt.as_deref()) {
                (Some(_), Some(x)) => format!("/{}", x),
                (Some(_), None) => String::new(),
                (None, Some(x)) => format!(" 1/{}", x),
                (None, None) => String::new(),
            };

            if multiline {
                format!("Δx={}\nΔy={}\nslope={}{}", dx_txt, dy_txt, num, unit_str)
            } else {
                format!("Δx={}  Δy={}  slope={}{}", dx_txt, dy_txt, num, unit_str)
            }
        } else {
            if multiline {
                format!("Δx=0\nΔy={}\nslope=∞", dy_txt)
            } else {
                format!("Δx=0  Δy={}", dy_txt)
            }
        }
    }
}
