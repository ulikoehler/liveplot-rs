use crate::data::math::{FilterKind, MathKind, MathTrace, MinMaxMode};
use crate::data::traces::TraceRef;
use eframe::egui;
use egui::{Color32, Ui};
use std::collections::HashMap;

//use super::app::ScopeAppMulti;
use crate::data::{scope::ScopeData, trace_look::TraceLook};
use crate::panels::panel_trait::{Panel, PanelState};
use crate::panels::trace_look_ui::render_trace_look_editor;
//use super::types::MathBuilderState;

#[derive(Debug, Clone)]
pub struct MathPanel {
    pub state: PanelState,
    builder: MathBuilderState,
    pub editing: Option<String>,
    pub error: Option<String>,
    creating: bool,

    math_traces: Vec<MathTrace>,
}

impl Default for MathPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("∫ Math"),
            builder: MathBuilderState::default(),
            editing: None,
            error: None,
            creating: false,

            math_traces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct MathBuilderState {
    pub name: String,
    pub kind_idx: usize,
    pub add_inputs: Vec<(usize, f64)>,
    pub mul_a_idx: usize,
    pub mul_b_idx: usize,
    pub single_idx: usize, // for differentiate/integrate/filter/minmax
    pub integ_y0: f64,
    pub filter_which: usize, // 0 LP,1 HP,2 BP,3 BQLP,4 BQHP,5 BQBP
    pub filter_f1: f64,
    pub filter_f2: f64,
    pub filter_q: f64,
    pub minmax_decay: f64,
    pub look: TraceLook,
}

impl Default for MathBuilderState {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind_idx: 0,
            add_inputs: vec![(0, 1.0), (0, 1.0)],
            mul_a_idx: 0,
            mul_b_idx: 0,
            single_idx: 0,
            integ_y0: 0.0,
            filter_which: 0,
            filter_f1: 1.0,
            filter_f2: 10.0,
            filter_q: 0.707,
            minmax_decay: 0.0,
            look: TraceLook::default(),
        }
    }
}

impl MathBuilderState {
    pub(super) fn from_def(def: &MathTrace, trace_order: &Vec<String>) -> Self {
        //use crate::math::{FilterKind, MathKind, MinMaxMode};
        let mut b = Self::default();
        b.name = def.name.clone();
        match &def.kind {
            MathKind::Add { inputs } => {
                b.kind_idx = 0;
                b.add_inputs = inputs
                    .iter()
                    .map(|(r, g)| {
                        let idx = trace_order.iter().position(|n| n == &r.0).unwrap_or(0);
                        (idx, *g)
                    })
                    .collect();
                if b.add_inputs.is_empty() {
                    b.add_inputs.push((0, 1.0));
                }
            }
            MathKind::Multiply { a, b: bb } => {
                b.kind_idx = 1;
                b.mul_a_idx = trace_order.iter().position(|n| n == &a.0).unwrap_or(0);
                b.mul_b_idx = trace_order.iter().position(|n| n == &bb.0).unwrap_or(0);
            }
            MathKind::Divide { a, b: bb } => {
                b.kind_idx = 2;
                b.mul_a_idx = trace_order.iter().position(|n| n == &a.0).unwrap_or(0);
                b.mul_b_idx = trace_order.iter().position(|n| n == &bb.0).unwrap_or(0);
            }
            MathKind::Differentiate { input } => {
                b.kind_idx = 3;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
            }
            MathKind::Integrate { input, y0 } => {
                b.kind_idx = 4;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                b.integ_y0 = *y0;
            }
            MathKind::Filter { input, kind } => {
                b.kind_idx = 5;
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                match kind {
                    FilterKind::Lowpass { cutoff_hz } => {
                        b.filter_which = 0;
                        b.filter_f1 = *cutoff_hz;
                    }
                    FilterKind::Highpass { cutoff_hz } => {
                        b.filter_which = 1;
                        b.filter_f1 = *cutoff_hz;
                    }
                    FilterKind::Bandpass {
                        low_cut_hz,
                        high_cut_hz,
                    } => {
                        b.filter_which = 2;
                        b.filter_f1 = *low_cut_hz;
                        b.filter_f2 = *high_cut_hz;
                    }
                    FilterKind::BiquadLowpass { cutoff_hz, q } => {
                        b.filter_which = 3;
                        b.filter_f1 = *cutoff_hz;
                        b.filter_q = *q;
                    }
                    FilterKind::BiquadHighpass { cutoff_hz, q } => {
                        b.filter_which = 4;
                        b.filter_f1 = *cutoff_hz;
                        b.filter_q = *q;
                    }
                    FilterKind::BiquadBandpass { center_hz, q } => {
                        b.filter_which = 5;
                        b.filter_f1 = *center_hz;
                        b.filter_q = *q;
                    }
                    FilterKind::Custom { .. } => {
                        b.filter_which = 0;
                    }
                }
            }
            MathKind::MinMax {
                input,
                decay_per_sec,
                mode,
            } => {
                b.kind_idx = if matches!(mode, MinMaxMode::Min) {
                    6
                } else {
                    7
                };
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                b.minmax_decay = decay_per_sec.unwrap_or(0.0);
            }
        }
        b
    }
}

