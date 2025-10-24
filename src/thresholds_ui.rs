use super::panel::{DockPanel, DockState};
use egui;
use egui::Color32;
use std::collections::HashMap;
// use egui_plot::{LineStyle, MarkerShape};
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};

use crate::thresholds::{ThresholdDef, ThresholdEvent, ThresholdKind};

use super::app::LivePlotApp;
use super::types::ThresholdBuilderState;

#[derive(Debug, Clone)]
pub struct ThresholdsPanel {
    pub dock: DockState,
    pub builder: super::types::ThresholdBuilderState,
    pub editing: Option<String>,
    pub error: Option<String>,
    pub creating: bool,
    pub looks: HashMap<String, super::trace_look::TraceLook>,
    pub start_looks: HashMap<String, super::trace_look::TraceLook>,
    pub stop_looks: HashMap<String, super::trace_look::TraceLook>,
    pub events_filter: Option<String>,
}

impl Default for ThresholdsPanel {
    fn default() -> Self {
        Self {
            dock: DockState::new("⚠️ Thresholds"),
            builder: super::types::ThresholdBuilderState::default(),
            editing: None,
            error: None,
            creating: false,
            looks: HashMap::new(),
            start_looks: HashMap::new(),
            stop_looks: HashMap::new(),
            events_filter: None,
        }
    }
}

