use super::panel_trait::{Panel, PanelState};
use crate::data::data::LivePlotData;
use crate::data::scope::ScopeData;
use crate::data::traces::TraceRef;
use crate::data::traces::TracesCollection;
use crate::data::triggers::{Trigger, TriggerSlope};
use crate::panels::trace_look_ui::render_trace_look_editor;
use egui::Ui;
use egui_plot::{HLine, Points, VLine};
use std::collections::HashMap;

pub struct TriggersPanel {
    pub state: PanelState,
    pub triggers: HashMap<String, Trigger>,
    pub builder: Option<Trigger>,
    pub editing: Option<String>,
}

impl Default for TriggersPanel {
    fn default() -> Self {
        let mut panel = Self {
            state: PanelState::new("Triggers", "🔔"),
            triggers: HashMap::new(),
            builder: None,
            editing: None,
        };
        // Add one default trigger on startup (disabled)
        let mut t = Trigger::default();
        t.name = "Trigger".to_string();
        t.enabled = false;
        panel.triggers.insert(t.name.clone(), t);
        panel
    }
}

impl Panel for TriggersPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn render_menu(&mut self, ui: &mut Ui, _data: &mut LivePlotData<'_>) {
        ui.menu_button("🔔 Trigger", |ui| {
            if ui.button("New").clicked() {
                let mut t = crate::data::triggers::Trigger::default();
                let idx = self.triggers.len() + 1;
                t.name = format!("Trigger{}", idx);
                t.enabled = false;
                self.triggers.insert(t.name.clone(), t);
                let st = self.state_mut();
                st.visible = true;
                st.detached = false;
                st.request_docket = true;
                ui.close();
            }
            if ui.button("Start all").clicked() {
                for (_n, trig) in self.triggers.iter_mut() {
                    trig.start();
                }
                ui.close();
            }
            if ui.button("Stop all").clicked() {
                for (_n, trig) in self.triggers.iter_mut() {
                    trig.stop();
                }
                ui.close();
            }
            if ui.button("Reset all").clicked() {
                for (_n, trig) in self.triggers.iter_mut() {
                    trig.reset_runtime_state();
                }
                ui.close();
            }
        });
    }

    fn clear_all(&mut self) {
        // Reset all triggers to disabled and clear last-trigger times
        for (_name, trig) in self.triggers.iter_mut() {
            trig.enabled = false;
            trig.reset_runtime_state();
        }
        self.builder = None;
        self.editing = None;
    }

    fn draw(
        &mut self,
        plot_ui: &mut egui_plot::PlotUi,
        scope: &ScopeData,
        traces: &TracesCollection,
    ) {
        if self.triggers.is_empty() {
            return;
        }
        let bounds = plot_ui.plot_bounds();
        let xr = bounds.range_x();
        let xmin = *xr.start();
        let xmax = *xr.end();

        for (name, trig) in self.triggers.iter() {
            if !trig.enabled {
                continue;
            }
            let Some(tr) = traces.get_trace(&trig.target) else {
                continue;
            };
            if !tr.look.visible {
                continue;
            }

            let color = trig.look.color;
            let mut width = trig.look.width.max(0.1);
            let style = trig.look.style;

            // Draw horizontal trigger level line
            let y_lin = trig.level + tr.offset;
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
                // Legend label can include info text
                let info = trig.get_info(&scope.y_axis);
                let label = if scope.show_info_in_legend {
                    format!("{} — {}", trig.name, info)
                } else {
                    trig.name.clone()
                };
                let h = HLine::new(label, y_plot)
                    .color(color)
                    .width(width)
                    .style(style);
                plot_ui.hline(h);
            }

            // Marker at last trigger time for each enabled trigger.
            if let Some(t) = trig.last_trigger_time() {
                if t >= xmin && t <= xmax {
                    // Emphasize when currently edited
                    if self.editing.as_deref() == Some(name) {
                        width *= 1.4;
                    }
                    let label = trig.name.clone();
                    if trig.look.show_points {
                        // Draw a point at the trigger level position
                        if y_plot.is_finite() {
                            let mut radius = trig.look.point_size;
                            if self.editing.as_deref() == Some(name) {
                                radius *= 1.2;
                            }
                            let p = Points::new(label, vec![[t, y_plot]])
                                .radius(radius)
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

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        let is_single_shot_triggered = self
            .triggers
            .values()
            .any(|tr| tr.single_shot && tr.is_triggered());

        if !is_single_shot_triggered {
            for (_name, tr) in self.triggers.iter_mut() {
                if tr.check_trigger(data) {
                    if tr.is_triggered() {
                        let tr_time = tr.last_trigger_time().unwrap();
                        let time_window =
                            data.scope_data.x_axis.bounds.1 - data.scope_data.x_axis.bounds.0;

                        let tr_pos = tr.trigger_position;
                        data.scope_data.x_axis.bounds = (
                            tr_time - time_window * tr_pos,
                            tr_time + time_window * (1.0 - tr_pos),
                        );
                    }
                }
            }
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
        ui.label("Trigger when a trace crosses a level; optionally pause after N samples.");

        ui.separator();

        // Global actions
        ui.horizontal(|ui| {
            if ui
                .button("♻ Reset all")
                .on_hover_text("Clear last trigger state for all triggers")
                .clicked()
            {
                // Resume if paused due to any trigger, then clear their state
                data.resume();
                for (_name, tr) in self.triggers.iter_mut() {
                    tr.reset();
                }
            }
            if ui
                .button("▶ Start all")
                .on_hover_text("Enable and start all triggers")
                .clicked()
            {
                // Resume stream and enable+start all triggers
                data.resume();
                for (_name, tr) in self.triggers.iter_mut() {
                    tr.enabled = true;
                    tr.start();
                }
            }
        });
        ui.add_space(6.0);

        // List existing triggers with enable toggle, quick info, and Remove button
        let mut removals: Vec<String> = Vec::new();
        for (name, tr) in self.triggers.iter_mut() {
            let name_str = name.clone();
            let mut to_remove = false;

            // Main row: enable toggle, name/info, remove button
            let row = ui.horizontal(|ui| {
                // Enable toggle
                ui.checkbox(&mut tr.enabled, "");

                // Clickable name to edit
                let name_resp =
                    ui.add(egui::Label::new(tr.name.clone()).sense(egui::Sense::click()));
                // Short info text
                let info = tr.get_info(&data.scope_data.y_axis);
                let info_resp = ui.add(egui::Label::new(info).sense(egui::Sense::click()));
                if name_resp.clicked() || info_resp.clicked() {
                    // Open editor with a copy of current settings
                    let mut t = Trigger::default();
                    t.name = tr.name.clone();
                    t.target = TraceRef(tr.target.0.clone());
                    t.enabled = tr.enabled;
                    t.level = tr.level;
                    t.slope = match tr.slope {
                        TriggerSlope::Rising => TriggerSlope::Rising,
                        TriggerSlope::Falling => TriggerSlope::Falling,
                        TriggerSlope::Any => TriggerSlope::Any,
                    };
                    t.single_shot = tr.single_shot;
                    t.trigger_position = tr.trigger_position;
                    t.look = tr.look.clone();
                    self.builder = Some(t);
                    self.editing = Some(name_str.clone());
                }
                if name_resp.hovered() || info_resp.hovered() {
                    // Highlight target trace when hovering the name
                    if !tr.target.0.is_empty() {
                        data.scope_data.hover_trace = Some(tr.target.clone());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 Remove").clicked() {
                        to_remove = true;
                    }
                });
            });

            // Hovering the whole row also highlights target trace
            if row.response.hovered() {
                if !tr.target.0.is_empty() {
                    data.scope_data.hover_trace = Some(tr.target.clone());
                }
            }

            // Stage removal (will be applied after the loop)
            if to_remove {
                removals.push(name_str.clone());
            }

            // Second row: Last info + Reset + Start/Stop
            let mut do_reset = false;
            let mut toggle_start: Option<bool> = None; // Some(true)=Start, Some(false)=Stop
            let (last_text, last_exists) = if let Some(t) = tr.last_trigger_time() {
                // Use x-axis formatter for time display
                let start_fmt = data.scope_data.x_axis.format_value(t, 4, 1.0);
                (format!("Last: {}", start_fmt), true)
            } else {
                (String::from("Last: –"), false)
            };
            let is_active = tr.is_active();
            let enabled_flag = tr.enabled;

            ui.horizontal(|ui| {
                ui.label(last_text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(last_exists, egui::Button::new("↺ Reset"))
                        .clicked()
                    {
                        do_reset = true;
                    }
                    if is_active {
                        if ui
                            .add_enabled(enabled_flag, egui::Button::new("⏹ Stop"))
                            .clicked()
                        {
                            toggle_start = Some(false);
                        }
                    } else {
                        if ui
                            .add_enabled(enabled_flag, egui::Button::new("▶ Start"))
                            .clicked()
                        {
                            toggle_start = Some(true);
                        }
                    }
                });
            });

            if do_reset {
                if tr.is_triggered() {
                    data.resume();
                }
                tr.reset();
            }
            if let Some(start) = toggle_start {
                if start {
                    data.resume();
                    tr.start();
                } else {
                    tr.stop();
                }
            }
        }

        // Apply removals after iteration to avoid mutable borrow conflicts
        if !removals.is_empty() {
            for n in removals {
                self.triggers.remove(&n);
                if self.editing.as_deref() == Some(&n) {
                    self.builder = None;
                    self.editing = None;
                }
            }
        }

        // New button
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("➕ New"))
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
            if self.editing.is_some() {
                ui.strong("Edit trigger");
            } else {
                ui.strong("New trigger");
            }
            ui.add_space(3.0);

            // Name
            let duplicate_name = self.triggers.contains_key(&builder.name)
                && self.editing.as_deref() != Some(builder.name.as_str());

            ui.horizontal(|ui| {
                ui.label("Name");
                if duplicate_name {
                    egui::Frame::default()
                        .stroke(egui::Stroke::new(1.5, egui::Color32::RED))
                        .show(ui, |ui| {
                            let resp = ui.add(egui::TextEdit::singleline(&mut builder.name));
                            let _resp = resp.on_hover_text(
                                "A trigger with this name already exists. Please choose another.",
                            );
                        });
                } else {
                    let resp = ui.add(egui::TextEdit::singleline(&mut builder.name));
                    let _resp = resp.on_hover_text("Enter a unique name for this trigger");
                }
            });

            // Target trace selection
            let trace_names = data.scope_data.trace_order.clone();
            let mut target_idx = trace_names
                .iter()
                .position(|n| n == &builder.target)
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
                builder.target = sel_name.clone();
            }

            // Level and slope
            ui.horizontal(|ui| {
                ui.label("Level");
                ui.add(egui::DragValue::new(&mut builder.level).speed(0.1));
                egui::ComboBox::from_label("Slope")
                    .selected_text(match builder.slope {
                        TriggerSlope::Rising => "Rising",
                        TriggerSlope::Falling => "Falling",
                        TriggerSlope::Any => "Any",
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(builder.slope, TriggerSlope::Rising),
                                "Rising",
                            )
                            .clicked()
                        {
                            builder.slope = TriggerSlope::Rising;
                        }
                        if ui
                            .selectable_label(
                                matches!(builder.slope, TriggerSlope::Falling),
                                "Falling",
                            )
                            .clicked()
                        {
                            builder.slope = TriggerSlope::Falling;
                        }
                        if ui
                            .selectable_label(matches!(builder.slope, TriggerSlope::Any), "Any")
                            .clicked()
                        {
                            builder.slope = TriggerSlope::Any;
                        }
                    });
            });

            // Trigger behavior

            ui.checkbox(&mut builder.enabled, "Enabled");
            ui.checkbox(&mut builder.single_shot, "Single shot");

            ui.horizontal(|ui| {
                ui.label("Trigger position (0..1)")
                    .on_hover_text("0 = pause now, 1 = pause after max_points");
                ui.add(egui::Slider::new(&mut builder.trigger_position, 0.0..=1.0).smart_aim(true))
                    .on_hover_text("0 = pause now, 1 = pause after max_points");
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
                let save_label = if self.editing.is_some() {
                    "Save"
                } else {
                    "Add trigger"
                };
                let can_save =
                    !builder.name.is_empty() && !builder.target.0.is_empty() && !duplicate_name;
                if ui
                    .add_enabled(can_save, egui::Button::new(save_label))
                    .clicked()
                {
                    // Stage a copy of the builder for saving after this UI block
                    let mut staged = Trigger::default();
                    staged.name = builder.name.clone();
                    staged.target = TraceRef(builder.target.0.clone());
                    staged.enabled = builder.enabled;
                    staged.level = builder.level;
                    staged.slope = match builder.slope {
                        TriggerSlope::Rising => TriggerSlope::Rising,
                        TriggerSlope::Falling => TriggerSlope::Falling,
                        TriggerSlope::Any => TriggerSlope::Any,
                    };
                    staged.single_shot = builder.single_shot;
                    staged.trigger_position = builder.trigger_position;
                    staged.look = builder.look.clone();
                    save_trigger = Some(staged);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });
            // Apply staged actions now that we are outside of the builder borrow scope
            if cancel_clicked {
                self.builder = None;
                self.editing = None;
            }
            if let Some(staged) = save_trigger {
                let key_old = self.editing.clone();
                let key_new = staged.name.clone();
                let entry = self
                    .triggers
                    .entry(key_new.clone())
                    .or_insert_with(|| Trigger::default());
                *entry = staged;
                if let Some(old) = key_old {
                    if old != key_new {
                        self.triggers.remove(&old);
                    }
                }
                self.builder = None;
                self.editing = None;
            }
        }
    }
}

impl TriggersPanel {
    pub fn reset_all(&mut self) {
        for (_name, tr) in self.triggers.iter_mut() {
            tr.reset();
        }
    }
}
