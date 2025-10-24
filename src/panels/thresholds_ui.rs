use super::panel_trait::{Panel, PanelState};
use crate::data::scope::ScopeData;
use crate::data::thresholds::{ThresholdDef, ThresholdEvent, ThresholdKind};
use crate::data::trace_look::TraceLook;
use crate::panels::trace_look_ui::render_trace_look_editor;
use chrono::Local;
use egui;
use egui::{Color32, Ui};
use egui_plot::{LineStyle, MarkerShape};
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ThresholdBuilderState {
    pub name: String,
    pub target_idx: usize,
    pub kind_idx: usize, // 0: >, 1: <, 2: in range
    pub thr1: f64,
    pub thr2: f64,
    pub min_duration_ms: f64,
    pub max_events: usize,
    pub look: TraceLook,
    pub look_start_events: TraceLook,
    pub look_stop_events: TraceLook,
}

impl Default for ThresholdBuilderState {
    fn default() -> Self {
        let mut look = TraceLook::default();
        look.style = LineStyle::Dashed { length: 6.0 };
        let mut look_start = TraceLook::default();
        look_start.show_points = true;
        look_start.point_size = 6.0;
        look_start.marker = MarkerShape::Diamond;
        // Hide line by default for start/stop looks; rely on points
        look_start.visible = true; // keep visible, but the renderer will use points setting
        let mut look_stop = TraceLook::default();
        look_stop.show_points = true;
        look_stop.point_size = 6.0;
        look_stop.marker = MarkerShape::Square;
        Self {
            name: String::new(),
            target_idx: 0,
            kind_idx: 0,
            thr1: 0.0,
            thr2: 1.0,
            min_duration_ms: 2.0,
            max_events: 100,
            look,
            look_start_events: look_start,
            look_stop_events: look_stop,
        }
    }
}

