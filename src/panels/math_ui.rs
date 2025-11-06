use crate::data::data::LivePlotData;
use crate::data::math::{FilterKind, MathKind, MathTrace, MinMaxMode};
use crate::data::traces::TraceRef;
use eframe::egui;
use egui::{Color32, Ui};
use std::collections::HashMap;

//use super::app::ScopeAppMulti;
use crate::data::trace_look::TraceLook;
use crate::panels::panel_trait::{Panel, PanelState};
use crate::panels::trace_look_ui::render_trace_look_editor;

#[derive(Debug, Clone)]
pub struct MathPanel {
    state: PanelState,
    builder: MathTrace,
    builder_look: TraceLook,
    editing: Option<TraceRef>,
    error: Option<String>,
    creating: bool,

    math_traces: Vec<MathTrace>,
}

impl Default for MathPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("∫ Math"),
            builder: MathTrace::new(TraceRef::default(), MathKind::Add { inputs: Vec::new() }),
            builder_look: TraceLook::default(),
            editing: None,
            error: None,
            creating: false,

            math_traces: Vec::new(),
        }
    }
}

impl Panel for MathPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn update_data(&mut self, _data: &mut LivePlotData<'_>) {
        let mut sources: HashMap<TraceRef, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in _data.traces.traces_iter() {
            sources.insert(name.clone(), tr.live.iter().copied().collect());
        }

        for def in self.math_traces.iter_mut() {
            let out = def.compute_math_trace(sources.clone());

            let tr = _data.get_trace_or_new(&def.name);
            tr.live = out.iter().copied().collect();

            sources.insert(def.name.clone(), out);
        }

        sources.clear();
        for (name, tr) in _data.traces.traces_iter() {
            if let Some(data) = tr.snap.clone() {
                sources.insert(name.clone(), data.iter().copied().collect());
            }
        }

        for def in self.math_traces.iter_mut() {
            let out = def.compute_math_trace(sources.clone());

            let tr = _data.get_trace_or_new(&def.name);
            tr.snap = Some(out.iter().copied().collect());

            sources.insert(def.name.clone(), out);
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
        ui.label("Create virtual traces from existing ones.");
        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        ui.separator();
        // Global storage reset for all stateful math traces
        ui.horizontal(|ui| {
            if ui
                .button("Reset All Storage")
                .on_hover_text("Reset integrators, filters, min/max for all math traces")
                .clicked()
            {
                for def in self.math_traces.iter_mut() {
                    // def.reset_math_storage();
                    data.traces.clear_trace(&def.name);
                }
            }
        });
        ui.add_space(6.0);
        // Existing math traces list with color editor, name, info, and Remove (right-aligned)
        // Reset hover before drawing; rows will set it when hovered

        let mut hover_trace_intern = None;
        for def in self.math_traces.clone().iter_mut() {
            let row = ui.horizontal(|ui| {
                // Color editor like in traces_ui
                if let Some(tr) = data.traces.get_trace_mut(&def.name) {
                    let mut c = tr.look.color;
                    let resp = ui
                        .color_edit_button_srgba(&mut c)
                        .on_hover_text("Change trace color");
                    if resp.hovered() {
                        hover_trace_intern = Some(def.name.clone());
                    }
                    if resp.changed() {
                        tr.look.color = c;
                    }
                } else {
                    ui.label("");
                }

                // Name (click to edit)
                let name_resp = ui.add(
                    egui::Label::new(def.name.0.clone())
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if name_resp.hovered() {
                    hover_trace_intern = Some(def.name.clone());
                }
                if name_resp.clicked() {
                    self.builder = def.clone();
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Info string (formula) - clickable to edit
                let info_text = if let Some(tr) = data.traces.get_trace(&def.name) {
                    tr.info.clone()
                } else {
                    String::new()
                };
                let info_resp = ui.add(
                    egui::Label::new(info_text)
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if info_resp.hovered() {
                    hover_trace_intern = Some(def.name.clone());
                }
                if info_resp.clicked() {
                    self.builder = def.clone();
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }
                // Right-aligned per-trace actions: Reset and Remove
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Remove button with hover highlight
                    let remove_resp = ui.button("Remove");
                    if remove_resp.hovered() {
                        hover_trace_intern = Some(def.name.clone());
                    }
                    if remove_resp.clicked() {
                        let removing = def.name.clone();
                        data.remove_trace(&removing);
                        self.math_traces.retain(|d| d.name != removing);
                        if self.editing.as_deref() == Some(&removing) {
                            self.editing = None;
                            self.creating = false;
                            self.builder = MathTrace::new(
                                TraceRef::default(),
                                MathKind::Add { inputs: Vec::new() },
                            );
                            self.builder_look = TraceLook::default();
                            self.error = None;
                        }
                    }
                    // Show Reset for kinds that have internal storage
                    let is_stateful = matches!(
                        def.kind,
                        MathKind::Integrate { .. }
                            | MathKind::Filter { .. }
                            | MathKind::MinMax { .. }
                    );
                    if is_stateful {
                        let reset_resp = ui
                            .button("Reset")
                            .on_hover_text("Reset integrator/filter/min/max state for this trace");
                        if reset_resp.hovered() {
                            hover_trace_intern = Some(def.name.clone());
                        }
                        if reset_resp.clicked() {
                            // def.reset_math_storage();
                            data.traces.clear_trace(&def.name);
                        }
                    }
                });
            });
            if row.response.hovered() {
                hover_trace_intern = Some(def.name.clone());
            }
        }
        if let Some(nm) = hover_trace_intern {
            data.scope_data.hover_trace = Some(nm);
        }

        // Style popup removed; the editor is part of the new/edit dialog above

        // Full-width New button after the list
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("New"))
            .on_hover_text("Create a new math trace")
            .clicked();
        if new_clicked {
            self.builder =
                MathTrace::new(TraceRef::default(), MathKind::Add { inputs: Vec::new() });
            self.editing = None;
            self.error = None;
            self.creating = true;
            self.builder_look = TraceLook::default();
        }

        // Settings panel (hidden unless creating or editing)
        let is_editing = self.editing.is_some();
        let is_creating = self.creating;
        if is_editing || is_creating {
            ui.add_space(12.0);
            ui.separator();
            if is_editing {
                ui.strong("Edit math trace");
            } else {
                ui.strong("New math trace");
            }
            // Name first, then Operation (no label; tooltip on combobox)
            // Duplicate name when creating, or when editing and changing to an existing different name
            let duplicate_name = {
                let same_as_editing = self.editing.as_deref() == Some(self.builder.name.as_str());
                let exists_in_math = self.math_traces.iter().any(|d| d.name == self.builder.name);
                let exists_in_data = data.traces.contains_key(&self.builder.name);
                (exists_in_math || exists_in_data)
                    && !same_as_editing
                    && !self.builder.name.is_empty()
            };

            ui.horizontal(|ui| {
                ui.label("Name");
                if duplicate_name {
                    egui::Frame::default()
                        .stroke(egui::Stroke::new(1.5, egui::Color32::RED))
                        .show(ui, |ui| {
                            let resp = ui.add(egui::TextEdit::singleline(&mut self.builder.name.0));
                            let _ = resp.on_hover_text(
                                "A trace with this name already exists. Please choose another.",
                            );
                        });
                } else {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.builder.name.0));
                    let _ = resp.on_hover_text("Enter a unique name for this trace");
                }
            });
            // Operation selection
            let kinds = [
                "Add/Subtract",
                "Multiply",
                "Divide",
                "Differentiate",
                "Integrate",
                "Filter",
                "Min",
                "Max",
            ];
            let mut kind_idx: usize = match &self.builder.kind {
                MathKind::Add { .. } => 0,
                MathKind::Multiply { .. } => 1,
                MathKind::Divide { .. } => 2,
                MathKind::Differentiate { .. } => 3,
                MathKind::Integrate { .. } => 4,
                MathKind::Filter { .. } => 5,
                MathKind::MinMax { mode, .. } => match mode {
                    MinMaxMode::Min => 6,
                    MinMaxMode::Max => 7,
                },
            };

            let prev_kind_idx = kind_idx;
            let ir = egui::ComboBox::from_id_salt("math_op")
                .selected_text(kinds[kind_idx])
                .show_ui(ui, |ui| {
                    for (i, k) in kinds.iter().enumerate() {
                        ui.selectable_value(&mut kind_idx, i, *k);
                    }
                });
            ir.response.on_hover_text("Operation");
            // Available source names, excluding the math trace's own name to avoid self-references
            let trace_names: Vec<TraceRef> = data
                .scope_data
                .trace_order
                .clone()
                .into_iter()
                .filter(|n| *n != self.builder.name)
                .collect();

            if kind_idx != prev_kind_idx {
                // Switch to a new kind with sensible defaults
                let first = trace_names.get(0).cloned().unwrap_or_default();
                let second = trace_names.get(1).cloned().unwrap_or_else(|| first.clone());
                self.builder.kind = match kind_idx {
                    0 => MathKind::Add {
                        inputs: vec![(first.clone(), 1.0), (second.clone(), 1.0)],
                    },
                    1 => MathKind::Multiply {
                        a: first.clone(),
                        b: second.clone(),
                    },
                    2 => MathKind::Divide {
                        a: first.clone(),
                        b: second.clone(),
                    },
                    3 => MathKind::Differentiate {
                        input: first.clone(),
                    },
                    4 => MathKind::Integrate {
                        input: first.clone(),
                        y0: 0.0,
                    },
                    5 => MathKind::Filter {
                        input: first.clone(),
                        kind: FilterKind::Lowpass { cutoff_hz: 1.0 },
                    },
                    6 => MathKind::MinMax {
                        input: first.clone(),
                        decay_per_sec: Some(0.0),
                        mode: MinMaxMode::Min,
                    },
                    7 => MathKind::MinMax {
                        input: first.clone(),
                        decay_per_sec: Some(0.0),
                        mode: MinMaxMode::Max,
                    },
                    _ => MathKind::Add { inputs: vec![] },
                };
            }

            // Initialize builder look color if blank name changed to a new one (use palette color based on future index)
            // Compute default color index for this potential new trace name
            if is_creating {
                let future_idx = if data.traces.contains_key(&self.builder.name) {
                    None
                } else {
                    Some(data.scope_data.trace_order.len())
                };
                if let Some(idx) = future_idx {
                    self.builder_look.color = TraceLook::alloc_color(idx);
                }
            }

            match &mut self.builder.kind {
                MathKind::Add { inputs } => {
                    // Ensure at least one input row for UX
                    if inputs.is_empty() {
                        if let Some(nm) = trace_names.get(0) {
                            inputs.push((nm.clone(), 1.0));
                        }
                    }
                    // Draw rows
                    for (idx, (trace_ref, gain)) in inputs.iter_mut().enumerate() {
                        let current_name = trace_ref.0.clone();
                        ui.horizontal(|ui| {
                            let mut selected = current_name.clone();
                            egui::ComboBox::from_id_salt(format!("add_sel_{}", idx))
                                .selected_text(selected.clone())
                                .show_ui(ui, |ui| {
                                    for n in trace_names.iter() {
                                        ui.selectable_value(&mut selected, n.0.clone(), n.0.clone());
                                    }
                                });
                            if selected != current_name {
                                *trace_ref = TraceRef(selected);
                            }
                            ui.label("gain");
                            ui.add(egui::DragValue::new(gain).speed(0.1));
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add input").clicked() {
                            let nm = trace_names.get(0).cloned().unwrap_or_default();
                            inputs.push((nm, 1.0));
                        }
                        if ui.button("Remove input").clicked() {
                            if inputs.len() > 1 {
                                inputs.pop();
                            }
                        }
                    });
                }
                MathKind::Multiply { a, b } | MathKind::Divide { a, b } => {
                    ui.horizontal(|ui| {
                        let mut sel_a = a.0.clone();
                        egui::ComboBox::from_label("A")
                            .selected_text(sel_a.clone())
                            .show_ui(ui, |ui| {
                                for n in trace_names.iter() {
                                    ui.selectable_value(&mut sel_a, n.0.clone(), n.0.clone());
                                }
                            });
                        if sel_a != a.0 {
                            a.0 = sel_a;
                        }
                        let mut sel_b = b.0.clone();
                        egui::ComboBox::from_label("B")
                            .selected_text(sel_b.clone())
                            .show_ui(ui, |ui| {
                                for n in trace_names.iter() {
                                    ui.selectable_value(&mut sel_b, n.0.clone(), n.0.clone());
                                }
                            });
                        if sel_b != b.0 {
                            b.0 = sel_b;
                        }
                    });
                }
                MathKind::Differentiate { input } => {
                    let mut sel = input.0.clone();
                    egui::ComboBox::from_label("Input")
                        .selected_text(sel.clone())
                        .show_ui(ui, |ui| {
                            for n in trace_names.iter() {
                                ui.selectable_value(&mut sel, n.0.clone(), n.0.clone());
                            }
                        });
                    if sel != input.0 {
                        input.0 = sel;
                    }
                }
                MathKind::Integrate { input, y0 } => {
                    let mut sel = input.0.clone();
                    egui::ComboBox::from_label("Input")
                        .selected_text(sel.clone())
                        .show_ui(ui, |ui| {
                            for n in trace_names.iter() {
                                ui.selectable_value(&mut sel, n.0.clone(), n.0.clone());
                            }
                        });
                    if sel != input.0 {
                        input.0 = sel;
                    }
                    ui.horizontal(|ui| {
                        ui.label("y0");
                        ui.add(egui::DragValue::new(y0).speed(0.1));
                    });
                }
                MathKind::Filter { input, kind } => {
                    let mut sel = input.0.clone();
                    egui::ComboBox::from_label("Input")
                        .selected_text(sel.clone())
                        .show_ui(ui, |ui| {
                            for n in trace_names.iter() {
                                ui.selectable_value(&mut sel, n.0.clone(), n.0.clone());
                            }
                        });
                    if sel != input.0 {
                        input.0 = sel;
                    }
                    // Map kind to index and editable params
                    let fk = [
                        "Lowpass (1st)",
                        "Highpass (1st)",
                        "Bandpass (1st)",
                        "Biquad LP",
                        "Biquad HP",
                        "Biquad BP",
                    ];
                    let mut which: usize = match kind {
                        FilterKind::Lowpass { .. } => 0,
                        FilterKind::Highpass { .. } => 1,
                        FilterKind::Bandpass { .. } => 2,
                        FilterKind::BiquadLowpass { .. } => 3,
                        FilterKind::BiquadHighpass { .. } => 4,
                        FilterKind::BiquadBandpass { .. } => 5,
                        FilterKind::Custom { .. } => 0,
                    };
                    let (mut f1, mut f2, mut q) = match kind {
                        FilterKind::Lowpass { cutoff_hz } => (*cutoff_hz, 0.0, 0.707),
                        FilterKind::Highpass { cutoff_hz } => (*cutoff_hz, 0.0, 0.707),
                        FilterKind::Bandpass {
                            low_cut_hz,
                            high_cut_hz,
                        } => (*low_cut_hz, *high_cut_hz, 0.707),
                        FilterKind::BiquadLowpass { cutoff_hz, q } => (*cutoff_hz, 0.0, *q),
                        FilterKind::BiquadHighpass { cutoff_hz, q } => (*cutoff_hz, 0.0, *q),
                        FilterKind::BiquadBandpass { center_hz, q } => (*center_hz, 0.0, *q),
                        FilterKind::Custom { params: _ } => (1.0, 0.0, 0.707),
                    };
                    egui::ComboBox::from_label("Filter")
                        .selected_text(fk[which])
                        .show_ui(ui, |ui| {
                            for (i, n) in fk.iter().enumerate() {
                                ui.selectable_value(&mut which, i, *n);
                            }
                        });
                    match which {
                        0 | 1 => {
                            ui.horizontal(|ui| {
                                ui.label("Cutoff Hz");
                                ui.add(egui::DragValue::new(&mut f1).speed(0.1));
                            });
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                ui.label("Low cut Hz");
                                ui.add(egui::DragValue::new(&mut f1).speed(0.1));
                            });
                            ui.horizontal(|ui| {
                                ui.label("High cut Hz");
                                ui.add(egui::DragValue::new(&mut f2).speed(0.1));
                            });
                        }
                        3 | 4 | 5 => {
                            let label = if which == 5 { "Center Hz" } else { "Cutoff Hz" };
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.add(egui::DragValue::new(&mut f1).speed(0.1));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Q");
                                ui.add(egui::DragValue::new(&mut q).speed(0.01));
                            });
                        }
                        _ => {}
                    }
                    // Write back updated kind
                    *kind = match which {
                        0 => FilterKind::Lowpass { cutoff_hz: f1 },
                        1 => FilterKind::Highpass { cutoff_hz: f1 },
                        2 => FilterKind::Bandpass {
                            low_cut_hz: f1,
                            high_cut_hz: f2,
                        },
                        3 => FilterKind::BiquadLowpass { cutoff_hz: f1, q },
                        4 => FilterKind::BiquadHighpass { cutoff_hz: f1, q },
                        5 => FilterKind::BiquadBandpass { center_hz: f1, q },
                        _ => FilterKind::Lowpass { cutoff_hz: f1 },
                    };
                }
                MathKind::MinMax {
                    input,
                    decay_per_sec,
                    mode: _,
                } => {
                    let mut sel = input.0.clone();
                    egui::ComboBox::from_label("Input")
                        .selected_text(sel.clone())
                        .show_ui(ui, |ui| {
                            for n in trace_names.iter() {
                                ui.selectable_value(&mut sel, n.0.clone(), n.0.clone());
                            }
                        });
                    if sel != input.0 {
                        input.0 = sel;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Decay (1/s, 0=none)");
                        let mut decay = decay_per_sec.unwrap_or(0.0);
                        ui.add(egui::DragValue::new(&mut decay).speed(0.1));
                        *decay_per_sec = Some(decay);
                    });
                }
            }

            // Unified Style and Save section
            ui.add_space(8.0);
            egui::CollapsingHeader::new("Style")
                .default_open(false)
                .show(ui, |ui| {
                    if is_editing {
                        if let Some(editing_name) = self.editing.clone() {
                            if let Some(tr) = data.traces.get_trace_mut(&editing_name) {
                                render_trace_look_editor(&mut tr.look, ui, true);
                            } else {
                                ui.label("Trace not found.");
                            }
                        }
                    } else {
                        render_trace_look_editor(&mut self.builder_look, ui, true);
                    }
                });

            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add trace" };
                if ui
                    .add_enabled(
                        !self.builder.name.0.is_empty() && !duplicate_name,
                        egui::Button::new(save_label),
                    )
                    .clicked()
                {
                    // Handle save: builder already holds the full MathTrace
                    let tr = self.builder.clone();
                    if self.error.is_none() {
                        if !is_creating {
                            // Preserve look if renaming and replace in-place to keep position
                            let mut prev_look: Option<TraceLook> = None;
                            let mut replace_idx: Option<usize> = None;

                            if let Some(orig) = self.editing.clone() {
                                replace_idx = self.math_traces.iter().position(|d| d.name == orig);

                                if orig != tr.name {
                                    // Grab previous look, then remove the old backing trace
                                    prev_look = data.traces.get_trace(&orig).map(|t| t.look.clone());
                                    data.remove_trace(&orig);
                                } else {
                                    // Keep current look when not renaming
                                    prev_look = data.traces.get_trace(&orig).map(|t| t.look.clone());
                                }
                            }

                            // Ensure backing trace exists and carry over look/info
                            let trace = data.get_trace_or_new(&tr.name);
                            if let Some(l) = prev_look {
                                trace.look = l;
                            }
                            trace.info = tr.math_formula_string();
                            trace.clear_all();

                            // Replace the math trace at the same index (keep position)
                            if let Some(i) = replace_idx {
                                self.math_traces[i] = tr.clone();
                            } else {
                                // Fallback (shouldn't happen), keep previous behavior
                                self.math_traces.push(tr.clone());
                            }
                        } else {
                            // Creating new
                            let trace = data.get_trace_or_new(&tr.name);
                            trace.look = self.builder_look.clone();
                            trace.info = tr.math_formula_string();
                            self.math_traces.push(tr.clone());
                        }
                        self.editing = None;
                        self.creating = false;
                        self.builder = MathTrace::new(
                            TraceRef::default(),
                            MathKind::Add { inputs: Vec::new() },
                        );
                        self.builder_look = TraceLook::default();
                        self.error = None;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.editing = None;
                        self.creating = false;
                        self.builder = MathTrace::new(
                            TraceRef::default(),
                            MathKind::Add { inputs: Vec::new() },
                        );
                        self.builder_look = TraceLook::default();
                        self.error = None;
                    }
                });
            });
        }
    }
}

// Public helpers for persistence/state management
impl MathPanel {
    pub fn get_math_traces(&self) -> &Vec<crate::data::math::MathTrace> {
        &self.math_traces
    }
    pub fn set_math_traces(&mut self, v: Vec<crate::data::math::MathTrace>) {
        self.math_traces = v;
    }
}
