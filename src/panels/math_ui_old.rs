use egui::Ui;
use super::panel_trait::{Panel, PanelState};
use crate::data::DataContext;
use crate::data::math as m;
use crate::data::trace_look::TraceLook;
use super::trace_look_ui::render_trace_look_editor;

// All UI state lives directly on MathPanel

pub struct MathPanel {
    pub state: PanelState,
    // UI builder settings
    math_name: String,
    kind_idx: usize,
    look: TraceLook,
    // Add
    add_inputs: Vec<(usize, f64)>,
    // Mul/Div
    mul_a_idx: usize,
    mul_b_idx: usize,
    // Single input ops
    single_idx: usize,
    // Integrator
    integ_y0: f64,
    // Filters
    filter_which: usize,
    filter_f1: f64,
    filter_f2: f64,
    filter_q: f64,
    // Min/Max
    minmax_decay: f64,
    editing: Option<String>,
    creating: bool,
}
impl Default for MathPanel {
    fn default() -> Self {
        Self {
            state: PanelState { visible: false, detached: false },
            math_name: String::new(),
            kind_idx: 0,
            look: TraceLook::default(),
            add_inputs: Vec::new(),
            mul_a_idx: 0,
            mul_b_idx: 0,
            single_idx: 0,
            integ_y0: 0.0,
            filter_which: 0,
            filter_f1: 0.0,
            filter_f2: 0.0,
            filter_q: 0.707,
            minmax_decay: 0.0,
            editing: None,
            creating: false,
        }
    }
}
impl Panel for MathPanel {
    fn name(&self) -> &'static str { "Math" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui, data: &mut DataContext) {
        ui.label("Create virtual traces from existing ones.");
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Reset All Storage").on_hover_text("Reset integrators, filters, min/max for all math traces").clicked() { data.math.reset_all_storage(); }
        });
        ui.add_space(6.0);

        // Existing math traces list
        for def in data.math.defs.clone() {
            ui.horizontal(|ui| {
                // Color editor
                if let Some(tr) = data.traces.traces.get_mut(&def.name) {
                    let mut c = tr.look.color;
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        tr.look.color = c;
                        // Keep style panel in sync when editing this trace
                        if self.editing.as_deref() == Some(def.name.as_str()) {
                            self.look.color = c;
                        }
                    }
                } else { ui.label(""); }

                // Name (edit on click)
                let name_clicked = ui.label(def.name.clone()).clicked();
                if name_clicked {
                    self.builder_from_def(&def, &data.traces.trace_order, &*data);
                    self.editing = Some(def.name.clone());
                    self.creating = false;
                }

                // Info
                let info_text = data.traces.traces.get(&def.name).map(|t| t.info.clone()).unwrap_or_default();
                let info_clicked = ui.label(info_text).clicked();
                if info_clicked {
                    self.builder_from_def(&def, &data.traces.trace_order, &*data);
                    self.editing = Some(def.name.clone());
                    self.creating = false;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Remove
                    if ui.button("Remove").clicked() {
                        let removing = def.name.clone();
                        data.math.remove_def(&removing);
                        if self.editing.as_deref() == Some(removing.as_str()) {
                            self.editing = None;
                            self.creating = false;
                            self.reset_ui();
                        }
                    }
                    // Reset if stateful
                    let is_stateful = matches!(def.kind, m::MathKind::Integrate { .. } | m::MathKind::Filter { .. } | m::MathKind::MinMax { .. });
                    if is_stateful { if ui.button("Reset").on_hover_text("Reset integrator/filter/min/max state for this trace").clicked() { data.math.reset_storage(&def.name); } }
                });
            });
        }

        ui.add_space(6.0);
        let new_clicked = ui.add_sized([ui.available_width(), 24.0], egui::Button::new("New")).clicked();
        if new_clicked {
            self.reset_ui();
            // For new math traces, start with two inputs for Add/Sub by default
            self.add_inputs = vec![(0, 1.0), (0, 1.0)];
            self.editing = None;
            self.creating = true;
        }

        let is_editing = self.editing.is_some();
        let is_creating = self.creating;
        if is_editing || is_creating {
            ui.add_space(12.0);
            ui.separator();
            ui.strong(if is_editing { "Edit math trace" } else { "New math trace" });
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.math_name); });
            let kinds = ["Add/Subtract","Multiply","Divide","Differentiate","Integrate","Filter","Min","Max"]; 
            let ir = egui::ComboBox::from_id_salt("math_op").selected_text(kinds[self.kind_idx]).show_ui(ui, |ui| { for (i,k) in kinds.iter().enumerate() { ui.selectable_value(&mut self.kind_idx, i, *k); } });
            ir.response.on_hover_text("Operation");
            let trace_names = data.traces.trace_order.clone();

            match self.kind_idx {
                0 => { // Add/Sub
                    // Rule: Minimum inputs should be one. For new traces, default to two inputs.
                    if is_creating {
                        while self.add_inputs.len() < 2 { self.add_inputs.push((0, 1.0)); }
                    } else if self.add_inputs.is_empty() {
                        self.add_inputs.push((0, 1.0));
                    }
                    for (idx, (sel, gain)) in self.add_inputs.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(format!("add_sel_{}", idx)).selected_text(trace_names.get(*sel).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(sel, i, n); } });
                            ui.label("gain"); ui.add(egui::DragValue::new(gain).speed(0.1));
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add input").clicked() { self.add_inputs.push((0,1.0)); }
                        // Minimum is one input; allow removing down to 1
                        if ui.button("Remove input").clicked() { if self.add_inputs.len() > 1 { self.add_inputs.pop(); } }
                    });
                }
                1 | 2 => { // Multiply/Divide
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_label("A").selected_text(trace_names.get(self.mul_a_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.mul_a_idx, i, n); } });
                        egui::ComboBox::from_label("B").selected_text(trace_names.get(self.mul_b_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.mul_b_idx, i, n); } });
                    });
                }
                3 => { // Differentiate
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.single_idx, i, n); } });
                }
                4 => { // Integrate
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.single_idx, i, n); } });
                    ui.horizontal(|ui| { ui.label("y0"); ui.add(egui::DragValue::new(&mut self.integ_y0).speed(0.1)); });
                }
                5 => { // Filter
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.single_idx, i, n); } });
                    let kinds = ["1st Lowpass","1st Highpass","1st Bandpass","Biquad Lowpass","Biquad Highpass","Biquad Bandpass"]; 
                    egui::ComboBox::from_id_salt("filter_kind").selected_text(kinds[self.filter_which]).show_ui(ui, |ui| { for (i,k) in kinds.iter().enumerate(){ ui.selectable_value(&mut self.filter_which, i, *k); } });
                    match self.filter_which {
                        0 => { ui.horizontal(|ui| { ui.label("cutoff (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); }); }
                        1 => { ui.horizontal(|ui| { ui.label("cutoff (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); }); }
                        2 => { ui.horizontal(|ui| { ui.label("low (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); ui.label("high (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f2).speed(0.1)); }); }
                        3 => { ui.horizontal(|ui| { ui.label("cutoff (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); ui.label("Q"); ui.add(egui::DragValue::new(&mut self.filter_q).speed(0.1)); }); }
                        4 => { ui.horizontal(|ui| { ui.label("cutoff (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); ui.label("Q"); ui.add(egui::DragValue::new(&mut self.filter_q).speed(0.1)); }); }
                        5 => { ui.horizontal(|ui| { ui.label("center (Hz)"); ui.add(egui::DragValue::new(&mut self.filter_f1).speed(0.1)); ui.label("Q"); ui.add(egui::DragValue::new(&mut self.filter_q).speed(0.1)); }); }
                        _ => {}
                    }
                }
                6 | 7 => { // Min / Max
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(self.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i,n) in trace_names.iter().enumerate(){ ui.selectable_value(&mut self.single_idx, i, n); } });
                    ui.horizontal(|ui| { ui.label("decay (1/s)"); ui.add(egui::DragValue::new(&mut self.minmax_decay).speed(0.01)); });
                }
                _ => {}
            }

            ui.add_space(5.0);
            egui::CollapsingHeader::new("Style").default_open(false).show(ui, |ui| {
                render_trace_look_editor(&mut self.look, ui, true);
            });
            // While editing, mirror builder look into the live trace so list row color stays in sync
            if let Some(edit_name) = self.editing.clone() {
                if let Some(tr) = data.traces.traces.get_mut(&edit_name) {
                    tr.look = self.look.clone();
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let save_label = if is_editing { "Save" } else { "Add trace" };
                let save_clicked = ui.button(save_label).clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() { self.editing=None; self.creating=false; self.reset_ui(); }
                });
                if save_clicked {
                    if !self.math_name.is_empty() {
                        let def = self.build_def(&trace_names);
                        if let Some(def) = def {
                            if is_editing { data.math.remove_def(self.editing.as_ref().unwrap()); }
                            let name = def.name.clone();
                            data.math.add_def(def);
                            // Apply chosen style to the corresponding trace (create if needed)
                            let entry = data.traces.traces.entry(name.clone()).or_insert_with(|| {
                                data.traces.trace_order.push(name.clone());
                                crate::data::traces::TraceState { name: name.clone(), look: self.look.clone(), offset: 0.0, live: Default::default(), snap: None, info: String::new() }
                            });
                            entry.look = self.look.clone();
                            self.editing=None; self.creating=false; self.reset_ui();
                        }
                    }
                }
            });
        }
    }
}

impl MathPanel {
    fn builder_from_def(&mut self, def: &m::MathTraceDef, order: &Vec<String>, data: &DataContext) {
        self.reset_ui();
        self.math_name = def.name.clone();
        match &def.kind {
            m::MathKind::Add { inputs } => { self.kind_idx=0; self.add_inputs = inputs.iter().map(|(r,g)| { let idx=order.iter().position(|n| n==&r.0).unwrap_or(0); (idx,*g) }).collect(); if self.add_inputs.is_empty(){ self.add_inputs.push((0,1.0)); } }
            m::MathKind::Multiply { a, b: bb } => { self.kind_idx=1; self.mul_a_idx = order.iter().position(|n| n==&a.0).unwrap_or(0); self.mul_b_idx = order.iter().position(|n| n==&bb.0).unwrap_or(0); }
            m::MathKind::Divide { a, b: bb } => { self.kind_idx=2; self.mul_a_idx = order.iter().position(|n| n==&a.0).unwrap_or(0); self.mul_b_idx = order.iter().position(|n| n==&bb.0).unwrap_or(0); }
            m::MathKind::Differentiate { input } => { self.kind_idx=3; self.single_idx = order.iter().position(|n| n==&input.0).unwrap_or(0); }
            m::MathKind::Integrate { input, y0 } => { self.kind_idx=4; self.single_idx = order.iter().position(|n| n==&input.0).unwrap_or(0); self.integ_y0 = *y0; }
            m::MathKind::Filter { input, kind } => { self.kind_idx=5; self.single_idx = order.iter().position(|n| n==&input.0).unwrap_or(0); match kind {
                m::FilterKind::Lowpass { cutoff_hz } => { self.filter_which=0; self.filter_f1=*cutoff_hz; }
                m::FilterKind::Highpass { cutoff_hz } => { self.filter_which=1; self.filter_f1=*cutoff_hz; }
                m::FilterKind::Bandpass { low_cut_hz, high_cut_hz } => { self.filter_which=2; self.filter_f1=*low_cut_hz; self.filter_f2=*high_cut_hz; }
                m::FilterKind::BiquadLowpass { cutoff_hz, q } => { self.filter_which=3; self.filter_f1=*cutoff_hz; self.filter_q=*q; }
                m::FilterKind::BiquadHighpass { cutoff_hz, q } => { self.filter_which=4; self.filter_f1=*cutoff_hz; self.filter_q=*q; }
                m::FilterKind::BiquadBandpass { center_hz, q } => { self.filter_which=5; self.filter_f1=*center_hz; self.filter_q=*q; }
                m::FilterKind::Custom { .. } => { self.filter_which=0; }
            } }
            m::MathKind::MinMax { input, decay_per_sec, mode } => { self.kind_idx = if matches!(mode, m::MinMaxMode::Min) {6} else {7}; self.single_idx = order.iter().position(|n| n==&input.0).unwrap_or(0); self.minmax_decay = decay_per_sec.unwrap_or(0.0); }
        }
        // Initialize look from existing trace if present
        if let Some(tr) = data.traces.traces.get(&def.name) { self.look = tr.look.clone(); }
    }

    fn build_def(&self, names: &Vec<String>) -> Option<m::MathTraceDef> {
        match self.kind_idx {
            0 => { // Add/Sub
                let inputs = self.add_inputs.iter().filter_map(|(i,g)| names.get(*i).cloned().map(|n| (m::TraceRef(n), *g))).collect();
                Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::Add { inputs } })
            }
            1 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::Multiply { a: m::TraceRef(names.get(self.mul_a_idx)?.clone()), b: m::TraceRef(names.get(self.mul_b_idx)?.clone()) } }) }
            2 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::Divide { a: m::TraceRef(names.get(self.mul_a_idx)?.clone()), b: m::TraceRef(names.get(self.mul_b_idx)?.clone()) } }) }
            3 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::Differentiate { input: m::TraceRef(names.get(self.single_idx)?.clone()) } }) }
            4 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::Integrate { input: m::TraceRef(names.get(self.single_idx)?.clone()), y0: self.integ_y0 } }) }
            5 => { let input = m::TraceRef(names.get(self.single_idx)?.clone()); let kind = match self.filter_which { 0=>m::FilterKind::Lowpass{ cutoff_hz:self.filter_f1 }, 1=>m::FilterKind::Highpass{ cutoff_hz:self.filter_f1 }, 2=>m::FilterKind::Bandpass{ low_cut_hz:self.filter_f1, high_cut_hz:self.filter_f2 }, 3=>m::FilterKind::BiquadLowpass{ cutoff_hz:self.filter_f1, q:self.filter_q }, 4=>m::FilterKind::BiquadHighpass{ cutoff_hz:self.filter_f1, q:self.filter_q }, 5=>m::FilterKind::BiquadBandpass{ center_hz:self.filter_f1, q:self.filter_q }, _=>m::FilterKind::Lowpass{cutoff_hz:self.filter_f1} }; Some(m::MathTraceDef { name:self.math_name.clone(), color_hint: None, kind: m::MathKind::Filter { input, kind } }) }
            6 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::MinMax { input: m::TraceRef(names.get(self.single_idx)?.clone()), decay_per_sec: Some(self.minmax_decay), mode: m::MinMaxMode::Min } }) }
            7 => { Some(m::MathTraceDef { name: self.math_name.clone(), color_hint: None, kind: m::MathKind::MinMax { input: m::TraceRef(names.get(self.single_idx)?.clone()), decay_per_sec: Some(self.minmax_decay), mode: m::MinMaxMode::Max } }) }
            _ => None,
        }
    }

    fn reset_ui(&mut self) {
        self.math_name.clear();
        self.kind_idx = 0;
        self.look = TraceLook::default();
        self.add_inputs.clear();
        self.mul_a_idx = 0;
        self.mul_b_idx = 0;
        self.single_idx = 0;
        self.integ_y0 = 0.0;
        self.filter_which = 0;
        self.filter_f1 = 0.0;
        self.filter_f2 = 0.0;
        self.filter_q = 0.707;
        self.minmax_decay = 0.0;
    }
}