impl Panel for MathPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn update_data(&mut self, _data: &mut ScopeData) {
        let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (name, _) in &_data.traces {
            sources.insert(
                name.clone(),
                _data.get_drawn_points(name.as_str()).unwrap().into(),
            );
        }

        for def in self.math_traces.iter_mut() {
            let out = def.compute_math_trace(sources.clone());

            let paused = _data.is_paused();
            let tr = _data.get_trace_or_new(def.name.as_str());
            if paused {
                tr.snap = Some(out.iter().copied().collect());
            } else {
                tr.live = out.iter().copied().collect();
            }
            sources.insert(def.name.clone(), out);
        }
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut ScopeData) {
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
                    def.reset_math_storage();
                    data.clear_trace(&def.name);
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
                if let Some(tr) = data.traces.get_mut(&def.name) {
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
                    egui::Label::new(def.name.clone())
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if name_resp.hovered() {
                    hover_trace_intern = Some(def.name.clone());
                }
                if name_resp.clicked() {
                    self.builder = MathBuilderState::from_def(def, &data.trace_order);
                    self.editing = Some(def.name.clone());
                    self.error = None;
                    self.creating = false;
                }

                // Info string (formula) - clickable to edit
                let info_text = if let Some(tr) = data.traces.get(&def.name) {
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
                    self.builder = MathBuilderState::from_def(def, &data.trace_order);
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
                            self.builder = MathBuilderState::default();
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
                            def.reset_math_storage();
                            data.clear_trace(&def.name);
                        }
                    }
                });
            });
            if row.response.hovered() {
                hover_trace_intern = Some(def.name.clone());
            }
        }
        if let Some(nm) = hover_trace_intern {
            data.hover_trace = Some(nm);
        }

        // Style popup removed; the editor is part of the new/edit dialog above

        // Full-width New button after the list
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized([ui.available_width(), 24.0], egui::Button::new("New"))
            .on_hover_text("Create a new math trace")
            .clicked();
        if new_clicked {
            self.builder = MathBuilderState::default();
            self.editing = None;
            self.error = None;
            self.creating = true;
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
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.builder.name);
            });
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
            let ir = egui::ComboBox::from_id_salt("math_op")
                .selected_text(kinds[self.builder.kind_idx])
                .show_ui(ui, |ui| {
                    for (i, k) in kinds.iter().enumerate() {
                        ui.selectable_value(&mut self.builder.kind_idx, i, *k);
                    }
                });
            ir.response.on_hover_text("Operation");
            let trace_names: Vec<String> = data.trace_order.clone();

            // Initialize builder look color if blank name changed to a new one (use palette color based on future index)
            // Compute default color index for this potential new trace name
            if is_creating {
                let future_idx = if data.traces.contains_key(&self.builder.name) {
                    // if name exists, keep current
                    None
                } else {
                    Some(data.trace_order.len())
                };
                if let Some(idx) = future_idx {
                    self.builder.look.color = TraceLook::alloc_color(idx);
                }
            }

            match self.builder.kind_idx {
                0 => {
                    // Add/Sub
                    for (idx, (sel, gain)) in self.builder.add_inputs.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(format!("add_sel_{}", idx))
                                .selected_text(trace_names.get(*sel).cloned().unwrap_or_default())
                                .show_ui(ui, |ui| {
                                    for (i, n) in trace_names.iter().enumerate() {
                                        ui.selectable_value(sel, i, n);
                                    }
                                });
                            ui.label("gain");
                            ui.add(egui::DragValue::new(gain).speed(0.1));
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add input").clicked() {
                            self.builder.add_inputs.push((0, 1.0));
                        }
                        if ui.button("Remove input").clicked() {
                            if self.builder.add_inputs.len() > 1 {
                                self.builder.add_inputs.pop();
                            }
                        }
                    });
                }
                1 | 2 => {
                    // Multiply/Divide
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_label("A")
                            .selected_text(
                                trace_names
                                    .get(self.builder.mul_a_idx)
                                    .cloned()
                                    .unwrap_or_default(),
                            )
                            .show_ui(ui, |ui| {
                                for (i, n) in trace_names.iter().enumerate() {
                                    ui.selectable_value(&mut self.builder.mul_a_idx, i, n);
                                }
                            });
                        egui::ComboBox::from_label("B")
                            .selected_text(
                                trace_names
                                    .get(self.builder.mul_b_idx)
                                    .cloned()
                                    .unwrap_or_default(),
                            )
                            .show_ui(ui, |ui| {
                                for (i, n) in trace_names.iter().enumerate() {
                                    ui.selectable_value(&mut self.builder.mul_b_idx, i, n);
                                }
                            });
                    });
                }
                3 => {
                    // Differentiate
                    egui::ComboBox::from_label("Input")
                        .selected_text(
                            trace_names
                                .get(self.builder.single_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut self.builder.single_idx, i, n);
                            }
                        });
                }
                4 => {
                    // Integrate
                    egui::ComboBox::from_label("Input")
                        .selected_text(
                            trace_names
                                .get(self.builder.single_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut self.builder.single_idx, i, n);
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label("y0");
                        ui.add(egui::DragValue::new(&mut self.builder.integ_y0).speed(0.1));
                    });
                }
                5 => {
                    // Filter
                    egui::ComboBox::from_label("Input")
                        .selected_text(
                            trace_names
                                .get(self.builder.single_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut self.builder.single_idx, i, n);
                            }
                        });
                    let fk = [
                        "Lowpass (1st)",
                        "Highpass (1st)",
                        "Bandpass (1st)",
                        "Biquad LP",
                        "Biquad HP",
                        "Biquad BP",
                    ];
                    egui::ComboBox::from_label("Filter")
                        .selected_text(fk[self.builder.filter_which])
                        .show_ui(ui, |ui| {
                            for (i, n) in fk.iter().enumerate() {
                                ui.selectable_value(&mut self.builder.filter_which, i, *n);
                            }
                        });
                    match self.builder.filter_which {
                        0 | 1 => {
                            ui.horizontal(|ui| {
                                ui.label("Cutoff Hz");
                                ui.add(
                                    egui::DragValue::new(&mut self.builder.filter_f1).speed(0.1),
                                );
                            });
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                ui.label("Low cut Hz");
                                ui.add(
                                    egui::DragValue::new(&mut self.builder.filter_f1).speed(0.1),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label("High cut Hz");
                                ui.add(
                                    egui::DragValue::new(&mut self.builder.filter_f2).speed(0.1),
                                );
                            });
                        }
                        3 | 4 | 5 => {
                            let label = match self.builder.filter_which {
                                3 | 4 => "Cutoff Hz",
                                _ => "Center Hz",
                            };
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.add(
                                    egui::DragValue::new(&mut self.builder.filter_f1).speed(0.1),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label("Q");
                                ui.add(
                                    egui::DragValue::new(&mut self.builder.filter_q).speed(0.01),
                                );
                            });
                        }
                        _ => {}
                    }
                }
                6 | 7 => {
                    // Min/Max
                    egui::ComboBox::from_label("Input")
                        .selected_text(
                            trace_names
                                .get(self.builder.single_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut self.builder.single_idx, i, n);
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label("Decay (1/s, 0=none)");
                        ui.add(egui::DragValue::new(&mut self.builder.minmax_decay).speed(0.1));
                    });
                }
                _ => {}
            }

            // Unified Style and Save section
            ui.add_space(8.0);
            egui::CollapsingHeader::new("Style")
                .default_open(false)
                .show(ui, |ui| {
                    if is_editing {
                        if let Some(editing_name) = self.editing.clone() {
                            if let Some(tr) = data.traces.get_mut(&editing_name) {
                                render_trace_look_editor(&mut tr.look, ui, true);
                            } else {
                                ui.label("Trace not found.");
                            }
                        }
                    } else {
                        render_trace_look_editor(&mut self.builder.look, ui, true);
                    }
                });

            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add trace" };
                if ui
                    .add_enabled(!self.builder.name.is_empty(), egui::Button::new(save_label))
                    .clicked()
                {
                    // Handle save logic
                    let mut new_trace: Option<MathTrace> = None;
                    match self.builder.kind_idx {
                        0 => {
                            let inputs = self
                                .builder
                                .add_inputs
                                .iter()
                                .filter_map(|(i, g)| {
                                    trace_names.get(*i).cloned().map(|n| (TraceRef(n), *g))
                                })
                                .collect();
                            if !self.builder.name.is_empty() {
                                new_trace = Some(MathTrace::new(
                                    self.builder.name.clone(),
                                    MathKind::Add { inputs },
                                ));
                            }
                        }
                        1 | 2 => {
                            if let (Some(a), Some(b)) = (
                                trace_names.get(self.builder.mul_a_idx),
                                trace_names.get(self.builder.mul_b_idx),
                            ) {
                                let kind = if self.builder.kind_idx == 1 {
                                    MathKind::Multiply {
                                        a: TraceRef(a.clone()),
                                        b: TraceRef(b.clone()),
                                    }
                                } else {
                                    MathKind::Divide {
                                        a: TraceRef(a.clone()),
                                        b: TraceRef(b.clone()),
                                    }
                                };
                                if !self.builder.name.is_empty() {
                                    new_trace =
                                        Some(MathTrace::new(self.builder.name.clone(), kind));
                                }
                            }
                        }
                        3 => {
                            if let Some(nm) = trace_names.get(self.builder.single_idx) {
                                if !self.builder.name.is_empty() {
                                    new_trace = Some(MathTrace::new(
                                        self.builder.name.clone(),
                                        MathKind::Differentiate {
                                            input: TraceRef(nm.clone()),
                                        },
                                    ));
                                }
                            }
                        }
                        4 => {
                            if let Some(nm) = trace_names.get(self.builder.single_idx) {
                                if !self.builder.name.is_empty() {
                                    new_trace = Some(MathTrace::new(
                                        self.builder.name.clone(),
                                        MathKind::Integrate {
                                            input: TraceRef(nm.clone()),
                                            y0: self.builder.integ_y0,
                                        },
                                    ));
                                }
                            }
                        }
                        5 => {
                            if let Some(nm) = trace_names.get(self.builder.single_idx) {
                                if !self.builder.name.is_empty() {
                                    let kind = match self.builder.filter_which {
                                        0 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::Lowpass {
                                                cutoff_hz: self.builder.filter_f1,
                                            },
                                        },
                                        1 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::Highpass {
                                                cutoff_hz: self.builder.filter_f1,
                                            },
                                        },
                                        2 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::Bandpass {
                                                low_cut_hz: self.builder.filter_f1,
                                                high_cut_hz: self.builder.filter_f2,
                                            },
                                        },
                                        3 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::BiquadLowpass {
                                                cutoff_hz: self.builder.filter_f1,
                                                q: self.builder.filter_q,
                                            },
                                        },
                                        4 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::BiquadHighpass {
                                                cutoff_hz: self.builder.filter_f1,
                                                q: self.builder.filter_q,
                                            },
                                        },
                                        5 => MathKind::Filter {
                                            input: TraceRef(nm.clone()),
                                            kind: FilterKind::BiquadBandpass {
                                                center_hz: self.builder.filter_f1,
                                                q: self.builder.filter_q,
                                            },
                                        },
                                        _ => return,
                                    };
                                    new_trace =
                                        Some(MathTrace::new(self.builder.name.clone(), kind));
                                }
                            }
                        }
                        6 | 7 => {
                            if let Some(nm) = trace_names.get(self.builder.single_idx) {
                                if !self.builder.name.is_empty() {
                                    new_trace = Some(MathTrace::new(
                                        self.builder.name.clone(),
                                        MathKind::MinMax {
                                            input: TraceRef(nm.clone()),
                                            decay_per_sec: Some(self.builder.minmax_decay),
                                            mode: if self.builder.kind_idx == 6 {
                                                MinMaxMode::Min
                                            } else {
                                                MinMaxMode::Max
                                            },
                                        },
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }

                    // if self.error.is_none() {
                    //     if let Some(tr) = data.traces.get_mut(&self.builder.name) {
                    //         if is_creating {
                    //             tr.look = self.builder.look.clone();
                    //         }
                    //     }
                    //     self.creating = false;
                    // }
                    if self.error.is_none() {
                        if let Some(tr) = new_trace {
                            if !is_creating {
                                if let Some(tr) = data.traces.get_mut(&self.builder.name) {
                                    self.builder.look = tr.look.clone();
                                }
                                data.remove_trace(self.editing.as_ref().unwrap());
                            }
                            let trace = data.get_trace_or_new(&self.builder.name);

                            trace.look = self.builder.look.clone();
                            trace.info = tr.math_formula_string();

                            if is_creating {
                                self.math_traces.push(tr.clone());
                            } else if let Some(editing_name) = self.editing.clone() {
                                // Replace existing
                                self.math_traces.retain(|d| d.name != editing_name);
                                self.math_traces.push(tr.clone());
                            }
                            self.editing = None;
                            self.builder = MathBuilderState::default();
                            self.error = None;
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.editing = None;
                        self.creating = false;
                        self.builder = MathBuilderState::default();
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
