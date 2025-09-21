use eframe::egui;
use egui::Color32;

use crate::math::{MathTraceDef, MathKind, FilterKind, TraceRef, MinMaxMode};

use super::app::ScopeAppMulti;
use super::types::MathBuilderState;

impl MathBuilderState {
    pub(super) fn from_def(def: &MathTraceDef, trace_order: &Vec<String>) -> Self {
        use crate::math::{FilterKind, MathKind, MinMaxMode};
        let mut b = Self::default();
        b.name = def.name.clone();
        match &def.kind {
            MathKind::Add { inputs } => {
                b.kind_idx = 0;
                b.add_inputs = inputs.iter().map(|(r, g)| {
                    let idx = trace_order.iter().position(|n| n == &r.0).unwrap_or(0);
                    (idx, *g)
                }).collect();
                if b.add_inputs.is_empty() { b.add_inputs.push((0, 1.0)); }
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
                    FilterKind::Lowpass { cutoff_hz } => { b.filter_which = 0; b.filter_f1 = *cutoff_hz; }
                    FilterKind::Highpass { cutoff_hz } => { b.filter_which = 1; b.filter_f1 = *cutoff_hz; }
                    FilterKind::Bandpass { low_cut_hz, high_cut_hz } => { b.filter_which = 2; b.filter_f1 = *low_cut_hz; b.filter_f2 = *high_cut_hz; }
                    FilterKind::BiquadLowpass { cutoff_hz, q } => { b.filter_which = 3; b.filter_f1 = *cutoff_hz; b.filter_q = *q; }
                    FilterKind::BiquadHighpass { cutoff_hz, q } => { b.filter_which = 4; b.filter_f1 = *cutoff_hz; b.filter_q = *q; }
                    FilterKind::BiquadBandpass { center_hz, q } => { b.filter_which = 5; b.filter_f1 = *center_hz; b.filter_q = *q; }
                    FilterKind::Custom { .. } => { b.filter_which = 0; }
                }
            }
            MathKind::MinMax { input, decay_per_sec, mode } => {
                b.kind_idx = if matches!(mode, MinMaxMode::Min) { 6 } else { 7 };
                b.single_idx = trace_order.iter().position(|n| n == &input.0).unwrap_or(0);
                b.minmax_decay = decay_per_sec.unwrap_or(0.0);
            }
        }
        b
    }
}

