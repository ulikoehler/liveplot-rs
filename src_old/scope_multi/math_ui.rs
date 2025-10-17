use eframe::egui;
use egui::Color32;

use crate::math::{FilterKind, MathKind, MathTraceDef, MinMaxMode, TraceRef};

use super::app::ScopeAppMulti;
use super::panel::{DockPanel, DockState};
use super::types::MathBuilderState;

#[derive(Debug, Clone)]
pub struct MathPanel {
    pub dock: DockState,
    pub builder: MathBuilderState,
    pub editing: Option<String>,
    pub error: Option<String>,
    pub creating: bool,
}

impl Default for MathPanel {
    fn default() -> Self {
        Self {
            dock: DockState::new("Math"),
            builder: MathBuilderState::default(),
            editing: None,
            error: None,
            creating: false,
        }
    }
}

impl DockPanel for MathPanel {
    fn dock_mut(&mut self) -> &mut DockState { &mut self.dock }
    fn panel_contents(&mut self, app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
        math_panel_contents(app, ui);
    }
}

impl MathBuilderState {
    pub(super) fn from_def(def: &MathTraceDef, trace_order: &Vec<String>) -> Self {
        use crate::math::{FilterKind, MathKind, MinMaxMode};
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

pub(super) fn math_panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
    ui.label("Create virtual traces from existing ones.");
    if let Some(err) = &app.math_panel.error {
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
            app.reset_all_math_storage();
        }
    });
    ui.add_space(6.0);
    // Existing math traces list with color editor, name, info, and Remove (right-aligned)
    // Reset hover before drawing; rows will set it when hovered
    app.hover_trace = None;
    for def in app.math_defs.clone().iter() {
        let row = ui.horizontal(|ui| {
            // Color editor like in traces_ui
            if let Some(tr) = app.traces.get_mut(&def.name) {
                let mut c = tr.look.color;
                let resp = ui
                    .color_edit_button_srgba(&mut c)
                    .on_hover_text("Change trace color");
                if resp.hovered() {
                    app.hover_trace = Some(def.name.clone());
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
                app.hover_trace = Some(def.name.clone());
            }
            if name_resp.clicked() {
                app.math_panel.builder = MathBuilderState::from_def(def, &app.trace_order);
                app.math_panel.editing = Some(def.name.clone());
                app.math_panel.error = None;
                app.math_panel.creating = false;
            }

            // Info string (formula) - clickable to edit
            let info_text = if let Some(tr) = app.traces.get(&def.name) {
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
                app.hover_trace = Some(def.name.clone());
            }
            if info_resp.clicked() {
                app.math_panel.builder = MathBuilderState::from_def(def, &app.trace_order);
                app.math_panel.editing = Some(def.name.clone());
                app.math_panel.error = None;
                app.math_panel.creating = false;
            }
            // Right-aligned per-trace actions: Reset and Remove
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Remove button with hover highlight
                let remove_resp = ui.button("Remove");
                if remove_resp.hovered() {
                    app.hover_trace = Some(def.name.clone());
                }
                    if remove_resp.clicked() {
                    let removing = def.name.clone();
                    app.remove_math_trace_internal(&removing);
                        if app.math_panel.editing.as_deref() == Some(&removing) {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                    }
                }
                // Show Reset for kinds that have internal storage
                let is_stateful = matches!(
                    def.kind,
                    MathKind::Integrate { .. } | MathKind::Filter { .. } | MathKind::MinMax { .. }
                );
                if is_stateful {
                    let reset_resp = ui
                        .button("Reset")
                        .on_hover_text("Reset integrator/filter/min/max state for this trace");
                    if reset_resp.hovered() {
                        app.hover_trace = Some(def.name.clone());
                    }
                    if reset_resp.clicked() {
                        let nm = def.name.clone();
                        app.reset_math_storage(&nm);
                    }
                }
            });
        });
        if row.response.hovered() {
            app.hover_trace = Some(def.name.clone());
        }
    }

    // Style popup removed; the editor is part of the new/edit dialog above

    // Full-width New button after the list
    ui.add_space(6.0);
    let new_clicked = ui
        .add_sized([ui.available_width(), 24.0], egui::Button::new("New"))
        .on_hover_text("Create a new math trace")
        .clicked();
    if new_clicked {
        app.math_panel.builder = MathBuilderState::default();
        app.math_panel.editing = None;
        app.math_panel.error = None;
        app.math_panel.creating = true;
    }

    // Settings panel (hidden unless creating or editing)
    let is_editing = app.math_panel.editing.is_some();
    let is_creating = app.math_panel.creating;
    if is_editing || is_creating {
        ui.add_space(12.0);
        ui.separator();
        if is_editing {
            ui.strong("Edit math trace");
        } else {
            ui.strong("New math trace");
        }
        // Name first, then Operation (no label; tooltip on combobox)
        ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut app.math_panel.builder.name); });
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
            .selected_text(kinds[app.math_panel.builder.kind_idx])
            .show_ui(ui, |ui| {
                for (i, k) in kinds.iter().enumerate() { ui.selectable_value(&mut app.math_panel.builder.kind_idx, i, *k); }
            });
        ir.response.on_hover_text("Operation");
        let trace_names: Vec<String> = app.trace_order.clone();

        // Initialize builder look color if blank name changed to a new one (use palette color based on future index)
        // Compute default color index for this potential new trace name
        if is_creating {
            let future_idx = if app.traces.contains_key(&app.math_panel.builder.name) {
                // if name exists, keep current
                None
            } else if app.math_panel.builder.name.is_empty() {
                None
            } else {
                Some(app.trace_order.len())
            };
            if let Some(idx) = future_idx {
                // Set default color only if look color is still default white to avoid clobbering user choice
                if app.math_panel.builder.look.color == egui::Color32::WHITE {
                    // Reuse the same palette as alloc_color
                    const PALETTE: [egui::Color32; 10] = [
                        egui::Color32::LIGHT_BLUE,
                        egui::Color32::LIGHT_RED,
                        egui::Color32::LIGHT_GREEN,
                        egui::Color32::GOLD,
                        egui::Color32::from_rgb(0xAA, 0x55, 0xFF),
                        egui::Color32::from_rgb(0xFF, 0xAA, 0x00),
                        egui::Color32::from_rgb(0x00, 0xDD, 0xDD),
                        egui::Color32::from_rgb(0xDD, 0x00, 0xDD),
                        egui::Color32::from_rgb(0x66, 0xCC, 0x66),
                        egui::Color32::from_rgb(0xCC, 0x66, 0x66),
                    ];
                    app.math_panel.builder.look.color = PALETTE[idx % PALETTE.len()];
                }
            }
        }

        // (Style editor moved to appear just before the Add/Save button per kind)

        match app.math_panel.builder.kind_idx {
            0 => {
                // Add/Sub
                for (idx, (sel, gain)) in app.math_panel.builder.add_inputs.iter_mut().enumerate() {
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
                    if ui.button("Add input").clicked() { app.math_panel.builder.add_inputs.push((0, 1.0)); }
                    if ui.button("Remove input").clicked() { if app.math_panel.builder.add_inputs.len() > 1 { app.math_panel.builder.add_inputs.pop(); } }
                });
                // Style editor just before Save/Add
                ui.add_space(5.0);
                egui::CollapsingHeader::new("Style")
                    .default_open(false)
                    .show(ui, |ui| {
                        if is_editing {
                            if let Some(editing_name) = app.math_panel.editing.clone() {
                                if let Some(tr) = app.traces.get_mut(&editing_name) {
                                    tr.look.render_editor(ui, true, None, false, None);
                                } else {
                                    ui.label("Trace not found.");
                                }
                            }
                        } else {
                            app.math_panel.builder.look.render_editor(ui, true, None, false, None);
                        }
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let save_clicked = ui.button(save_label).clicked();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        let inputs = app
                            .math_panel
                            .builder
                            .add_inputs
                            .iter()
                            .filter_map(|(i, g)| {
                                trace_names.get(*i).cloned().map(|n| (TraceRef(n), *g))
                            })
                            .collect();
                        if !app.math_panel.builder.name.is_empty() {
                            let def = MathTraceDef {
                                name: app.math_panel.builder.name.clone(),
                                color_hint: None,
                                kind: MathKind::Add { inputs },
                            };
                            // Apply and then set created/edited look
                            app.apply_add_or_edit(def);
                            if app.math_panel.error.is_none() {
                                if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                    if is_creating {
                                        tr.look = app.math_panel.builder.look.clone();
                                    }
                                }
                            }
                            if app.math_panel.error.is_none() {
                                app.math_panel.creating = false;
                            }
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
                                .get(app.math_panel.builder.mul_a_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut app.math_panel.builder.mul_a_idx, i, n);
                            }
                        });
                    egui::ComboBox::from_label("B")
                        .selected_text(
                            trace_names
                                .get(app.math_panel.builder.mul_b_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in trace_names.iter().enumerate() {
                                ui.selectable_value(&mut app.math_panel.builder.mul_b_idx, i, n);
                            }
                        });
                });
                ui.add_space(10.0);
                // Style editor just before Save/Add
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Style")
                        .default_open(false)
                        .show(ui, |ui| {
                            if is_editing {
                                if let Some(editing_name) = app.math_panel.editing.clone() {
                                    if let Some(tr) = app.traces.get_mut(&editing_name) {
                                        tr.look.render_editor(ui, true, None, false, None);
                                    } else {
                                        ui.label("Trace not found.");
                                    }
                                }
                            } else {
                                app.math_panel.builder.look.render_editor(ui, true, None, false, None);
                            }
                        });
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let mut save_clicked = false;
                    if ui.button(save_label).clicked() {
                        save_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        if let (Some(a), Some(b)) = (
                            trace_names.get(app.math_panel.builder.mul_a_idx),
                            trace_names.get(app.math_panel.builder.mul_b_idx),
                        ) {
                            let kind = if app.math_panel.builder.kind_idx == 1 {
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
                            if !app.math_panel.builder.name.is_empty() {
                                let def = MathTraceDef {
                                    name: app.math_panel.builder.name.clone(),
                                    color_hint: None,
                                    kind,
                                };
                                app.apply_add_or_edit(def);
                                if app.math_panel.error.is_none() {
                                    if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                        if is_creating {
                                            tr.look = app.math_panel.builder.look.clone();
                                        }
                                    }
                                    app.math_panel.creating = false;
                                }
                            }
                        }
                    }
                });
            }
            3 => {
                // Differentiate
                egui::ComboBox::from_label("Input")
                    .selected_text(
                        trace_names
                            .get(app.math_panel.builder.single_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, n) in trace_names.iter().enumerate() {
                            ui.selectable_value(&mut app.math_panel.builder.single_idx, i, n);
                        }
                    });
                ui.add_space(10.0);
                // Style editor just before Save/Add
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Style")
                    .default_open(false)
                    .show(ui, |ui| {
                        if is_editing {
                            if let Some(editing_name) = app.math_panel.editing.clone() {
                                if let Some(tr) = app.traces.get_mut(&editing_name) {
                                    tr.look.render_editor(ui, true, None, false, None);
                                } else {
                                    ui.label("Trace not found.");
                                }
                            }
                        } else {
                            app.math_panel
                                .builder
                                .look
                                .render_editor(ui, true, None, false, None);
                        }
                    });
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let mut save_clicked = false;
                    if ui.button(save_label).clicked() {
                        save_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        if let Some(nm) = trace_names.get(app.math_panel.builder.single_idx) {
                            if !app.math_panel.builder.name.is_empty() {
                                let def = MathTraceDef {
                                    name: app.math_panel.builder.name.clone(),
                                    color_hint: None,
                                    kind: MathKind::Differentiate {
                                        input: TraceRef(nm.clone()),
                                    },
                                };
                                app.apply_add_or_edit(def);
                                if app.math_panel.error.is_none() {
                                    if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                        if is_creating {
                                            tr.look = app.math_panel.builder.look.clone();
                                        }
                                    }
                                    app.math_panel.creating = false;
                                }
                            }
                        }
                    }
                });
            }
            4 => {
                // Integrate
                egui::ComboBox::from_label("Input")
                    .selected_text(
                        trace_names
                            .get(app.math_panel.builder.single_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, n) in trace_names.iter().enumerate() {
                            ui.selectable_value(&mut app.math_panel.builder.single_idx, i, n);
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("y0");
                    ui.add(egui::DragValue::new(&mut app.math_panel.builder.integ_y0).speed(0.1));
                });
                ui.add_space(10.0);
                // Style editor just before Save/Add
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Style")
                    .default_open(false)
                    .show(ui, |ui| {
                        if is_editing {
                            if let Some(editing_name) = app.math_panel.editing.clone() {
                                if let Some(tr) = app.traces.get_mut(&editing_name) {
                                    tr.look.render_editor(ui, true, None, false, None);
                                } else {
                                    ui.label("Trace not found.");
                                }
                            }
                        } else {
                            app.math_panel
                                .builder
                                .look
                                .render_editor(ui, true, None, false, None);
                        }
                    });
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let mut save_clicked = false;
                    if ui.button(save_label).clicked() {
                        save_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        if let Some(nm) = trace_names.get(app.math_panel.builder.single_idx) {
                            if !app.math_panel.builder.name.is_empty() {
                                let def = MathTraceDef {
                                    name: app.math_panel.builder.name.clone(),
                                    color_hint: None,
                                    kind: MathKind::Integrate {
                                        input: TraceRef(nm.clone()),
                                        y0: app.math_panel.builder.integ_y0,
                                    },
                                };
                                app.apply_add_or_edit(def);
                                if app.math_panel.error.is_none() {
                                    if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                        if is_creating {
                                            tr.look = app.math_panel.builder.look.clone();
                                        }
                                    }
                                    app.math_panel.creating = false;
                                }
                            }
                        }
                    }
                });
            }
            5 => {
                // Filter
                egui::ComboBox::from_label("Input")
                    .selected_text(
                        trace_names
                            .get(app.math_panel.builder.single_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, n) in trace_names.iter().enumerate() {
                            ui.selectable_value(&mut app.math_panel.builder.single_idx, i, n);
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
                    .selected_text(fk[app.math_panel.builder.filter_which])
                    .show_ui(ui, |ui| {
                        for (i, n) in fk.iter().enumerate() {
                            ui.selectable_value(&mut app.math_panel.builder.filter_which, i, *n);
                        }
                    });
                match app.math_panel.builder.filter_which {
                    0 | 1 => {
                        ui.horizontal(|ui| {
                            ui.label("Cutoff Hz");
                            ui.add(
                                egui::DragValue::new(&mut app.math_panel.builder.filter_f1).speed(0.1),
                            );
                        });
                    }
                    2 => {
                        ui.horizontal(|ui| {
                            ui.label("Low cut Hz");
                            ui.add(
                                egui::DragValue::new(&mut app.math_panel.builder.filter_f1).speed(0.1),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("High cut Hz");
                            ui.add(
                                egui::DragValue::new(&mut app.math_panel.builder.filter_f2).speed(0.1),
                            );
                        });
                    }
                    3 | 4 | 5 => {
                        let label = match app.math_panel.builder.filter_which {
                            3 | 4 => "Cutoff Hz",
                            _ => "Center Hz",
                        };
                        ui.horizontal(|ui| {
                            ui.label(label);
                            ui.add(
                                egui::DragValue::new(&mut app.math_panel.builder.filter_f1).speed(0.1),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Q");
                            ui.add(
                                egui::DragValue::new(&mut app.math_panel.builder.filter_q).speed(0.01),
                            );
                        });
                    }
                    _ => {}
                }
                ui.add_space(10.0);
                // Style editor just before Save/Add
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Style")
                    .default_open(false)
                    .show(ui, |ui| {
                        if is_editing {
                            if let Some(editing_name) = app.math_panel.editing.clone() {
                                if let Some(tr) = app.traces.get_mut(&editing_name) {
                                    tr.look.render_editor(ui, true, None, false, None);
                                } else {
                                    ui.label("Trace not found.");
                                }
                            }
                        } else {
                            app.math_panel
                                .builder
                                .look
                                .render_editor(ui, true, None, false, None);
                        }
                    });
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let mut save_clicked = false;
                    if ui.button(save_label).clicked() {
                        save_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        if let Some(nm) = trace_names.get(app.math_panel.builder.single_idx) {
                            if !app.math_panel.builder.name.is_empty() {
                                let kind = match app.math_panel.builder.filter_which {
                                    0 => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::Lowpass {
                                            cutoff_hz: app.math_panel.builder.filter_f1,
                                        },
                                    },
                                    1 => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::Highpass {
                                            cutoff_hz: app.math_panel.builder.filter_f1,
                                        },
                                    },
                                    2 => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::Bandpass {
                                            low_cut_hz: app.math_panel.builder.filter_f1,
                                            high_cut_hz: app.math_panel.builder.filter_f2,
                                        },
                                    },
                                    3 => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::BiquadLowpass {
                                            cutoff_hz: app.math_panel.builder.filter_f1,
                                            q: app.math_panel.builder.filter_q,
                                        },
                                    },
                                    4 => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::BiquadHighpass {
                                            cutoff_hz: app.math_panel.builder.filter_f1,
                                            q: app.math_panel.builder.filter_q,
                                        },
                                    },
                                    _ => MathKind::Filter {
                                        input: TraceRef(nm.clone()),
                                        kind: FilterKind::BiquadBandpass {
                                            center_hz: app.math_panel.builder.filter_f1,
                                            q: app.math_panel.builder.filter_q,
                                        },
                                    },
                                };
                                let def = MathTraceDef {
                                    name: app.math_panel.builder.name.clone(),
                                    color_hint: None,
                                    kind,
                                };
                                app.apply_add_or_edit(def);
                                if app.math_panel.error.is_none() {
                                    if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                        if is_creating {
                                            tr.look = app.math_panel.builder.look.clone();
                                        }
                                    }
                                    app.math_panel.creating = false;
                                }
                            }
                        }
                    }
                });
            }
            6 | 7 => {
                // Min/Max
                egui::ComboBox::from_label("Input")
                    .selected_text(
                        trace_names
                            .get(app.math_panel.builder.single_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, n) in trace_names.iter().enumerate() {
                            ui.selectable_value(&mut app.math_panel.builder.single_idx, i, n);
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Decay (1/s, 0=none)");
                    ui.add(egui::DragValue::new(&mut app.math_panel.builder.minmax_decay).speed(0.1));
                });
                // Style editor just before Save/Add
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Style")
                    .default_open(false)
                    .show(ui, |ui| {
                        if is_editing {
                            if let Some(editing_name) = app.math_panel.editing.clone() {
                                if let Some(tr) = app.traces.get_mut(&editing_name) {
                                    tr.look.render_editor(ui, true, None, false, None);
                                } else {
                                    ui.label("Trace not found.");
                                }
                            }
                        } else {
                            app.math_panel
                                .builder
                                .look
                                .render_editor(ui, true, None, false, None);
                        }
                    });
                ui.horizontal(|ui| {
                    let save_label = if is_editing { "Save" } else { "Add trace" };
                    let mut save_clicked = false;
                    if ui.button(save_label).clicked() {
                        save_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            app.math_panel.editing = None;
                            app.math_panel.creating = false;
                            app.math_panel.builder = MathBuilderState::default();
                            app.math_panel.error = None;
                        }
                    });
                    if save_clicked {
                        if let Some(nm) = trace_names.get(app.math_panel.builder.single_idx) {
                            if !app.math_panel.builder.name.is_empty() {
                                let mode = if app.math_panel.builder.kind_idx == 6 {
                                    MinMaxMode::Min
                                } else {
                                    MinMaxMode::Max
                                };
                                let decay_opt = if app.math_panel.builder.minmax_decay > 0.0 {
                                    Some(app.math_panel.builder.minmax_decay)
                                } else {
                                    None
                                };
                                let def = MathTraceDef {
                                    name: app.math_panel.builder.name.clone(),
                                    color_hint: None,
                                    kind: MathKind::MinMax {
                                        input: TraceRef(nm.clone()),
                                        decay_per_sec: decay_opt,
                                        mode,
                                    },
                                };
                                app.apply_add_or_edit(def);
                                if app.math_panel.error.is_none() {
                                    if let Some(tr) = app.traces.get_mut(&app.math_panel.builder.name) {
                                        if is_creating {
                                            tr.look = app.math_panel.builder.look.clone();
                                        }
                                    }
                                    app.math_panel.creating = false;
                                }
                            }
                        }
                    }
                });
            }
            _ => {}
        }
    }
}

// Removed unused show_math_dialog helper; dialogs are shown via DockPanel::show_detached_dialog
