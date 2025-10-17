use egui::Ui;
use super::panel_trait::{Panel, PanelState};
use crate::data::DataContext;

use crate::data::thresholds as thr;

use crate::panels::trace_look_ui::render_trace_look_editor;
use crate::data::trace_look::TraceLook;

#[derive(Debug, Clone)]
struct BuilderState {
    name: String,
    target_idx: usize,
    kind_idx: usize, // 0: >, 1: <, 2: in range
    thr1: f64,
    thr2: f64,
    min_duration_ms: f64,
    max_events: usize,
    look_line: TraceLook,
    look_start: TraceLook,
    look_stop: TraceLook,
}
impl Default for BuilderState {
    fn default() -> Self { Self { name: String::new(), target_idx: 0, kind_idx: 0, thr1: 0.0, thr2: 1.0, min_duration_ms: 2.0, max_events: 100, look_line: TraceLook::default(), look_start: TraceLook::default(), look_stop: TraceLook::default() } }
}

pub struct ThresholdsPanel {
    pub state: PanelState,
    builder: BuilderState,
    editing: Option<String>,
    creating: bool,
    error: Option<String>,
    looks: std::collections::HashMap<String, TraceLook>,
    start_looks: std::collections::HashMap<String, TraceLook>,
    stop_looks: std::collections::HashMap<String, TraceLook>,
    events_filter: Option<String>,
}
impl Default for ThresholdsPanel {
    fn default() -> Self { Self { state: PanelState { visible: false, detached: false }, builder: Default::default(), editing: None, creating: false, error: None, looks: Default::default(), start_looks: Default::default(), stop_looks: Default::default(), events_filter: None } }
}
impl Panel for ThresholdsPanel {
    fn name(&self) -> &'static str { "Thresholds" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui, data: &mut DataContext) {
        ui.label("Detect and log when a trace exceeds a condition.");
        if let Some(err) = &self.error { ui.colored_label(egui::Color32::LIGHT_RED, err); }
        ui.separator();