impl DockPanel for ThresholdsPanel {
    fn dock_mut(&mut self) -> &mut DockState {
        &mut self.dock
    }
    fn panel_contents(&mut self, app: &mut LivePlotApp, ui: &mut egui::Ui) {
        // (no-op)
        ui.label("Detect and log when a trace exceeds a condition.");
        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        ui.separator();
        // Existing thresholds list: color edit (threshold color), name/info, and Remove right-aligned
        // Reset hover highlights for this frame
        app.hover_trace = None;
        app.hover_threshold = None;
        for def in app.threshold_defs.clone().iter() {
            let row = ui.horizontal(|ui| {
                // Threshold line color editor (from per-threshold look)
                let mut line_look = self.looks.get(&def.name).cloned().unwrap_or_default();
                let mut col = line_look.color;
                let color_resp = ui
                    .color_edit_button_srgba(&mut col)
                    .on_hover_text("Change threshold color");
                if color_resp.hovered() {
                    app.hover_threshold = Some(def.name.clone());
                }
                if color_resp.changed() {
                    line_look.color = col;
                    self.looks.insert(def.name.clone(), line_look);
                    // Keep event colors in sync with the line color
                    if let Some(le) = self.start_looks.get_mut(&def.name) {
                        le.color = col;
                    }
                    if let Some(le) = self.stop_looks.get_mut(&def.name) {
                        le.color = col;
                    }
                }

                // Clickable name: opens editor; hover highlights target trace
                let name_resp = ui.add(
                    egui::Label::new(def.name.clone())
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if name_resp.hovered() {
                    app.hover_threshold = Some(def.name.clone());
                }
                if name_resp.clicked() {
                    self.builder = ThresholdBuilderState::default();
                    self.builder.name = def.name.clone();
                    self.builder.target_idx = app
                        .trace_order
                        .iter()
                        .position(|n| n == &def.target.0)
                        .unwrap_or(0);
                    match &def.kind {
                        ThresholdKind::GreaterThan { value } => {
                            self.builder.kind_idx = 0;
                            self.builder.thr1 = *value;
                        }
                        ThresholdKind::LessThan { value } => {
                            self.builder.kind_idx = 1;
                            self.builder.thr1 = *value;
                        }
                        ThresholdKind::InRange { low, high } => {
                            self.builder.kind_idx = 2;
                            self.builder.thr1 = *low;
                            self.builder.thr2 = *high;
                        }
                    }
                    self.builder.min_duration_ms = def.min_duration_s * 1000.0;
                    self.builder.max_events = def.max_events;
                    // Pre-fill looks from stored per-threshold styles
                    if let Some(l) = self.looks.get(&def.name) {
                        self.builder.look = l.clone();
                    }
                    if let Some(l) = self.start_looks.get(&def.name) {
                        self.builder.look_start_events = l.clone();
                    }
                    if let Some(l) = self.stop_looks.get(&def.name) {
                        self.builder.look_stop_events = l.clone();
                    }
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Info text like math traces: target + condition; hover highlights target trace
                let info_text = match &def.kind {
                    ThresholdKind::GreaterThan { value } => {
                        format!("{} > {:.3}", def.target.0, value)
                    }
                    ThresholdKind::LessThan { value } => format!("{} < {:.3}", def.target.0, value),
                    ThresholdKind::InRange { low, high } => {
                        format!("{} in [{:.3}, {:.3}]", def.target.0, low, high)
                    }
                };
                let info_resp = ui.add(
                    egui::Label::new(info_text)
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if info_resp.hovered() {
                    app.hover_threshold = Some(def.name.clone());
                }
                if info_resp.clicked() {
                    // Same as clicking the name: open editor
                    self.builder = ThresholdBuilderState::default();
                    self.builder.name = def.name.clone();
                    self.builder.target_idx = app
                        .trace_order
                        .iter()
                        .position(|n| n == &def.target.0)
                        .unwrap_or(0);
                    match &def.kind {
                        ThresholdKind::GreaterThan { value } => {
                            self.builder.kind_idx = 0;
                            self.builder.thr1 = *value;
                        }
                        ThresholdKind::LessThan { value } => {
                            self.builder.kind_idx = 1;
                            self.builder.thr1 = *value;
                        }
                        ThresholdKind::InRange { low, high } => {
                            self.builder.kind_idx = 2;
                            self.builder.thr1 = *low;
                            self.builder.thr2 = *high;
                        }
                    }
                    self.builder.min_duration_ms = def.min_duration_s * 1000.0;
                    self.builder.max_events = def.max_events;
                    if let Some(l) = self.looks.get(&def.name) {
                        self.builder.look = l.clone();
                    }
                    if let Some(l) = self.start_looks.get(&def.name) {
                        self.builder.look_start_events = l.clone();
                    }
                    if let Some(l) = self.stop_looks.get(&def.name) {
                        self.builder.look_stop_events = l.clone();
                    }
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Right-aligned actions: Clear (events) and Remove (definition)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let remove_resp = ui.button("Remove");
                    if remove_resp.hovered() {
                        app.hover_threshold = Some(def.name.clone());
                    }
                    if remove_resp.clicked() {
                        let removing = def.name.clone();
                        app.remove_threshold_internal(&removing);
                        if self.editing.as_deref() == Some(&removing) {
                            self.editing = None;
                            self.creating = false;
                            self.builder = ThresholdBuilderState::default();
                            self.error = None;
                        }
                    }
                    let clear_resp = ui
                        .button("Clear")
                        .on_hover_text("Clear events for this threshold");
                    if clear_resp.hovered() {
                        app.hover_threshold = Some(def.name.clone());
                    }
                    if clear_resp.clicked() {
                        app.clear_threshold_events(&def.name);
                    }
                });
            });
            if row.response.hovered() {
                app.hover_threshold = Some(def.name.clone());
            }
            // Optional short summary of recent events below each row
            if let Some(st) = app.threshold_states.get(&def.name) {
                let cnt = st.events.len();
                if let Some(last) = st.events.back() {
                    // Use the same time formatting as the events table
                    let start_fmt = app.x_date_format.format_value(last.start_t);
                    let resp = ui.label(format!(
                        "Events: {} • last: {} • {} ms • area {}",
                        cnt,
                        start_fmt,
                        format!("{:.3}", last.duration * 1000.0),
                        format!("{:.4}", last.area)
                    ));
                    if resp.hovered() {
                        app.hover_threshold = Some(def.name.clone());
                    }
                } else {
                    let resp = ui.label("Events: 0");
                    if resp.hovered() {
                        app.hover_threshold = Some(def.name.clone());
                    }
                }
            }
        }

        // Full-width New button
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("New"))
            .on_hover_text("Create a new threshold")
            .clicked();
        if new_clicked {
            self.builder = ThresholdBuilderState::default();
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
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.builder.name);
            });
            let trace_names: Vec<String> = app.trace_order.clone();
            egui::ComboBox::from_label("Trace")
                .selected_text(
                    trace_names
                        .get(self.builder.target_idx)
                        .cloned()
                        .unwrap_or_default(),
                )
                .show_ui(ui, |ui| {
                    for (i, n) in trace_names.iter().enumerate() {
                        ui.selectable_value(&mut self.builder.target_idx, i, n);
                    }
                });
            // Default color when creating: use selected trace color at 75% alpha if not set by user yet
            if is_creating {
                if let Some(sel_name) = trace_names.get(self.builder.target_idx) {
                    if let Some(tr) = app.traces.get(sel_name) {
                        if self.builder.look.color == egui::Color32::WHITE {
                            let c = tr.look.color;
                            self.builder.look.color =
                                egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 191);
                        }
                    }
                }
            }
            let kinds = [">", "<", "in range"];
            egui::ComboBox::from_label("Condition")
                .selected_text(kinds[self.builder.kind_idx])
                .show_ui(ui, |ui| {
                    for (i, k) in kinds.iter().enumerate() {
                        ui.selectable_value(&mut self.builder.kind_idx, i, *k);
                    }
                });
            match self.builder.kind_idx {
                0 | 1 => {
                    ui.horizontal(|ui| {
                        ui.label("Value");
                        ui.add(egui::DragValue::new(&mut self.builder.thr1).speed(0.01));
                    });
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.label("Low");
                        ui.add(egui::DragValue::new(&mut self.builder.thr1).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("High");
                        ui.add(egui::DragValue::new(&mut self.builder.thr2).speed(0.01));
                    });
                }
            }
            ui.horizontal(|ui| {
                ui.label("Min duration (ms)");
                ui.add(egui::DragValue::new(&mut self.builder.min_duration_ms).speed(0.1));
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
                    self.builder
                        .look
                        .render_editor(ui, false, None, false, None);
                });
            // Keep event colors locked to the line color
            self.builder.look_start_events.color = self.builder.look.color;
            self.builder.look_stop_events.color = self.builder.look.color;
            egui::CollapsingHeader::new("Style: Event start")
                .default_open(false)
                .show(ui, |ui| {
                    self.builder.look_start_events.render_editor(
                        ui,
                        true,
                        None,
                        true,
                        Some(self.builder.look.color),
                    );
                });
            egui::CollapsingHeader::new("Style: Event stop")
                .default_open(false)
                .show(ui, |ui| {
                    self.builder.look_stop_events.render_editor(
                        ui,
                        true,
                        None,
                        true,
                        Some(self.builder.look.color),
                    );
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add threshold" };
                let mut save_clicked = false;
                if ui.button(save_label).clicked() {
                    save_clicked = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.editing = None;
                        self.creating = false;
                        self.builder = ThresholdBuilderState::default();
                        self.error = None;
                    }
                });
                if save_clicked {
                    if let Some(nm) = trace_names.get(self.builder.target_idx) {
                        if !self.builder.name.is_empty() {
                            let kind = match self.builder.kind_idx {
                                0 => ThresholdKind::GreaterThan {
                                    value: self.builder.thr1,
                                },
                                1 => ThresholdKind::LessThan {
                                    value: self.builder.thr1,
                                },
                                _ => ThresholdKind::InRange {
                                    low: self.builder.thr1.min(self.builder.thr2),
                                    high: self.builder.thr1.max(self.builder.thr2),
                                },
                            };
                            let def = ThresholdDef {
                                name: self.builder.name.clone(),
                                display_name: None,
                                target: crate::math::TraceRef(nm.clone()),
                                kind,
                                color_hint: Some([
                                    self.builder.look.color.r(),
                                    self.builder.look.color.g(),
                                    self.builder.look.color.b(),
                                ]),
                                min_duration_s: (self.builder.min_duration_ms / 1000.0).max(0.0),
                                max_events: self.builder.max_events,
                            };
                            if is_editing {
                                let orig = self.editing.clone().unwrap();
                                app.remove_threshold_internal(&orig);
                                app.add_threshold_internal(def.clone());
                                self.looks
                                    .insert(def.name.clone(), self.builder.look.clone());
                                // Save start/stop looks (colors are already synced to line color)
                                self.start_looks.insert(
                                    def.name.clone(),
                                    self.builder.look_start_events.clone(),
                                );
                                self.stop_looks.insert(
                                    def.name.clone(),
                                    self.builder.look_stop_events.clone(),
                                );
                                self.editing = None;
                                self.creating = false;
                                self.builder = ThresholdBuilderState::default();
                                self.error = None;
                            } else {
                                if app.threshold_defs.iter().any(|d| d.name == def.name) {
                                    self.error =
                                        Some("A threshold with this name already exists".into());
                                } else {
                                    app.add_threshold_internal(def.clone());
                                    self.looks
                                        .insert(def.name.clone(), self.builder.look.clone());
                                    self.start_looks.insert(
                                        def.name.clone(),
                                        self.builder.look_start_events.clone(),
                                    );
                                    self.stop_looks.insert(
                                        def.name.clone(),
                                        self.builder.look_stop_events.clone(),
                                    );
                                    self.creating = false;
                                    self.builder = ThresholdBuilderState::default();
                                    self.error = None;
                                }
                            }
                        }
                    }
                }
            });
        }

        ui.separator();
        ui.heading("Threshold events");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            // Build list of names from current thresholds and from the log
            let mut names: Vec<String> =
                app.threshold_defs.iter().map(|d| d.name.clone()).collect();
            for e in app.threshold_event_log.iter() {
                if !names.iter().any(|n| n == &e.threshold) {
                    names.push(e.threshold.clone());
                }
            }
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
            if ui.button("Export to CSV").clicked() {
                // Collect filtered events (newest first as shown)
                let evts: Vec<&ThresholdEvent> = app
                    .threshold_event_log
                    .iter()
                    .rev()
                    .filter(|e| {
                        self.events_filter
                            .as_ref()
                            .map_or(true, |f| &e.threshold == f)
                    })
                    .collect();
                if !evts.is_empty() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("threshold_events.csv")
                        .add_filter("CSV", &["csv"])
                        .save_file()
                    {
                        if let Err(e) =
                            super::export_helpers::save_threshold_events_csv(&path, &evts)
                        {
                            eprintln!("Failed to export events CSV: {e}");
                        }
                    }
                }
            }
            if ui
                .button("Clear events")
                .on_hover_text("Delete all threshold events (global log and per-threshold buffers)")
                .clicked()
            {
                app.clear_all_threshold_events();
            }
        });
        // Build filtered, newest-first slice indices for table
        let filtered: Vec<&ThresholdEvent> = app
            .threshold_event_log
            .iter()
            .rev()
            .filter(|e| {
                self.events_filter
                    .as_ref()
                    .map_or(true, |f| &e.threshold == f)
            })
            .collect();

        // Delegate for rendering with egui_table
        struct EventsDelegate<'a> {
            items: &'a [&'a ThresholdEvent],
            fmt: crate::config::XDateFormat,
            to_clear: Vec<ThresholdEvent>,
            hover_threshold_out: &'a mut Option<String>,
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
                    6 => "",
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
                            ui.label(self.fmt.format_value(e.start_t));
                        }
                        2 => {
                            ui.label(self.fmt.format_value(e.end_t));
                        }
                        3 => {
                            ui.label(format!("{:.3}", e.duration * 1000.0));
                        }
                        4 => {
                            ui.label(&e.trace);
                        }
                        5 => {
                            ui.label(format!("{:.6}", e.area));
                        }
                        6 => {
                            let ev_clear = ui
                                .small_button("Clear")
                                .on_hover_text("Remove this event from the list");
                            if ev_clear.hovered() {
                                *self.hover_threshold_out = Some(e.threshold.clone());
                            }
                            if ev_clear.clicked() {
                                self.to_clear.push(e.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut delegate = EventsDelegate {
            items: &filtered,
            fmt: app.x_date_format,
            to_clear: Vec::new(),
            hover_threshold_out: &mut app.hover_threshold,
        };
        let cols = vec![
            egui_table::Column::new(152.0),
            egui_table::Column::new(172.0),
            egui_table::Column::new(172.0),
            egui_table::Column::new(132.0),
            egui_table::Column::new(132.0),
            egui_table::Column::new(112.0),
            egui_table::Column::new(72.0),
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
        // Apply row clears after rendering
        if !delegate.to_clear.is_empty() {
            for ev in delegate.to_clear {
                app.remove_threshold_event(&ev);
            }
        }
    }
}
// Removed unused show_thresholds_dialog helper; dialogs are shown via DockPanel::show_detached_dialog