pub(super) fn show_math_dialog(app: &mut ScopeAppMulti, ctx: &egui::Context) {
    let mut show_flag = app.show_math_dialog;
    egui::Window::new("Math traces").open(&mut show_flag).show(ctx, |ui| {
        ui.label("Create virtual traces from existing ones.");
        if let Some(err) = &app.math_error { ui.colored_label(Color32::LIGHT_RED, err); }
        ui.separator();
        // Existing math traces list with remove button
        for def in app.math_defs.clone().iter() {
            ui.horizontal(|ui| {
                ui.label(format!("{}: {:?}", def.name, def.kind));
                if ui.button("Edit").clicked() {
                    // initialize builder from existing def
                    app.math_builder = MathBuilderState::from_def(def, &app.trace_order);
                    app.math_editing = Some(def.name.clone());
                }
                if ui.button("Remove").clicked() {
                    app.remove_math_trace_internal(&def.name);
                }
            });
        }
        ui.separator();
        let editing = app.math_editing.clone();
        let is_editing = editing.is_some();
        let header = if is_editing { "Edit" } else { "Add new" };
        ui.collapsing(header, |ui| {
            let kinds = ["Add/Subtract", "Multiply", "Divide", "Differentiate", "Integrate", "Filter", "Min", "Max"];
            egui::ComboBox::from_label("Operation").selected_text(kinds[app.math_builder.kind_idx]).show_ui(ui, |ui| {
                for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut app.math_builder.kind_idx, i, *k); }
            });
            ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut app.math_builder.name); });
            let trace_names: Vec<String> = app.trace_order.clone();
            match app.math_builder.kind_idx {
                0 => { // Add/Sub
                    for (idx, (sel, gain)) in app.math_builder.add_inputs.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(format!("add_sel_{}", idx))
                                .selected_text(trace_names.get(*sel).cloned().unwrap_or_default())
                                .show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(sel, i, n); } });
                            ui.label("gain"); ui.add(egui::DragValue::new(gain).speed(0.1));
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add input").clicked() { app.math_builder.add_inputs.push((0, 1.0)); }
                        if ui.button("Remove input").clicked() { if app.math_builder.add_inputs.len() > 1 { app.math_builder.add_inputs.pop(); } }
                    });
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        let inputs = app.math_builder.add_inputs.iter().filter_map(|(i, g)| trace_names.get(*i).cloned().map(|n| (TraceRef(n), *g))).collect();
                        if !app.math_builder.name.is_empty() {
                            let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind: MathKind::Add { inputs } };
                            app.apply_add_or_edit(def);
                        }
                    }
                }
                1 | 2 => { // Multiply/Divide
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_label("A").selected_text(trace_names.get(app.math_builder.mul_a_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.mul_a_idx, i, n); } });
                        egui::ComboBox::from_label("B").selected_text(trace_names.get(app.math_builder.mul_b_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.mul_b_idx, i, n); } });
                    });
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        if let (Some(a), Some(b)) = (trace_names.get(app.math_builder.mul_a_idx), trace_names.get(app.math_builder.mul_b_idx)) {
                            let kind = if app.math_builder.kind_idx == 1 { MathKind::Multiply { a: TraceRef(a.clone()), b: TraceRef(b.clone()) } } else { MathKind::Divide { a: TraceRef(a.clone()), b: TraceRef(b.clone()) } };
                            if !app.math_builder.name.is_empty() { let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind }; app.apply_add_or_edit(def); }
                        }
                    }
                }
                3 => { // Differentiate
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(app.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.single_idx, i, n); } });
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        if let Some(nm) = trace_names.get(app.math_builder.single_idx) { if !app.math_builder.name.is_empty() { let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind: MathKind::Differentiate { input: TraceRef(nm.clone()) } }; app.apply_add_or_edit(def); } }
                    }
                }
                4 => { // Integrate
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(app.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.single_idx, i, n); } });
                    ui.horizontal(|ui| { ui.label("y0"); ui.add(egui::DragValue::new(&mut app.math_builder.integ_y0).speed(0.1)); });
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        if let Some(nm) = trace_names.get(app.math_builder.single_idx) { if !app.math_builder.name.is_empty() { let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind: MathKind::Integrate { input: TraceRef(nm.clone()), y0: app.math_builder.integ_y0 } }; app.apply_add_or_edit(def); } }
                    }
                }
                5 => { // Filter
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(app.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.single_idx, i, n); } });
                    let fk = ["Lowpass (1st)", "Highpass (1st)", "Bandpass (1st)", "Biquad LP", "Biquad HP", "Biquad BP"];
                    egui::ComboBox::from_label("Filter").selected_text(fk[app.math_builder.filter_which]).show_ui(ui, |ui| { for (i, n) in fk.iter().enumerate() { ui.selectable_value(&mut app.math_builder.filter_which, i, *n); } });
                    match app.math_builder.filter_which {
                        0 | 1 => { ui.horizontal(|ui| { ui.label("Cutoff Hz"); ui.add(egui::DragValue::new(&mut app.math_builder.filter_f1).speed(0.1)); }); },
                        2 => { ui.horizontal(|ui| { ui.label("Low cut Hz"); ui.add(egui::DragValue::new(&mut app.math_builder.filter_f1).speed(0.1)); }); ui.horizontal(|ui| { ui.label("High cut Hz"); ui.add(egui::DragValue::new(&mut app.math_builder.filter_f2).speed(0.1)); }); },
                        3 | 4 | 5 => {
                            let label = match app.math_builder.filter_which { 3 | 4 => "Cutoff Hz", _ => "Center Hz" };
                            ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(&mut app.math_builder.filter_f1).speed(0.1)); });
                            ui.horizontal(|ui| { ui.label("Q"); ui.add(egui::DragValue::new(&mut app.math_builder.filter_q).speed(0.01)); });
                        }
                        _ => {}
                    }
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        if let Some(nm) = trace_names.get(app.math_builder.single_idx) { if !app.math_builder.name.is_empty() {
                            let kind = match app.math_builder.filter_which {
                                0 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Lowpass { cutoff_hz: app.math_builder.filter_f1 } },
                                1 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Highpass { cutoff_hz: app.math_builder.filter_f1 } },
                                2 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::Bandpass { low_cut_hz: app.math_builder.filter_f1, high_cut_hz: app.math_builder.filter_f2 } },
                                3 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadLowpass { cutoff_hz: app.math_builder.filter_f1, q: app.math_builder.filter_q } },
                                4 => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadHighpass { cutoff_hz: app.math_builder.filter_f1, q: app.math_builder.filter_q } },
                                _ => MathKind::Filter { input: TraceRef(nm.clone()), kind: FilterKind::BiquadBandpass { center_hz: app.math_builder.filter_f1, q: app.math_builder.filter_q } },
                            };
                            let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind }; app.apply_add_or_edit(def);
                        } }
                    }
                }
                6 | 7 => { // Min/Max
                    egui::ComboBox::from_label("Input").selected_text(trace_names.get(app.math_builder.single_idx).cloned().unwrap_or_default()).show_ui(ui, |ui| { for (i, n) in trace_names.iter().enumerate() { ui.selectable_value(&mut app.math_builder.single_idx, i, n); } });
                    ui.horizontal(|ui| { ui.label("Decay (1/s, 0=none)"); ui.add(egui::DragValue::new(&mut app.math_builder.minmax_decay).speed(0.1)); });
                    if ui.button(if is_editing { "Save" } else { "Add trace" }).clicked() {
                        if let Some(nm) = trace_names.get(app.math_builder.single_idx) { if !app.math_builder.name.is_empty() { let mode = if app.math_builder.kind_idx == 6 { MinMaxMode::Min } else { MinMaxMode::Max }; let decay_opt = if app.math_builder.minmax_decay > 0.0 { Some(app.math_builder.minmax_decay) } else { None }; let def = MathTraceDef { name: app.math_builder.name.clone(), color_hint: None, kind: MathKind::MinMax { input: TraceRef(nm.clone()), decay_per_sec: decay_opt, mode } }; app.apply_add_or_edit(def); } }
                    }
                }
                _ => {}
            }
            if is_editing {
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { app.math_editing = None; app.math_builder = MathBuilderState::default(); app.math_error = None; }
                });
            }
        });
    });
    app.show_math_dialog = show_flag;
}