        // List existing thresholds
        for def in data.thresholds.defs.clone().iter() {
            ui.horizontal(|ui| {
                // Color editor
                let mut look = self.looks.get(&def.name).cloned().unwrap_or_else(|| {
                    let mut l = TraceLook::default();
                    if let Some(rgb) = def.color_hint { l.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]); }
                    l
                });
                let mut col = look.color;
                if ui.color_edit_button_srgba(&mut col).changed() {
                    look.color = col;
                    self.looks.insert(def.name.clone(), look.clone());
                    if let Some(s) = self.start_looks.get_mut(&def.name) { s.color = col; }
                    if let Some(s) = self.stop_looks.get_mut(&def.name) { s.color = col; }
                }
                // Name clickable to edit
                if ui.button(&def.name).clicked() {
                    self.builder = BuilderState::default();
                    self.builder.name = def.name.clone();
                    // target index from trace list
                    let names: Vec<String> = data.traces.trace_order.clone();
                    self.builder.target_idx = names.iter().position(|n| *n == def.target.0).unwrap_or(0);
                    match &def.kind {
                        thr::ThresholdKind::GreaterThan { value } => { self.builder.kind_idx = 0; self.builder.thr1 = *value; }
                        thr::ThresholdKind::LessThan { value } => { self.builder.kind_idx = 1; self.builder.thr1 = *value; }
                        thr::ThresholdKind::InRange { low, high } => { self.builder.kind_idx = 2; self.builder.thr1 = *low; self.builder.thr2 = *high; }
                    }
                    self.builder.min_duration_ms = def.min_duration_s * 1000.0;
                    self.builder.max_events = def.max_events;
                    if let Some(l) = self.looks.get(&def.name) { self.builder.look_line = l.clone(); }
                    if let Some(l) = self.start_looks.get(&def.name) { self.builder.look_start = l.clone(); }
                    if let Some(l) = self.stop_looks.get(&def.name) { self.builder.look_stop = l.clone(); }
                    self.editing = Some(def.name.clone());
                    self.creating = false;
                    self.error = None;
                }
                // Info
                let info_text = match &def.kind {
                    thr::ThresholdKind::GreaterThan { value } => format!("{} > {:.3}", def.target.0, value),
                    thr::ThresholdKind::LessThan { value } => format!("{} < {:.3}", def.target.0, value),
                    thr::ThresholdKind::InRange { low, high } => format!("{} in [{:.3}, {:.3}]", def.target.0, low, high),
                };
                ui.label(info_text);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Remove").clicked() {
                        data.thresholds.remove_def(&def.name);
                        if self.editing.as_deref() == Some(&def.name) { self.editing = None; self.creating = false; self.builder = Default::default(); self.error = None; }
                    }
                    if ui.button("Clear").on_hover_text("Clear events for this threshold").clicked() {
                        data.thresholds.clear_events_for(&def.name);
                    }
                });
            });
            // Short summary
            if let Some(st) = data.thresholds.state.get(&def.name) {
                if let Some(last) = st.events.back() {
                    ui.label(format!("Events: {} • last: {} • {} ms • area {}", st.events.len(), crate::config::XDateFormat::Iso8601Time.format_value(last.start_t), format!("{:.3}", last.duration * 1000.0), format!("{:.4}", last.area)));
                } else { ui.label("Events: 0"); }
            }
        }

        ui.add_space(6.0);
        if ui.add_sized([ui.available_width(), 24.0], egui::Button::new("New")).clicked() {
            self.builder = Default::default();
            self.editing = None;
            self.creating = true;
            self.error = None;
        }

        let is_editing = self.editing.is_some();
        let is_creating = self.creating;
        if is_editing || is_creating {
            ui.add_space(10.0);
            ui.separator();
            ui.strong(if is_editing { "Edit threshold" } else { "New threshold" });
            ui.add_space(3.0);

            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.builder.name); });
            let trace_names: Vec<String> = data.traces.trace_order.clone();
            egui::ComboBox::from_label("Trace")
                .selected_text(trace_names.get(self.builder.target_idx).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut self.builder.target_idx, i, n); }
                });
            // Default color when creating: use selected trace color at 75% alpha
            if is_creating {
                if let Some(sel) = trace_names.get(self.builder.target_idx) {
                    if let Some(tr) = data.traces.traces.get(sel) {
                        if self.builder.look_line.color == egui::Color32::LIGHT_BLUE { let c = tr.look.color; self.builder.look_line.color = egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 191); }
                        self.builder.look_start.color = self.builder.look_line.color;
                        self.builder.look_stop.color = self.builder.look_line.color;
                    }
                }
            }
            let kinds = [">", "<", "in range"];
            egui::ComboBox::from_label("Condition")
                .selected_text(kinds[self.builder.kind_idx])
                .show_ui(ui, |ui| { for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut self.builder.kind_idx, i, *k); } });
            match self.builder.kind_idx { 0 | 1 => { ui.horizontal(|ui| { ui.label("Value"); ui.add(egui::DragValue::new(&mut self.builder.thr1).speed(0.01)); }); }, _ => {
                ui.horizontal(|ui| { ui.label("Low"); ui.add(egui::DragValue::new(&mut self.builder.thr1).speed(0.01)); });
                ui.horizontal(|ui| { ui.label("High"); ui.add(egui::DragValue::new(&mut self.builder.thr2).speed(0.01)); });
            }}
            ui.horizontal(|ui| { ui.label("Min duration (ms)"); ui.add(egui::DragValue::new(&mut self.builder.min_duration_ms).speed(0.1)); });
            ui.horizontal(|ui| { ui.label("Max events"); ui.add(egui::DragValue::new(&mut self.builder.max_events).speed(1)); });

            ui.add_space(5.0);
            egui::CollapsingHeader::new("Style: Threshold line").default_open(false).show(ui, |ui| { render_trace_look_editor(&mut self.builder.look_line, ui, false); });
            // keep start/stop colors locked to line color
            self.builder.look_start.color = self.builder.look_line.color;
            self.builder.look_stop.color = self.builder.look_line.color;
            egui::CollapsingHeader::new("Style: Event start").default_open(false).show(ui, |ui| { render_trace_look_editor(&mut self.builder.look_start, ui, true); });
            egui::CollapsingHeader::new("Style: Event stop").default_open(false).show(ui, |ui| { render_trace_look_editor(&mut self.builder.look_stop, ui, true); });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add threshold" };
                let mut save_clicked = false;
                if ui.button(save_label).clicked() { save_clicked = true; }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() { self.editing = None; self.creating = false; self.builder = Default::default(); self.error = None; }
                });
                if save_clicked {
                    if let Some(nm) = trace_names.get(self.builder.target_idx) {
                        if !self.builder.name.is_empty() {
                            let kind = match self.builder.kind_idx { 0 => thr::ThresholdKind::GreaterThan { value: self.builder.thr1 }, 1 => thr::ThresholdKind::LessThan { value: self.builder.thr1 }, _ => thr::ThresholdKind::InRange { low: self.builder.thr1.min(self.builder.thr2), high: self.builder.thr1.max(self.builder.thr2) } };
                            let def = thr::ThresholdDef { name: self.builder.name.clone(), display_name: None, target: crate::data::math::TraceRef(nm.clone()), kind, color_hint: Some([self.builder.look_line.color.r(), self.builder.look_line.color.g(), self.builder.look_line.color.b()]), min_duration_s: (self.builder.min_duration_ms/1000.0).max(0.0), max_events: self.builder.max_events };
                            if is_editing { let orig = self.editing.clone().unwrap(); data.thresholds.remove_def(&orig); data.thresholds.add_def(def.clone()); self.looks.insert(def.name.clone(), self.builder.look_line.clone()); self.start_looks.insert(def.name.clone(), self.builder.look_start.clone()); self.stop_looks.insert(def.name.clone(), self.builder.look_stop.clone()); self.editing=None; self.creating=false; self.builder=Default::default(); self.error=None; }
                            else { if data.thresholds.defs.iter().any(|d| d.name == def.name) { self.error = Some("A threshold with this name already exists".into()); } else { data.thresholds.add_def(def.clone()); self.looks.insert(def.name.clone(), self.builder.look_line.clone()); self.start_looks.insert(def.name.clone(), self.builder.look_start.clone()); self.stop_looks.insert(def.name.clone(), self.builder.look_stop.clone()); self.creating=false; self.builder=Default::default(); self.error=None; } }
                        }
                    }
                }
            });
        }

        ui.separator();
        ui.heading("Threshold events");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let mut names: Vec<String> = data.thresholds.defs.iter().map(|d| d.name.clone()).collect();
            for e in &data.thresholds.event_log { if !names.iter().any(|n| n == &e.threshold) { names.push(e.threshold.clone()); } }
            names.sort(); names.dedup();
            let mut sel = self.events_filter.clone();
            egui::ComboBox::from_id_salt("thr_events_filter").selected_text(match &sel { Some(s) => s.clone(), None => "All".to_string() }).show_ui(ui, |ui| {
                if ui.selectable_label(sel.is_none(), "All").clicked() { sel = None; }
                for n in &names { if ui.selectable_label(sel.as_ref()==Some(n), n).clicked() { sel = Some(n.clone()); } }
            });
            if sel != self.events_filter { self.events_filter = sel; }
            if ui.button("Export to CSV").clicked() {
                let evts: Vec<&thr::ThresholdEvent> = data.thresholds.event_log.iter().rev().filter(|e| self.events_filter.as_ref().map_or(true, |f| &e.threshold == f)).collect();
                if !evts.is_empty() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("threshold_events.csv").add_filter("CSV", &["csv"]).save_file() {
                        if let Err(e) = save_events_csv(&path, &evts) { eprintln!("Failed to export events CSV: {e}"); }
                    }
                }
            }
            if ui.button("Clear events").on_hover_text("Delete all threshold events").clicked() { data.thresholds.clear_all_events(); }
        });

        // Events table (newest first)
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.push_id("thr_events_table", |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Threshold"); ui.add_space(8.0);
                    ui.strong("Start time"); ui.add_space(8.0);
                    ui.strong("End time"); ui.add_space(8.0);
                    ui.strong("Duration (ms)"); ui.add_space(8.0);
                    ui.strong("Trace"); ui.add_space(8.0);
                    ui.strong("Area"); ui.add_space(8.0);
                    ui.add_space(ui.available_width()-10.0); // stretch
                });
                ui.separator();
                // To support removing during iteration, collect indices
                let filtered: Vec<_> = data.thresholds.event_log.iter().enumerate().rev().filter(|(_i,e)| self.events_filter.as_ref().map_or(true, |f| &e.threshold == f)).collect();
                let mut to_remove: Vec<thr::ThresholdEvent> = Vec::new();
                for (_idx, e) in filtered {
                    ui.horizontal(|ui| {
                        ui.label(&e.threshold);
                        ui.label(crate::config::XDateFormat::Iso8601Time.format_value(e.start_t));
                        ui.label(crate::config::XDateFormat::Iso8601Time.format_value(e.end_t));
                        ui.label(format!("{:.3}", e.duration * 1000.0));
                        ui.label(&e.trace);
                        ui.label(format!("{:.6}", e.area));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Clear").clicked() { to_remove.push(e.clone()); }
                        });
                    });
                }
                for ev in to_remove { data.thresholds.remove_event(&ev); }
            });
        });
    }
}

fn save_events_csv(path: &std::path::Path, items: &[&thr::ThresholdEvent]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "threshold,trace,start_time,end_time,duration_ms,area")?;
    for e in items {
        writeln!(f, "{},{},{},{},{:.3},{}", e.threshold, e.trace, crate::config::XDateFormat::Iso8601Time.format_value(e.start_t), crate::config::XDateFormat::Iso8601Time.format_value(e.end_t), e.duration * 1000.0, e.area)?;
    }
    Ok(())
}