impl ThresholdBuilderState {
    fn new_from_def(def: &ThresholdDef, data: &ScopeData) -> Self {
        let target_idx = data
            .trace_order
            .iter()
            .position(|n| n == &def.target.0)
            .unwrap_or(0);
        let (kind_idx, thr1, thr2) = match &def.kind {
            ThresholdKind::GreaterThan { value } => (0, *value, 0.0),
            ThresholdKind::LessThan { value } => (1, *value, 0.0),
            ThresholdKind::InRange { low, high } => (2, *low, *high),
        };
        let look = def.look.clone();
        let look_start = def.start_look.clone();
        let look_stop = def.stop_look.clone();
        Self {
            name: def.name.clone(),
            target_idx,
            kind_idx,
            thr1,
            thr2,
            min_duration_ms: def.min_duration_s * 1000.0,
            max_events: def.max_events,
            look,
            look_start_events: look_start,
            look_stop_events: look_stop,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThresholdsPanel {
    pub state: PanelState,
    builder: ThresholdBuilderState,
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
            state: PanelState::new("⚠ Thresholds"),
            builder: ThresholdBuilderState::default(),
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

    fn update_data(&mut self, _data: &mut ScopeData) {
        // (no-op)
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut ScopeData) {
        ui.label("Detect and log when a trace exceeds a condition.");
        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        ui.separator();
        // Existing thresholds list: color edit (threshold color), name/info, and Remove right-aligned
        // Reset hover highlights for this frame
        self.hover_threshold = None;
        for (name, def) in self.thresholds.clone().iter() {
            let row = ui.horizontal(|ui| {
                // Threshold line color editor (from per-threshold look)
                let mut line_look = def.look.clone();
                let mut col = line_look.color;
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
                    self.builder = ThresholdBuilderState::new_from_def(&def, data);
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
                    self.hover_threshold = Some(def.name.clone());
                }
                if info_resp.clicked() {
                    // Same as clicking the name: open editor
                    self.builder = ThresholdBuilderState::new_from_def(&def, data);
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Right-aligned actions: Clear (events) and Remove (definition)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let remove_resp = ui.button("Remove");
                    if remove_resp.hovered() {
                        self.hover_threshold = Some(def.name.clone());
                    }
                    if remove_resp.clicked() {
                        let removing = def.name.clone();
                        self.thresholds.remove(&removing);
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
                        self.hover_threshold = Some(def.name.clone());
                    }
                    if clear_resp.clicked() {
                        def.clear_threshold_events();
                    }
                });
            });
            if row.response.hovered() {
                self.hover_threshold = Some(def.name.clone());
            }
            // Optional short summary of recent events below each row
            if let Some(st) = self.thresholds.get(&def.name) {
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
                        self.hover_threshold = Some(def.name.clone());
                    }
                } else {
                    let resp = ui.label("Events: 0");
                    if resp.hovered() {
                        self.hover_threshold = Some(def.name.clone());
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
            let trace_names: Vec<String> = data.trace_order.clone();
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
                    if let Some(tr) = data.traces.get(sel_name) {
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
                    render_trace_look_editor(&mut self.builder.look, ui, false);
                    // self.builder
                    //     .look
                    //     .render_editor(ui, false, None, false, None);
                });
            // Keep event colors locked to the line color
            self.builder.look_start_events.color = self.builder.look.color;
            self.builder.look_stop_events.color = self.builder.look.color;
            egui::CollapsingHeader::new("Style: Event start")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut self.builder.look_start_events, ui, true);
                    // self.builder.look_start_events.render_editor(
                    //     ui,
                    //     true,
                    //     None,
                    //     true,
                    //     Some(self.builder.look.color),
                    // );
                });
            egui::CollapsingHeader::new("Style: Event stop")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut self.builder.look_stop_events, ui, true);
                    // self.builder.look_stop_events.render_editor(
                    //     ui,
                    //     true,
                    //     None,
                    //     true,
                    //     Some(self.builder.look.color),
                    // );
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
                            let mut def = ThresholdDef::default();
                            def.name = self.builder.name.clone();
                            def.target = crate::data::thresholds::TraceRef(nm.clone());
                            def.kind = kind;
                            def.look = self.builder.look.clone();
                            def.start_look = self.builder.look_start_events.clone();
                            def.stop_look = self.builder.look_stop_events.clone();
                            def.min_duration_s = (self.builder.min_duration_ms / 1000.0).max(0.0);
                            def.max_events = self.builder.max_events;

                            if is_editing {
                                let orig = self.editing.clone().unwrap();
                                self.thresholds.insert(def.name.clone(), def);
                                self.editing = None;
                                self.creating = false;
                                self.builder = ThresholdBuilderState::default();
                                self.error = None;
                            } else {
                                if self.thresholds.iter().any(|(name, d)| d.name == def.name) {
                                    self.error =
                                        Some("A threshold with this name already exists".into());
                                } else {
                                    self.thresholds.insert(def.name.clone(), def);

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

        let threshold_event_log = self
            .thresholds
            .values()
            .flat_map(|t| t.get_last_threshold_event().into_iter());

        ui.horizontal(|ui| {
            ui.label("Filter:");
            // Build list of names from current thresholds and from the log
            let mut names: Vec<String> = self
                .thresholds
                .iter()
                .map(|(name, d)| d.name.clone())
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
            if ui.button("Export to CSV").clicked() {
                // Collect filtered events (newest first as shown)
                let evts: Vec<ThresholdEvent> = threshold_event_log
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
                for def in self.thresholds.values() {
                    def.clear_threshold_events();
                }
            }
        });
        // Build filtered, newest-first slice indices for table
        let filtered: Vec<ThresholdEvent> = threshold_event_log
            .filter(|e| {
                self.events_filter
                    .as_ref()
                    .map_or(true, |f| &e.threshold == f)
            })
            .collect();

        // Delegate for rendering with egui_table
        struct EventsDelegate<'a> {
            items: &'a [&'a ThresholdEvent],
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
                            ui.label(self.fmt.format_value(e.start_t)); // formatted time
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
            items: filtered.iter().collect::<Vec<&ThresholdEvent>>().as_slice(),
            to_clear: Vec::new(),
            hover_threshold_out: &mut self.hover_threshold,
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
