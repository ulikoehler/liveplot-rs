use super::panel_trait::{Panel, PanelState};
use crate::data::scope::ScopeData;
use crate::data::traces::TraceRef;
use crate::data::triggers::{Trigger, TriggerSlope};
use crate::panels::trace_look_ui::render_trace_look_editor;
use egui::Ui;
use egui_plot::{HLine, Points, VLine};
use std::collections::HashMap;

pub struct TriggersPanel {
    pub state: PanelState,
    pub triggers: HashMap<String, Trigger>,
    pub builder : Option<Trigger>,
    pub editing: Option<String>,
}

impl Default for TriggersPanel {
    fn default() -> Self {
        Self {
            state: PanelState {
                title: "Triggers",
                visible: false,
                detached: false,
                request_docket: false,
            },
            triggers: HashMap::new(),
            builder: None,
            editing: None,
        }
    }
}

impl Panel for TriggersPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn draw(&mut self, plot_ui: &mut egui_plot::PlotUi, data: &ScopeData) {
        if self.triggers.is_empty() { return; }
        let bounds = plot_ui.plot_bounds();
        let xr = bounds.range_x();
        let xmin = *xr.start();
        let xmax = *xr.end();

        for (name, trig) in self.triggers.iter() {
            if !trig.enabled { continue; }
            let Some(tr) = data.traces.get(&trig.target.0) else { continue; };
            if !tr.look.visible { continue; }

            let color = trig.look.color;
            let width = trig.look.width.max(0.1);
            let style = trig.look.style;

            // Draw horizontal trigger level line
            let y_lin = trig.level + tr.offset;
            let y_plot = if data.y_axis.log_scale {
                if y_lin > 0.0 { y_lin.log10() } else { f64::NAN }
            } else { y_lin };
            if y_plot.is_finite() {
                // Legend label can include info text
                let info = trig.get_info(&data.y_axis);
                let label = if data.show_info_in_legend {
                    format!("{} — {}", trig.name, info)
                } else {
                    trig.name.clone()
                };
                let h = HLine::new(label, y_plot).color(color).width(width).style(style);
                plot_ui.hline(h);
            }

            // Selected marker at last trigger time: point or vline based on look.show_points
            if self.editing.as_deref() == Some(name) {
                if let Some(t) = trig.last_trigger_time() {
                    if t >= xmin && t <= xmax {
                        let label = trig.name.clone();
                        if trig.look.show_points {
                            // Draw a point at the trigger level position
                            if y_plot.is_finite() {
                                let p = Points::new(label, vec![[t, y_plot]])
                                    .radius(trig.look.point_size)
                                    .shape(trig.look.marker)
                                    .color(color);
                                plot_ui.points(p);
                            }
                        } else {
                            // Draw a vertical line at trigger time
                            let v = VLine::new(label, t).color(color).width(width).style(style);
                            plot_ui.vline(v);
                        }
                    }
                }
            }
        }
    }

    fn update_data(&mut self, _data: &mut ScopeData) {
        for (_name, trigger) in self.triggers.iter_mut() {
            trigger.check_trigger(_data);
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut ScopeData) {
        ui.label("Trigger when a trace crosses a level; optionally pause after N samples.");
        ui.separator();

        // List existing triggers with enable toggle, quick info, and Remove button
        let names_snapshot: Vec<String> = self.triggers.keys().cloned().collect();
        for name in names_snapshot {
            let mut to_remove = false;
            let row = ui.horizontal(|ui| {
                if let Some(tr) = self.triggers.get_mut(&name) {
                    // Enable toggle
                    ui.checkbox(&mut tr.enabled, "");

                    // Clickable name to edit
                    let name_resp = ui.add(egui::Label::new(tr.name.clone()).sense(egui::Sense::click()));
                    if name_resp.clicked() {
                        // Open editor with a copy of current settings
                        let mut t = Trigger::default();
                        t.name = tr.name.clone();
                        t.target = TraceRef(tr.target.0.clone());
                        t.enabled = tr.enabled;
                        t.level = tr.level;
                        t.slope = match tr.slope { TriggerSlope::Rising => TriggerSlope::Rising, TriggerSlope::Falling => TriggerSlope::Falling, TriggerSlope::Any => TriggerSlope::Any };
                        t.single_shot = tr.single_shot;
                        t.trigger_position = tr.trigger_position;
                        t.look = tr.look.clone();
                        self.builder = Some(t);
                        self.editing = Some(name.clone());
                    }
                    if name_resp.hovered() {
                        // Highlight target trace when hovering the name
                        if !tr.target.0.is_empty() {
                            data.hover_trace = Some(tr.target.0.clone());
                        }
                    }

                    // Short info text
                    let info = tr.get_info(&data.y_axis);
                    ui.label(info);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Remove").clicked() { to_remove = true; }
                    });
                }
            });
            if row.response.hovered() {
                if let Some(tr) = self.triggers.get(&name) {
                    if !tr.target.0.is_empty() { data.hover_trace = Some(tr.target.0.clone()); }
                }
            }
            if to_remove {
                self.triggers.remove(&name);
                if self.editing.as_deref() == Some(&name) { self.builder = None; self.editing = None; }
            }
        }

        // New button
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("New"))
            .on_hover_text("Create a new trigger")
            .clicked();
        if new_clicked {
            self.builder = Some(Trigger::default());
            self.editing = None;
        }

        // Editor for creating/editing
        if let Some(builder) = &mut self.builder {
            ui.add_space(12.0);
            ui.separator();
            if self.editing.is_some() { ui.strong("Edit trigger"); } else { ui.strong("New trigger"); }
            ui.add_space(3.0);

            // Name
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut builder.name); });

            // Target trace selection
            let trace_names: Vec<String> = data.trace_order.clone();
            let mut target_idx = trace_names.iter().position(|n| n == &builder.target.0).unwrap_or(0);
            egui::ComboBox::from_label("Trace")
                .selected_text(trace_names.get(target_idx).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, nm) in trace_names.iter().enumerate() {
                        if ui.selectable_label(i == target_idx, nm).clicked() { target_idx = i; }
                    }
                });
            if let Some(sel_name) = trace_names.get(target_idx) { builder.target = TraceRef(sel_name.clone()); }

            // Level and slope
            ui.horizontal(|ui| {
                ui.label("Level");
                ui.add(egui::DragValue::new(&mut builder.level).speed(0.1));
                egui::ComboBox::from_label("Slope")
                    .selected_text(match builder.slope { TriggerSlope::Rising => "Rising", TriggerSlope::Falling => "Falling", TriggerSlope::Any => "Any" })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(matches!(builder.slope, TriggerSlope::Rising), "Rising").clicked() { builder.slope = TriggerSlope::Rising; }
                        if ui.selectable_label(matches!(builder.slope, TriggerSlope::Falling), "Falling").clicked() { builder.slope = TriggerSlope::Falling; }
                        if ui.selectable_label(matches!(builder.slope, TriggerSlope::Any), "Any").clicked() { builder.slope = TriggerSlope::Any; }
                    });
            });

            // Trigger behavior
            ui.horizontal(|ui| {
                ui.checkbox(&mut builder.enabled, "Enabled");
                ui.checkbox(&mut builder.single_shot, "Single shot");
            });
            ui.horizontal(|ui| {
                ui.label("Trigger position (0..1)");
                ui.add(egui::Slider::new(&mut builder.trigger_position, 0.0..=1.0).smart_aim(true));
                ui.label("0 = pause now, 1 = pause after max_points");
            });

            // Style
            ui.add_space(5.0);
            egui::CollapsingHeader::new("Style")
                .default_open(false)
                .show(ui, |ui| {
                    render_trace_look_editor(&mut builder.look, ui, false);
                });

            // Save/Add + cancel
            ui.add_space(10.0);
            let mut cancel_clicked = false;
            let mut save_trigger: Option<Trigger> = None;
            ui.horizontal(|ui| {
                let save_label = if self.editing.is_some() { "Save" } else { "Add trigger" };
                let can_save = !builder.name.is_empty() && !builder.target.0.is_empty();
                if ui.add_enabled(can_save, egui::Button::new(save_label)).clicked() {
                    // Stage a copy of the builder for saving after this UI block
                    let mut staged = Trigger::default();
                    staged.name = builder.name.clone();
                    staged.target = TraceRef(builder.target.0.clone());
                    staged.enabled = builder.enabled;
                    staged.level = builder.level;
                    staged.slope = match builder.slope { TriggerSlope::Rising => TriggerSlope::Rising, TriggerSlope::Falling => TriggerSlope::Falling, TriggerSlope::Any => TriggerSlope::Any };
                    staged.single_shot = builder.single_shot;
                    staged.trigger_position = builder.trigger_position;
                    staged.look = builder.look.clone();
                    save_trigger = Some(staged);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() { cancel_clicked = true; }
                });
            });
            // Apply staged actions now that we are outside of the builder borrow scope
            if cancel_clicked { self.builder = None; self.editing = None; }
            if let Some(staged) = save_trigger {
                let key_old = self.editing.clone();
                let key_new = staged.name.clone();
                let entry = self.triggers.entry(key_new.clone()).or_insert_with(|| Trigger::default());
                *entry = staged;
                if let Some(old) = key_old { if old != key_new { self.triggers.remove(&old); } }
                self.builder = None;
                self.editing = None;
            }
        }
    }
}
