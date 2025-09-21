use eframe::egui;
use egui::Color32;
use egui_table::{Table, TableDelegate, HeaderRow as EgHeaderRow};

use crate::thresholds::{ThresholdDef, ThresholdKind, ThresholdEvent};

use super::app::ScopeAppMulti;
use super::types::ThresholdBuilderState;

pub(super) fn show_thresholds_dialog(app: &mut ScopeAppMulti, ctx: &egui::Context) {
    let mut show_flag = app.show_thresholds_dialog;
    egui::Window::new("Thresholds").open(&mut show_flag).show(ctx, |ui| {
        ui.label("Detect and log when a trace exceeds a condition.");
        if let Some(err) = &app.thr_error { ui.colored_label(Color32::LIGHT_RED, err); }
        ui.separator();
        // List existing thresholds
        for def in app.threshold_defs.clone().iter() {
            ui.horizontal(|ui| {
                ui.label(format!("{} on {}: {:?}, min_dur={:.3} ms, cap={} events", def.name, def.target.0, def.kind, def.min_duration_s*1000.0, def.max_events));
                if ui.button("Edit").clicked() {
                    app.thr_builder = ThresholdBuilderState::default();
                    app.thr_builder.name = def.name.clone();
                    app.thr_builder.target_idx = app.trace_order.iter().position(|n| n == &def.target.0).unwrap_or(0);
                    match &def.kind {
                        ThresholdKind::GreaterThan { value } => { app.thr_builder.kind_idx = 0; app.thr_builder.thr1 = *value; },
                        ThresholdKind::LessThan { value } => { app.thr_builder.kind_idx = 1; app.thr_builder.thr1 = *value; },
                        ThresholdKind::InRange { low, high } => { app.thr_builder.kind_idx = 2; app.thr_builder.thr1 = *low; app.thr_builder.thr2 = *high; },
                    }
                    app.thr_builder.min_duration_ms = def.min_duration_s * 1000.0;
                    app.thr_builder.max_events = def.max_events;
                    app.thr_editing = Some(def.name.clone());
                }
                if ui.button("Remove").clicked() { app.remove_threshold_internal(&def.name); }
            });
            // Show a short summary of recent events
            if let Some(st) = app.threshold_states.get(&def.name) {
                let cnt = st.events.len();
                if let Some(last) = st.events.back() {
                    ui.label(format!("Events: {}, last: start={:.3}s, dur={:.3}ms, area={:.4}", cnt, last.start_t, last.duration*1000.0, last.area));
                } else { ui.label("Events: 0"); }
            }
        }
        ui.separator();
        let editing = app.thr_editing.clone();
        let is_editing = editing.is_some();
        let header = if is_editing { "Edit" } else { "Add new" };
        ui.collapsing(header, |ui| {
            let kinds = [">", "<", "in range"];
            egui::ComboBox::from_label("Condition").selected_text(kinds[app.thr_builder.kind_idx]).show_ui(ui, |ui| { for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut app.thr_builder.kind_idx, i, *k); } });
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut app.thr_builder.name); });
            let trace_names: Vec<String> = app.trace_order.clone();
            egui::ComboBox::from_label("Trace").selected_text(trace_names.get(app.thr_builder.target_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.thr_builder.target_idx, i, n); } });
            match app.thr_builder.kind_idx {
                0 | 1 => { ui.horizontal(|ui| { ui.label("Value"); ui.add(egui::DragValue::new(&mut app.thr_builder.thr1).speed(0.01)); }); },
                _ => {
                    ui.horizontal(|ui| { ui.label("Low"); ui.add(egui::DragValue::new(&mut app.thr_builder.thr1).speed(0.01)); });
                    ui.horizontal(|ui| { ui.label("High"); ui.add(egui::DragValue::new(&mut app.thr_builder.thr2).speed(0.01)); });
                }
            }
            ui.horizontal(|ui| { ui.label("Min duration (ms)"); ui.add(egui::DragValue::new(&mut app.thr_builder.min_duration_ms).speed(0.1)); });
            ui.horizontal(|ui| { ui.label("Max events"); ui.add(egui::DragValue::new(&mut app.thr_builder.max_events).speed(1)); });
            if ui.button(if is_editing { "Save" } else { "Add threshold" }).clicked() {
                if let Some(nm) = trace_names.get(app.thr_builder.target_idx) { if !app.thr_builder.name.is_empty() {
                    let kind = match app.thr_builder.kind_idx { 0 => ThresholdKind::GreaterThan { value: app.thr_builder.thr1 }, 1 => ThresholdKind::LessThan { value: app.thr_builder.thr1 }, _ => ThresholdKind::InRange { low: app.thr_builder.thr1.min(app.thr_builder.thr2), high: app.thr_builder.thr1.max(app.thr_builder.thr2) } };
                    let def = ThresholdDef { name: app.thr_builder.name.clone(), target: crate::math::TraceRef(nm.clone()), kind, min_duration_s: (app.thr_builder.min_duration_ms / 1000.0).max(0.0), max_events: app.thr_builder.max_events };
                    if is_editing {
                        // replace existing by name
                        app.remove_threshold_internal(&editing.unwrap());
                        app.add_threshold_internal(def);
                    } else {
                        if app.threshold_defs.iter().any(|d| d.name == def.name) { app.thr_error = Some("A threshold with this name already exists".into()); } else { app.add_threshold_internal(def); app.thr_builder = ThresholdBuilderState::default(); }
                    }
                } }
            }
            if is_editing { if ui.button("Cancel").clicked() { app.thr_editing = None; app.thr_builder = ThresholdBuilderState::default(); app.thr_error = None; } }
        });

        ui.separator();
        ui.heading("Threshold events");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            // Build list of names from current thresholds and from the log
            let mut names: Vec<String> = app.threshold_defs.iter().map(|d| d.name.clone()).collect();
            for e in app.threshold_event_log.iter() { if !names.iter().any(|n| n == &e.threshold) { names.push(e.threshold.clone()); } }
            names.sort(); names.dedup();
            let mut sel = app.threshold_events_filter.clone();
            egui::ComboBox::from_id_salt("thr_events_filter")
                .selected_text(match &sel { Some(s) => format!("{}", s), None => "All".to_string() })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(sel.is_none(), "All").clicked() { sel = None; }
                    for n in &names { if ui.selectable_label(sel.as_ref() == Some(n), n).clicked() { sel = Some(n.clone()); } }
                });
            if sel != app.threshold_events_filter { app.threshold_events_filter = sel; }
            if ui.button("Export to CSV").clicked() {
                // Collect filtered events (newest first as shown)
                let evts: Vec<&ThresholdEvent> = app.threshold_event_log.iter().rev()
                    .filter(|e| app.threshold_events_filter.as_ref().map_or(true, |f| &e.threshold == f))
                    .collect();
                if !evts.is_empty() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("threshold_events.csv")
                        .add_filter("CSV", &["csv"]).save_file() {
                        if let Err(e) = super::export_helpers::save_threshold_events_csv(&path, &evts) { eprintln!("Failed to export events CSV: {e}"); }
                    }
                }
            }
        });
        // Build filtered, newest-first slice indices for table
        let filtered: Vec<&ThresholdEvent> = app
            .threshold_event_log
            .iter()
            .rev()
            .filter(|e| app.threshold_events_filter.as_ref().map_or(true, |f| &e.threshold == f))
            .collect();

        // Delegate for rendering with egui_table
        struct EventsDelegate<'a> {
            items: &'a [&'a ThresholdEvent],
            fmt: crate::config::XDateFormat,
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
                        0 => { ui.label(&e.threshold); }
                        1 => { ui.label(self.fmt.format_value(e.start_t)); }
                        2 => { ui.label(self.fmt.format_value(e.end_t)); }
                        3 => { ui.label(format!("{:.3}", e.duration * 1000.0)); }
                        4 => { ui.label(&e.trace); }
                        5 => { ui.label(format!("{:.6}", e.area)); }
                        _ => {}
                    }
                }
            }
        }

        let mut delegate = EventsDelegate { items: &filtered, fmt: app.x_date_format };
        let cols = vec![
            egui_table::Column::new(152.0),
            egui_table::Column::new(172.0),
            egui_table::Column::new(172.0),
            egui_table::Column::new(132.0),
            egui_table::Column::new(132.0),
            egui_table::Column::new(112.0),
        ];
        let avail_w = ui.available_width();
        let table_h = 260.0;
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(avail_w, table_h), egui::Sense::hover());
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
    });
    app.show_thresholds_dialog = show_flag;
}
