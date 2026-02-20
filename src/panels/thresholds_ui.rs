use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::scope::AxisSettings;
use crate::data::scope::ScopeData;
use crate::data::thresholds::{ThresholdDef, ThresholdEvent, ThresholdKind};
use crate::data::traces::TracesCollection;
use crate::panels::trace_look_ui::render_trace_look_editor;
use chrono::Local;
use egui;
use egui::{Color32, Ui};
use egui_plot::{HLine, LineStyle, MarkerShape, Points, VLine};
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};
use std::cmp::Ordering;
use std::collections::HashMap;

// Builder state removed; we edit a ThresholdDef directly

#[derive(Debug, Clone)]
pub struct ThresholdsPanel {
    state: PanelState,
    builder: ThresholdDef,
    pub editing: Option<String>,
    pub error: Option<String>,
    pub creating: bool,
    pub thresholds: HashMap<String, ThresholdDef>,
    pub events_filter: Option<String>,
    hover_threshold: Option<String>,
}

impl Default for ThresholdsPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Thresholds", "⚠"),
            builder: ThresholdDef::default(),
            editing: None,
            error: None,
            creating: false,
            thresholds: HashMap::new(),
            events_filter: None,
            hover_threshold: None,
        }
    }
}

impl Panel for ThresholdsPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::Thresholds)
    }

    fn render_menu(
        &mut self,
        ui: &mut Ui,
        _data: &mut LivePlotData<'_>,
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
        let mr = ui.menu_button(label, |ui| {
            if ui.button("Show Thresholds").clicked() {
                let st = self.state_mut();
                st.visible = true;
                st.request_focus = true;
                ui.close();
            }

            ui.separator();

            if ui.button("New").clicked() {
                self.builder = ThresholdDef::default();
                self.editing = None;
                self.creating = true;
                self.error = None;
                let st = self.state_mut();
                st.visible = true;
                st.detached = false;
                st.request_docket = true;
                ui.close();
            }
            if ui.button("X Clear events").clicked() {
                self.clear_all_events();
                ui.close();
            }
        });
        if !tooltip.is_empty() {
            mr.response.on_hover_text(tooltip);
        }
    }

    fn clear_all(&mut self) {
        self.events_filter = None;
        self.hover_threshold = None;
        // Clear per-threshold events (if they are stored within the defs) and reset map
        // Current implementation tracks events in runtime buffers associated with thresholds;
        // provide a bulk clear by calling the helper if available.
        self.clear_all_events();
    }

    fn draw(
        &mut self,
        plot_ui: &mut egui_plot::PlotUi,
        scope: &ScopeData,
        traces: &TracesCollection,
    ) {
        // Threshold overlays
        if !self.thresholds.is_empty() {
            let bounds = plot_ui.plot_bounds();
            let xr = bounds.range_x();
            let xmin = *xr.start();
            let xmax = *xr.end();
            for (_name, def) in &self.thresholds {
                if let Some(tr) = traces.get_trace(&def.target) {
                    if !tr.look.visible {
                        continue;
                    }
                    let mut thr_color = def.look.color;
                    let mut thr_expand_line = 1.0;
                    let mut thr_expand_points = 1.0;
                    if let Some(hov_thr) = &self.hover_threshold {
                        if &def.name != hov_thr {
                            thr_color = Color32::from_rgba_unmultiplied(
                                thr_color.r(),
                                thr_color.g(),
                                thr_color.b(),
                                60,
                            );
                        } else {
                            thr_color = Color32::from_rgba_unmultiplied(
                                thr_color.r(),
                                thr_color.g(),
                                thr_color.b(),
                                255,
                            );
                            thr_expand_line = 1.6;
                            thr_expand_points = 1.2;
                        }
                    }
                    let ev_base = def.look.color;
                    let ev_color = if let Some(hov_thr) = &self.hover_threshold {
                        if &def.name != hov_thr {
                            Color32::from_rgba_unmultiplied(
                                ev_base.r(),
                                ev_base.g(),
                                ev_base.b(),
                                60,
                            )
                        } else {
                            ev_base
                        }
                    } else {
                        ev_base
                    };

                    let mut draw_hline = |label: &str, y_world: f64| {
                        let y_lin = y_world + tr.offset;
                        let y_plot = if scope.y_axis.log_scale {
                            if y_lin > 0.0 {
                                y_lin.log10()
                            } else {
                                f64::NAN
                            }
                        } else {
                            y_lin
                        };
                        if y_plot.is_finite() {
                            let h = HLine::new(label, y_plot)
                                .color(thr_color)
                                .width(def.look.width * thr_expand_line)
                                .style(def.look.style);
                            plot_ui.hline(h);
                        }
                    };

                    let thr_info = def.get_info(&scope.y_axis);
                    let legend_label = if scope.show_info_in_legend {
                        format!("{} — {}", def.name, thr_info)
                    } else {
                        def.name.clone()
                    };

                    match def.kind {
                        ThresholdKind::GreaterThan { value } => {
                            draw_hline(&legend_label, value);
                        }
                        ThresholdKind::LessThan { value } => {
                            draw_hline(&legend_label, value);
                        }
                        ThresholdKind::InRange { low, high } => {
                            draw_hline(&legend_label, low);
                            draw_hline(&legend_label, high);
                        }
                    }

                    let state = def.get_runtime_state();

                    let marker_y_world = match def.kind {
                        ThresholdKind::GreaterThan { value } => value,
                        ThresholdKind::LessThan { value } => value,
                        ThresholdKind::InRange { low, high } => (low + high) * 0.5,
                    };
                    let y_lin = marker_y_world + tr.offset;
                    let marker_y_plot = if scope.y_axis.log_scale {
                        if y_lin > 0.0 {
                            y_lin.log10()
                        } else {
                            f64::NAN
                        }
                    } else {
                        y_lin
                    };
                    if marker_y_plot.is_finite() {
                        for ev in state.events.iter() {
                            if ev.end_t < xmin || ev.start_t > xmax {
                                continue;
                            }
                            if def.start_look.show_points {
                                let p = Points::new(
                                    legend_label.clone(),
                                    vec![[ev.start_t, marker_y_plot]],
                                )
                                .radius(def.start_look.point_size * thr_expand_points)
                                .shape(def.start_look.marker)
                                .color(ev_color);
                                plot_ui.points(p);
                            } else {
                                let s = VLine::new(legend_label.clone(), ev.start_t)
                                    .color(ev_color)
                                    .width(def.start_look.width * thr_expand_line)
                                    .style(def.start_look.style);

                                plot_ui.vline(s);
                            }
                            if def.stop_look.show_points {
                                let p = Points::new(
                                    legend_label.clone(),
                                    vec![[ev.end_t, marker_y_plot]],
                                )
                                .radius(def.stop_look.point_size * thr_expand_points)
                                .shape(def.stop_look.marker)
                                .color(ev_color);
                                plot_ui.points(p);
                            } else {
                                let e = VLine::new(legend_label.clone(), ev.end_t)
                                    .color(ev_color)
                                    .width(def.stop_look.width * thr_expand_line)
                                    .style(def.stop_look.style);

                                plot_ui.vline(e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        if data.pending_requests.clear_thresholds {
            self.clear_all();
            data.pending_requests.clear_thresholds = false;
        }

        let sources = data.get_all_drawn_points();

        for def in self.thresholds.values_mut() {
            def.process_threshold(sources.clone());
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
        ui.label("Detect and log when a trace exceeds a condition.");
        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        ui.separator();
        // Existing thresholds list: color edit (threshold color), name/info, and Remove right-aligned
        // Reset hover highlights for this frame
        self.hover_threshold = None;
        let names_snapshot: Vec<String> = self.thresholds.keys().cloned().collect();
        for name in names_snapshot {
            let mut action_remove = false;
            let mut action_clear = false;
            let row = ui.horizontal(|ui| {
                let def = self
                    .thresholds
                    .get_mut(&name)
                    .expect("threshold existed in snapshot");
                // Threshold line color editor (from per-threshold look)
                let mut col = def.look.color;
                let color_resp = ui
                    .color_edit_button_srgba(&mut col)
                    .on_hover_text("Change threshold color");
                if color_resp.hovered() {
                    self.hover_threshold = Some(def.name.clone());
                }
                if color_resp.changed() {
                    def.look.color = col;
                    def.start_look.color = col;
                    def.stop_look.color = col;
                }

                // Clickable name: opens editor; hover highlights target trace
                let name_resp = ui.add(
                    egui::Label::new(def.name.clone())
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if name_resp.hovered() {
                    self.hover_threshold = Some(def.name.clone());
                }
                if name_resp.clicked() {
                    self.builder = def.clone();
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Info text like math traces: target + condition; hover highlights target trace
                let default_axis = AxisSettings::default();
                let axis_setting = data
                    .scope_containing_trace(&def.target)
                    .map(|scope| &scope.y_axis)
                    .unwrap_or(&default_axis);
                let info_text = def.get_info(axis_setting);
                let info_resp = ui.add(
                    egui::Label::new(info_text)
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if info_resp.hovered() {
                    self.hover_threshold = Some(def.name.clone());
                }
                if info_resp.clicked() {
                    // Same as clicking the name: open editor
                    self.builder = def.clone();
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Right-aligned actions: Clear (events) and Remove (definition)
                let removing_name = def.name.clone();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let remove_resp = ui.button("🗑 Remove");
                    if remove_resp.hovered() {
                        self.hover_threshold = Some(removing_name.clone());
                    }
                    if remove_resp.clicked() {
                        action_remove = true;
                    }
                    let clear_resp = ui
                        .button("X Clear")
                        .on_hover_text("Clear events for this threshold");
                    if clear_resp.hovered() {
                        self.hover_threshold = Some(removing_name.clone());
                    }
                    if clear_resp.clicked() {
                        action_clear = true;
                    }
                });
            });
            if action_remove {
                let removing = name.clone();
                // Emit THRESHOLD_REMOVED event
                if let Some(ctrl) = &data.event_ctrl {
                    let mut evt =
                        crate::events::PlotEvent::new(crate::events::EventKind::THRESHOLD_REMOVED);
                    evt.threshold = Some(crate::events::ThresholdMeta {
                        threshold_name: removing.clone(),
                        trace: self.thresholds.get(&removing).map(|d| d.target.clone()),
                        start_t: None,
                        end_t: None,
                        duration: None,
                        area: None,
                    });
                    ctrl.emit_filtered(evt);
                }
                self.thresholds.remove(&removing);
                if self.editing.as_deref() == Some(&removing) {
                    self.editing = None;
                    self.creating = false;
                    self.builder = ThresholdDef::default();
                    self.error = None;
                }
            } else if action_clear {
                if let Some(def) = self.thresholds.get_mut(&name) {
                    def.clear_threshold_events();
                }
            }
            if row.response.hovered() {
                self.hover_threshold = Some(name.clone());
            }
            // Optional short summary of recent events below each row
            if let Some(st) = self.thresholds.get(&name) {
                let cnt = st.count_threshold_events();
                if let Some(last) = st.get_last_threshold_event() {
                    // Use the same time formatting as the events table
                    let start_fmt = {
                        let val = last.start_t;
                        let secs = val as i64;
                        let nsecs = ((val - secs as f64) * 1e9) as u32;
                        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

                        dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
                    };
                    let resp = ui.label(format!(
                        "Events: {} • last: {} • {} ms • area {}",
                        cnt,
                        start_fmt,
                        format!("{:.3}", last.duration * 1000.0),
                        format!("{:.4}", last.area)
                    ));
                    if resp.hovered() {
                        self.hover_threshold = Some(name.clone());
                    }
                } else {
                    let resp = ui.label("Events: 0");
                    if resp.hovered() {
                        self.hover_threshold = Some(name.clone());
                    }
                }
            }
        }

        // Full-width New button
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("➕ New"))
            .on_hover_text("Create a new threshold")
            .clicked();
        if new_clicked {
            self.builder = ThresholdDef::default();
            // Apply previous builder-style defaults for looks
            self.builder.look.style = LineStyle::Dashed { length: 6.0 };
            self.builder.start_look.show_points = true;
            self.builder.start_look.point_size = 6.0;
            self.builder.start_look.marker = MarkerShape::Diamond;
            self.builder.start_look.style = LineStyle::Dotted { spacing: 4.0 };
            self.builder.start_look.visible = true;
            self.builder.stop_look.show_points = true;
            self.builder.stop_look.point_size = 6.0;
            self.builder.stop_look.marker = MarkerShape::Square;
            self.builder.stop_look.style = LineStyle::Dotted { spacing: 4.0 };
            self.editing = None;
            self.error = None;
            self.creating = true;
        }

        // Settings panel (like math): shown when creating or editing
        let is_editing = self.editing.is_some();
        let is_creating = self.creating;
        if is_editing || is_creating {
            ui.add_space(12.0);
            ui.separator();
            if is_editing {
                ui.strong("Edit threshold");
            } else {
                ui.strong("New threshold");
            }

            ui.add_space(3.0);

            // Name, Trace, Condition
            // Duplicate name when creating, or when editing and changing to an existing different name
            let duplicate_name = self.thresholds.contains_key(&self.builder.name)
                && self.editing.as_deref() != Some(self.builder.name.as_str());

            ui.horizontal(|ui| {
                ui.label("Name");
                if duplicate_name {
                    egui::Frame::default()
                        .stroke(egui::Stroke::new(1.5, egui::Color32::RED))
                        .show(ui, |ui| {
                            let resp = ui.add(egui::TextEdit::singleline(&mut self.builder.name));
                            let _resp = resp.on_hover_text(
                                "A threshold with this name already exists. Please choose another.",
                            );
                        });
                } else {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.builder.name));
                    let _resp = resp.on_hover_text("Enter a unique name for this threshold");
                }
            });
            let trace_names = data.traces.all_trace_names();
            let mut target_idx = trace_names
                .iter()
                .position(|n| n == &self.builder.target)
                .unwrap_or(0);
            egui::ComboBox::from_label("Trace")
                .selected_text(
                    trace_names
                        .get(target_idx)
                        .map(|t| t.to_string())
                        .unwrap_or_default(),
                )
                .show_ui(ui, |ui| {
                    for (i, n) in trace_names.iter().enumerate() {
                        if ui.selectable_label(target_idx == i, n.as_str()).clicked() {
                            target_idx = i;
                        }
                    }
                });
            if let Some(sel_name) = trace_names.get(target_idx) {
                self.builder.target = sel_name.clone();
            }
            // Default color when creating: use selected trace color at 75% alpha if not set by user yet
            if is_creating {
                if let Some(tr) = data.traces.get_trace(&self.builder.target) {
                    if self.builder.look.color == egui::Color32::WHITE {
                        let c = tr.look.color;
                        self.builder.look.color =
                            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 191);
                    }
                }
            }
            let kinds = [">", "<", "in range"];
            let mut kind_idx: usize = match &self.builder.kind {
                ThresholdKind::GreaterThan { .. } => 0,
                ThresholdKind::LessThan { .. } => 1,
                ThresholdKind::InRange { .. } => 2,
            };
            egui::ComboBox::from_label("Condition")
                .selected_text(kinds[kind_idx])
                .show_ui(ui, |ui| {
                    for (i, k) in kinds.iter().enumerate() {
                        if ui.selectable_label(kind_idx == i, *k).clicked() {
                            kind_idx = i;
                        }
                    }
                });
            // Render and update threshold values according to selected kind
            match (&mut self.builder.kind, kind_idx) {
                (ThresholdKind::GreaterThan { value }, 0) => {
                    let mut v = *value;
                    ui.horizontal(|ui| {
                        ui.label("Value");
                        if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                            *value = v;
                        }
                    });
                }
                (ThresholdKind::LessThan { value }, 1) => {
                    let mut v = *value;
                    ui.horizontal(|ui| {
                        ui.label("Value");
                        if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                            *value = v;
                        }
                    });
                }
                (ThresholdKind::InRange { low, high }, 2) => {
                    let mut lo = *low;
                    let mut hi = *high;
                    ui.horizontal(|ui| {
                        ui.label("Low");
                        ui.add(egui::DragValue::new(&mut lo).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("High");
                        ui.add(egui::DragValue::new(&mut hi).speed(0.01));
                    });
                    if lo != *low || hi != *high {
                        *low = lo.min(hi);
                        *high = lo.max(hi);
                    }
                }
                // Variant switch requested
                (old_kind, new_idx) => {
                    let (v1, v2) = match old_kind {
                        ThresholdKind::GreaterThan { value } => (*value, *value),
                        ThresholdKind::LessThan { value } => (*value, *value),
                        ThresholdKind::InRange { low, high } => (*low, *high),
                    };
                    self.builder.kind = match new_idx {
                        0 => ThresholdKind::GreaterThan { value: v1 },
                        1 => ThresholdKind::LessThan { value: v1 },
                        _ => ThresholdKind::InRange {
                            low: v1.min(v2),
                            high: v1.max(v2),
                        },
                    };
                    // Render fields for new variant
                    match &mut self.builder.kind {
                        ThresholdKind::GreaterThan { value }
                        | ThresholdKind::LessThan { value } => {
                            let mut v = *value;
                            ui.horizontal(|ui| {
                                ui.label("Value");
                                if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                                    *value = v;
                                }
                            });
                        }
                        ThresholdKind::InRange { low, high } => {
                            let mut lo = *low;
                            let mut hi = *high;
                            ui.horizontal(|ui| {
                                ui.label("Low");
                                ui.add(egui::DragValue::new(&mut lo).speed(0.01));
                            });
                            ui.horizontal(|ui| {
                                ui.label("High");
                                ui.add(egui::DragValue::new(&mut hi).speed(0.01));
                            });
                            if lo != *low || hi != *high {
                                *low = lo.min(hi);
                                *high = lo.max(hi);
                            }
                        }
                    }
                }
            }
            ui.horizontal(|ui| {
                ui.label("Min duration (ms)");
                let mut ms = self.builder.min_duration_s * 1000.0;
                if ui.add(egui::DragValue::new(&mut ms).speed(0.1)).changed() {
                    self.builder.min_duration_s = (ms / 1000.0).max(0.0);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Max events");
                ui.add(egui::DragValue::new(&mut self.builder.max_events).speed(1));
            });

            // Collapsible style editors (moved here, just before Save/Add)
            ui.add_space(5.0);
            egui::CollapsingHeader::new("Style: Threshold line")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut self.builder.look, ui, false);
                });
            // Keep event colors locked to the line color
            self.builder.start_look.color = self.builder.look.color;
            self.builder.stop_look.color = self.builder.look.color;
            egui::CollapsingHeader::new("Style: Event start")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut self.builder.start_look, ui, true);
                });
            egui::CollapsingHeader::new("Style: Event stop")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut self.builder.stop_look, ui, true);
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add threshold" };
                let can_save = !self.builder.name.is_empty() && !duplicate_name;
                let mut save_clicked = false;
                if ui
                    .add_enabled(can_save, egui::Button::new(save_label))
                    .clicked()
                {
                    save_clicked = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖ Cancel").clicked() {
                        self.editing = None;
                        self.creating = false;
                        self.builder = ThresholdDef::default();
                        self.error = None;
                    }
                });
                if save_clicked {
                    if !self.builder.name.is_empty() {
                        if is_editing {
                            // Insert/replace edited definition; remove old key when renaming
                            let old_key = self.editing.clone();
                            self.thresholds
                                .insert(self.builder.name.clone(), self.builder.clone());
                            if let Some(old) = old_key {
                                if old != self.builder.name {
                                    self.thresholds.remove(&old);
                                }
                            }
                            self.editing = None;
                            self.creating = false;
                            self.builder = ThresholdDef::default();
                            self.error = None;
                        } else {
                            if self
                                .thresholds
                                .iter()
                                .any(|(_name, d)| d.name == self.builder.name)
                            {
                                self.error =
                                    Some("A threshold with this name already exists".into());
                            } else {
                                self.thresholds
                                    .insert(self.builder.name.clone(), self.builder.clone());
                                self.creating = false;
                                self.builder = ThresholdDef::default();
                                self.error = None;
                            }
                        }
                    }
                }
            });
        }

        ui.separator();
        ui.heading("Threshold events");

        // Build list of names for filter first, without borrowing events
        ui.horizontal(|ui| {
            ui.label("Filter:");
            // Build list of names from current thresholds and from the log
            let mut names: Vec<String> = self
                .thresholds
                .iter()
                .map(|(_name, d)| d.name.clone())
                .collect();

            names.sort();
            names.dedup();
            let mut sel = self.events_filter.clone();
            egui::ComboBox::from_id_salt("thr_events_filter")
                .selected_text(match &sel {
                    Some(s) => format!("{}", s),
                    None => "All".to_string(),
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(sel.is_none(), "All").clicked() {
                        sel = None;
                    }
                    for n in &names {
                        if ui.selectable_label(sel.as_ref() == Some(n), n).clicked() {
                            sel = Some(n.clone());
                        }
                    }
                });
            if sel != self.events_filter {
                self.events_filter = sel;
            }
            if ui.button("📄 Export to CSV").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("threshold_events.csv")
                    .add_filter("CSV", &["csv"])
                    .save_file()
                {
                    if let Err(e) = self.save_threshold_events_csv(&path) {
                        eprintln!("Failed to export events CSV: {e}");
                    }
                }
            }
            if ui
                .button("X Clear events")
                .on_hover_text("Delete all threshold events (global log and per-threshold buffers)")
                .clicked()
            {
                for def in self.thresholds.values_mut() {
                    def.clear_threshold_events();
                }
            }
        });
        // Build filtered, newest-first slice indices for table, after filter selection possibly changed
        let mut filtered: Vec<ThresholdEvent> = self
            .thresholds
            .values()
            .flat_map(|t| t.get_threshold_events().into_iter())
            .filter(|e| {
                self.events_filter
                    .as_ref()
                    .map_or(true, |f| &e.threshold == f)
            })
            .collect();
        // Sort by start time descending (latest first)
        filtered.sort_by(|a, b| match b.start_t.partial_cmp(&a.start_t) {
            Some(ord) => ord,
            None => Ordering::Equal,
        });

        // Delegate for rendering with egui_table
        struct EventsDelegate<'a> {
            items: &'a [&'a ThresholdEvent],
            hover_threshold_out: &'a mut Option<String>,
            axis: &'a AxisSettings,
        }
        impl<'a> TableDelegate for EventsDelegate<'a> {
            fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
                let col = cell.col_range.start;
                let text = match col {
                    0 => "Threshold",
                    1 => "Start time",
                    2 => "End time",
                    3 => "Duration (ms)",
                    4 => "Trace",
                    5 => "Area",
                    _ => "",
                };
                ui.add_space(4.0);
                ui.strong(text);
            }
            fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
                let row = cell.row_nr as usize;
                let col = cell.col_nr;
                if let Some(e) = self.items.get(row).copied() {
                    ui.add_space(4.0);
                    match col {
                        0 => {
                            let resp = ui.add(
                                egui::Label::new(&e.threshold)
                                    .truncate()
                                    .show_tooltip_when_elided(true)
                                    .sense(egui::Sense::hover()),
                            );
                            if resp.hovered() {
                                *self.hover_threshold_out = Some(e.threshold.clone());
                            }
                        }
                        1 => {
                            ui.label(self.axis.format_value(e.start_t, 3, e.duration));
                        }
                        2 => {
                            ui.label(self.axis.format_value(e.end_t, 3, e.duration));
                        }
                        3 => {
                            ui.label(self.axis.format_value(e.duration * 1000.0, 3, e.duration));
                        }
                        4 => {
                            ui.label(&e.trace.0);
                        }
                        5 => {
                            ui.label(format!("{:.6}", e.area));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Build items slice with a longer-lived binding to avoid temporary drop issues
        let items_vec: Vec<&ThresholdEvent> = filtered.iter().collect();

        let default_axis = AxisSettings::new_time_axis();
        let axis_setting = data
            .primary_scope()
            .map(|scope| &scope.x_axis)
            .unwrap_or(&default_axis);

        let mut delegate = EventsDelegate {
            items: items_vec.as_slice(),
            hover_threshold_out: &mut self.hover_threshold,
            axis: axis_setting,
        };
        let cols = vec![
            egui_table::Column::new(160.0),
            egui_table::Column::new(180.0),
            egui_table::Column::new(180.0),
            egui_table::Column::new(140.0),
            egui_table::Column::new(140.0),
            egui_table::Column::new(120.0),
        ];
        let avail_w = ui.available_width();
        // Expand table to the bottom of the panel
        let remaining_h = ui.available_height();
        let (rect, _resp) =
            ui.allocate_exact_size(egui::vec2(avail_w, remaining_h), egui::Sense::hover());
        let ui_builder = egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min));
        let mut table_ui = ui.new_child(ui_builder);
        Table::new()
            .id_salt("thr_events_table")
            .num_rows(filtered.len() as u64)
            .columns(cols)
            .headers(vec![EgHeaderRow::new(24.0)])
            .show(&mut table_ui, &mut delegate);
    }
}

impl ThresholdsPanel {
    pub const SHOW_THRESHOLDS_LABEL: &'static str = "👁 Show Thresholds";
    pub const NEW_LABEL: &'static str = "⊞ New";

    pub fn save_threshold_events_csv(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        writeln!(
            f,
            "end_time_seconds,threshold,trace,start_time_seconds,duration_seconds,area"
        )?;

        let events: Vec<ThresholdEvent> = self
            .thresholds
            .values()
            .flat_map(|t| t.get_threshold_events().into_iter())
            .filter(|e| {
                self.events_filter
                    .as_ref()
                    .map_or(true, |f| &e.threshold == f)
            })
            .collect();

        for e in events {
            writeln!(
                f,
                "{:.9},{},{},{:.9},{:.9},{:.9}",
                e.end_t, e.threshold, e.trace.0, e.start_t, e.duration, e.area
            )?;
        }

        Ok(())
    }

    pub fn clear_all_events(&mut self) {
        for def in self.thresholds.values_mut() {
            def.clear_threshold_events();
        }
    }
}
// Removed unused show_thresholds_dialog helper; dialogs are shown via DockPanel::show_detached_dialog

// tests moved to `tests/thresholds_ui.rs`
