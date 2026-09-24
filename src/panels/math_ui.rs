use crate::data::data::LivePlotData;
use crate::data::math::{FilterKind, FormulaTimeMode, MathKind, MathTrace, MinMaxMode};
use crate::data::traces::TraceRef;
use eframe::egui;
use egui::{Color32, Ui};
use std::collections::HashMap;

use crate::data::trace_look::TraceLook;
use crate::panels::panel_trait::{Panel, PanelState};
use crate::panels::trace_look_ui::render_trace_look_editor;
use egui_phosphor_icons::icons::{BROOM, PLUS, RECYCLE};

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
            state: PanelState::new("Math", "∫"), // split into icon + title
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

    fn hotkey_name(&self) -> Option<crate::data::hotkeys::HotkeyName> {
        Some(crate::data::hotkeys::HotkeyName::Math)
    }

    fn render_menu(
        &mut self,
        ui: &mut egui::Ui,
        data: &mut LivePlotData<'_>,
        collapsed: bool,
        tooltip: &str,
    ) {
        let label = if collapsed {
            self.icon_only()
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.title().to_string())
        } else {
            self.title_and_icon()
        };
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let mr = egui::containers::menu::MenuButton::new(label)
            .config(menu_cfg)
            .ui(ui, |ui| {
                if ui.button("Show Math").clicked() {
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }

                ui.separator();

                if ui.button(format!("{} New", PLUS.as_str())).clicked() {
                    self.builder =
                        MathTrace::new(TraceRef::default(), MathKind::Add { inputs: Vec::new() });
                    self.editing = None;
                    self.creating = true;
                    self.error = None;
                    let st = self.state_mut();
                    st.visible = true;
                    st.request_focus = true;
                    ui.close();
                }
                if ui
                    .button(format!("{} Reset All Storage", RECYCLE.as_str()))
                    .clicked()
                {
                    for def in self.math_traces.iter_mut() {
                        data.traces.clear_trace(&def.name);
                        def.reset_runtime_state();
                    }
                    ui.close();
                }
                if ui.button(format!("{} Clear All", BROOM.as_str())).clicked() {
                    // Remove math traces & their underlying live data
                    for def in self.math_traces.iter() {
                        // Emit MATH_TRACE_REMOVED for each
                        if let Some(ctrl) = &data.event_ctrl {
                            let mut evt = crate::events::PlotEvent::new(
                                crate::events::EventKind::MATH_TRACE_REMOVED,
                            );
                            evt.math_trace = Some(crate::events::MathTraceMeta {
                                name: def.name.0.clone(),
                                formula: None,
                            });
                            ctrl.emit_filtered(evt);
                        }
                        data.traces.remove_trace(&def.name);
                    }
                    self.math_traces.clear();
                    self.editing = None;
                    self.creating = false;
                    ui.close();
                }
            });
        if !tooltip.is_empty() {
            mr.0.on_hover_text(tooltip);
        }
    }

    fn update_data(&mut self, data: &mut LivePlotData<'_>) {
        if data.pending_requests.clear_math {
            for def in self.math_traces.iter_mut() {
                data.traces.clear_trace(&def.name);
                def.reset_runtime_state();
            }
            data.pending_requests.clear_math = false;
        }

        if self.math_traces.is_empty() {
            return;
        }

        // Collect only the traces that are actually referenced as inputs by
        // any math trace definition, plus the math traces' own previous output.
        let mut needed: std::collections::HashSet<TraceRef> = std::collections::HashSet::new();
        for def in &self.math_traces {
            let inputs = def.input_trace_names();
            // A formula without trace references (e.g. `sin(2*pi*t)`) still
            // needs a time grid — feed every existing trace into the sources
            // map so the union of timestamps spans the timeline.
            if inputs.is_empty() && matches!(def.kind, MathKind::Formula { .. }) {
                needed.extend(data.traces.all_trace_names());
            }
            needed.extend(inputs);
            needed.insert(def.name.clone());
        }

        // ── Live data pass ───────────────────────────────────────────────
        let mut sources: HashMap<TraceRef, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in data.traces.traces_iter() {
            if needed.contains(name) {
                sources.insert(name.clone(), tr.live.iter().copied().collect());
            }
        }

        for def in self.math_traces.iter_mut() {
            let out = def.compute_math_trace(&sources);

            let tr = data.get_trace_or_new(&def.name);
            tr.set_live_points(&out);
            tr.info = def.math_formula_string();

            sources.insert(def.name.clone(), out);
        }

        // ── Snapshot data pass ───────────────────────────────────────────
        // Only run while a snapshot exists (i.e. a scope is paused). Writing
        // `snap` unconditionally would keep `TracesCollection::has_snapshot`
        // permanently true for math traces and make the render caches track a
        // stale buffer instead of `live`.
        if !data.traces.has_snapshot() {
            return;
        }
        sources.clear();
        for (name, tr) in data.traces.traces_iter() {
            if needed.contains(name) {
                if let Some(d) = tr.snap.clone() {
                    sources.insert(name.clone(), d.iter().copied().collect());
                }
            }
        }

        for def in self.math_traces.iter_mut() {
            let out = def.compute_math_trace(&sources);

            let tr = data.get_trace_or_new(&def.name);
            tr.set_snap_points(&out);
            tr.info = def.math_formula_string();

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
                .button(format!("{} Reset All Storage", RECYCLE.as_str()))
                .on_hover_text("Reset integrators, filters, min/max for all math traces")
                .clicked()
            {
                for def in self.math_traces.iter_mut() {
                    data.traces.clear_trace(&def.name);
                    def.reset_runtime_state();
                }
            }
        });
        ui.add_space(6.0);
        // Existing math traces list with color editor, name, info, and Remove (right-aligned)
        // Reset hover before drawing; rows will set it when hovered

        let mut hover_trace_intern: Option<Vec<TraceRef>> = None;
        // Set when a formula trace's reset button is clicked — the def being
        // iterated is a clone, so the origin update is applied afterwards.
        let mut pending_t_reset: Option<TraceRef> = None;
        for def in self.math_traces.clone().iter_mut() {
            let row = ui.horizontal(|ui| {
                // Color editor like in traces_ui
                if let Some(tr) = data.traces.get_trace_mut(&def.name) {
                    let mut c = tr.look.color;
                    let resp = ui
                        .color_edit_button_srgba(&mut c)
                        .on_hover_text("Change trace color");
                    if resp.hovered() {
                        hover_trace_intern = Some(vec![def.name.clone()]);
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
                    hover_trace_intern = Some(vec![def.name.clone()]);
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
                    hover_trace_intern = Some(vec![def.name.clone()]);
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
                    let remove_resp = ui
                        .button(egui_phosphor_icons::icons::TRASH)
                        .on_hover_text("Remove");
                    if remove_resp.hovered() {
                        hover_trace_intern = Some(vec![def.name.clone()]);
                    }
                    if remove_resp.clicked() {
                        let removing = def.name.clone();
                        // Emit MATH_TRACE_REMOVED
                        if let Some(ctrl) = &data.event_ctrl {
                            let mut evt = crate::events::PlotEvent::new(
                                crate::events::EventKind::MATH_TRACE_REMOVED,
                            );
                            evt.math_trace = Some(crate::events::MathTraceMeta {
                                name: removing.0.clone(),
                                formula: None,
                            });
                            ctrl.emit_filtered(evt);
                        }
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
                    // Show Reset for kinds that have internal storage, and for
                    // formula traces in resettable-t mode (moves t=0 to the
                    // newest sample, like the integrator reset).
                    let is_stateful = matches!(
                        def.kind,
                        MathKind::Integrate { .. }
                            | MathKind::Filter { .. }
                            | MathKind::MinMax { .. }
                    );
                    let resettable_t = matches!(def.kind, MathKind::Formula { .. })
                        && matches!(def.time_mode, FormulaTimeMode::Resettable);
                    if is_stateful || resettable_t {
                        let reset_resp = ui
                            .button(egui_phosphor_icons::icons::ARROW_CLOCKWISE)
                            .on_hover_text(if resettable_t {
                                "Reset t to 0 at the newest sample"
                            } else {
                                "Reset integrator/filter/min/max state for this trace"
                            });
                        if reset_resp.hovered() {
                            hover_trace_intern = Some(vec![def.name.clone()]);
                        }
                        if reset_resp.clicked() {
                            if resettable_t {
                                pending_t_reset = Some(def.name.clone());
                            }
                            data.traces.clear_trace(&def.name);
                        }
                    }
                });
            });
            if row.response.hovered() {
                hover_trace_intern = Some(vec![def.name.clone()]);
            }
        }
        if let Some(nm) = hover_trace_intern {
            data.traces.hover_trace = Some(nm);
        }
        // Apply a formula trace's t-reset: origin moves to the newest sample
        // (or None → auto-init at the first sample when no data exists).
        if let Some(name) = pending_t_reset {
            if let Some(d) = self.math_traces.iter_mut().find(|d| d.name == name) {
                d.t_origin = data
                    .traces
                    .traces_iter()
                    .filter_map(|(_, tr)| tr.live.back().map(|p| p[0]))
                    .reduce(f64::max);
            }
        }

        // Style popup removed; the editor is part of the new/edit dialog above

        // Full-width New button after the list
        ui.add_space(6.0);
        let new_clicked = ui
            .add_sized(
                [ui.available_width(), 24.0],
                egui::Button::new(format!("{} New", PLUS.as_str())),
            )
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
                        .stroke(egui::Stroke::new(1.5_f32, egui::Color32::RED))
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
                "Formula",
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
                MathKind::Formula { .. } => 8,
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
                .traces
                .all_trace_names()
                .into_iter()
                .filter(|n| *n != self.builder.name)
                .collect();

            if kind_idx != prev_kind_idx {
                // Switch to a new kind with sensible defaults
                let first = trace_names.first().cloned().unwrap_or_default();
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
                    8 => MathKind::Formula {
                        expr: String::new(),
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
                    Some(data.traces.len())
                };
                if let Some(idx) = future_idx {
                    self.builder_look.color = TraceLook::alloc_color(idx);
                }
            }

            match &mut self.builder.kind {
                MathKind::Add { inputs } => {
                    // Ensure at least one input row for UX
                    if inputs.is_empty() {
                        if let Some(nm) = trace_names.first() {
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
                                        ui.selectable_value(
                                            &mut selected,
                                            n.0.clone(),
                                            n.0.clone(),
                                        );
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
                        if ui.button(format!("{} Add input", PLUS.as_str())).clicked() {
                            let nm = trace_names.first().cloned().unwrap_or_default();
                            inputs.push((nm, 1.0));
                        }
                        if ui.button("Remove input").clicked() && inputs.len() > 1 {
                            inputs.pop();
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
                        3..=5 => {
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
                MathKind::Formula { expr } => {
                    // Insert chips: one per available trace (in trace color),
                    // plus t / pi / e and a function menu. Clicking inserts at
                    // the text cursor of the formula edit below.
                    let edit_id = egui::Id::new("math_formula_edit");
                    let mut pending_insert: Option<(String, usize)> = None;
                    ui.horizontal_wrapped(|ui| {
                        for n in &trace_names {
                            let color = data
                                .traces
                                .get_trace(n)
                                .map(|t| t.look.color)
                                .unwrap_or_else(|| ui.visuals().text_color());
                            if ui
                                .button(egui::RichText::new(n.0.clone()).color(color))
                                .on_hover_text(format!("Insert {{{}}}", n.0))
                                .clicked()
                            {
                                pending_insert = Some((format!("{{{}}}", n.0), 0));
                            }
                        }
                        for (label, snip) in [("t", "t"), ("π", "pi"), ("e", "e")] {
                            if ui
                                .button(label)
                                .on_hover_text(format!("Insert {snip}"))
                                .clicked()
                            {
                                pending_insert = Some((snip.to_string(), 0));
                            }
                        }
                        let menu_cfg = egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
                        let mb = egui::containers::menu::MenuButton::new("ƒx")
                            .config(menu_cfg)
                            .ui(ui, |ui| {
                                for f in crate::data::expr::Func::ALL {
                                    if ui.button(f.name()).clicked() {
                                        // `name()` with the cursor between the
                                        // parens (one char back from the end).
                                        pending_insert = Some((format!("{}()", f.name()), 1));
                                        ui.close();
                                    }
                                }
                            });
                        mb.0.on_hover_text("Insert function");
                    });

                    // `t` semantics for this formula: absolute timestamp vs.
                    // seconds since a resettable origin (per-trace state on
                    // the builder; applied to the trace on save).
                    ui.horizontal(|ui| {
                        ui.label("t:");
                        ui.selectable_value(
                            &mut self.builder.time_mode,
                            FormulaTimeMode::Absolute,
                            "absolute",
                        )
                        .on_hover_text("t = absolute timestamp in seconds");
                        ui.selectable_value(
                            &mut self.builder.time_mode,
                            FormulaTimeMode::Resettable,
                            "since reset",
                        )
                        .on_hover_text("t = seconds since reset (auto-starts at the first sample)");
                    });

                    // Syntax-highlight {trace} refs in the trace's color.
                    let edit_colors: HashMap<String, Color32> = data
                        .traces
                        .traces_iter()
                        .map(|(n, tr)| (n.0.clone(), tr.look.color))
                        .collect();
                    let edit_font = egui::FontSelection::Default.resolve(ui.style());
                    let edit_text_color = ui
                        .visuals()
                        .override_text_color
                        .unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
                    let edit_line_height = ui.fonts_mut(|f| f.row_height(&edit_font))
                        + ui.spacing().extra_text_line_spacing;
                    let mut layouter = formula_layouter(
                        &edit_colors,
                        edit_font,
                        edit_text_color,
                        edit_line_height,
                    );
                    let output = egui::TextEdit::multiline(expr)
                        .id(edit_id)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("e.g. sqrt({a}^2 + {b}^2)")
                        .layouter(&mut layouter)
                        .show(ui);
                    if let Some((snippet, back)) = pending_insert.take() {
                        insert_snippet(ui.ctx(), edit_id, expr, &snippet, back);
                        output.response.request_focus();
                    }

                    // Live preview / error display
                    if !expr.trim().is_empty() {
                        match crate::data::expr::parse(expr) {
                            Ok(ast) => {
                                let color_map: HashMap<TraceRef, Color32> = data
                                    .traces
                                    .traces_iter()
                                    .map(|(n, tr)| (n.clone(), tr.look.color))
                                    .collect();
                                egui::Frame::default()
                                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                                    .inner_margin(egui::Margin::same(4))
                                    .show(ui, |ui| {
                                        crate::panels::formula_render::show_formula(
                                            ui,
                                            &ast,
                                            &color_map,
                                            ui.visuals().text_color(),
                                        );
                                    });
                                let unknown: Vec<String> = ast
                                    .referenced_traces()
                                    .into_iter()
                                    .filter(|n| !data.traces.contains_key(n))
                                    .map(|n| n.0)
                                    .collect();
                                if !unknown.is_empty() {
                                    ui.colored_label(
                                        Color32::YELLOW,
                                        format!("unknown trace(s): {}", unknown.join(", ")),
                                    );
                                }
                            }
                            Err(e) => {
                                ui.colored_label(Color32::LIGHT_RED, e.to_string());
                            }
                        }
                    }
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
                let save_label = if is_editing {
                    "Save".to_string()
                } else {
                    format!("{} Add trace", PLUS.as_str())
                };
                let formula_ok = match &self.builder.kind {
                    MathKind::Formula { expr } => {
                        !expr.trim().is_empty() && crate::data::expr::parse(expr).is_ok()
                    }
                    _ => true,
                };
                let can_save = !self.builder.name.0.is_empty() && !duplicate_name && formula_ok;
                // Don't treat Enter as Save while typing a multiline formula —
                // it should insert a newline instead.
                let formula_has_focus = matches!(self.builder.kind, MathKind::Formula { .. })
                    && ui
                        .ctx()
                        .memory(|m| m.has_focus(egui::Id::new("math_formula_edit")));
                let enter_pressed = can_save
                    && !formula_has_focus
                    && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add_enabled(can_save, egui::Button::new(save_label))
                    .clicked()
                    || enter_pressed
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
                                    prev_look =
                                        data.traces.get_trace(&orig).map(|t| t.look.clone());
                                    data.remove_trace(&orig);
                                } else {
                                    // Keep current look when not renaming
                                    prev_look =
                                        data.traces.get_trace(&orig).map(|t| t.look.clone());
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
                            // Emit MATH_TRACE_ADDED
                            if let Some(ctrl) = &data.event_ctrl {
                                let mut evt = crate::events::PlotEvent::new(
                                    crate::events::EventKind::MATH_TRACE_ADDED,
                                );
                                evt.math_trace = Some(crate::events::MathTraceMeta {
                                    name: tr.name.0.clone(),
                                    formula: Some(tr.math_formula_string()),
                                });
                                ctrl.emit_filtered(evt);
                            }
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

    fn settings_snapshot(&self, data: &LivePlotData<'_>) -> Option<String> {
        let looks: Vec<(String, crate::persistence::TraceLookSerde)> = self
            .math_traces
            .iter()
            .filter_map(|mt| {
                data.traces.get_trace(&mt.name).map(|tr| {
                    (
                        mt.name.0.clone(),
                        crate::persistence::TraceLookSerde::from(&tr.look),
                    )
                })
            })
            .collect();
        serde_json::to_string(&(self.math_traces.clone(), looks)).ok()
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

/// Insert `snippet` into `expr` at the cursor of the `TextEdit` identified by
/// `edit_id`, replacing any active selection. The cursor is placed `back`
/// characters before the end of the inserted text (e.g. `back = 1` lands inside
/// the parens of an inserted `f()`).
fn insert_snippet(
    ctx: &egui::Context,
    edit_id: egui::Id,
    expr: &mut String,
    snippet: &str,
    back: usize,
) {
    let char_to_byte = |ci: usize| -> usize {
        expr.char_indices()
            .nth(ci)
            .map(|(i, _)| i)
            .unwrap_or(expr.len())
    };
    let mut state = egui::TextEdit::load_state(ctx, edit_id).unwrap_or_default();
    let end = expr.chars().count();
    let (lo, hi) = state
        .cursor
        .char_range()
        .map(|r| {
            (
                r.primary.index.0.min(r.secondary.index.0).min(end),
                r.primary.index.0.max(r.secondary.index.0).min(end),
            )
        })
        .unwrap_or((end, end));
    expr.replace_range(char_to_byte(lo)..char_to_byte(hi), snippet);
    let new_pos = lo + snippet.chars().count().saturating_sub(back);
    let c = egui::text::CCursor::new(new_pos);
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(c, c)));
    egui::TextEdit::store_state(ctx, edit_id, state);
}

/// Custom [`egui::TextEdit`] layouter for the formula editor: renders
/// `{trace}` references in the trace's color, everything else in the default
/// text color. Mirrors `TextEdit`'s default multiline layouter (font,
/// `line_height`, wrapping) so the text looks identical apart from colors.
fn formula_layouter<'a>(
    trace_colors: &'a HashMap<String, Color32>,
    font_id: egui::FontId,
    text_color: Color32,
    line_height: f32,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> + 'a {
    move |ui, buf, wrap_width| {
        let text = buf.as_str();
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = wrap_width;
        job.break_on_newline = true;
        job.keep_trailing_whitespace = true;
        let format = |color: Color32| {
            let mut f = egui::text::TextFormat::simple(font_id.clone(), color);
            f.line_height = Some(line_height);
            f
        };
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            let (plain, after_open) = rest.split_at(open);
            if !plain.is_empty() {
                job.append(plain, 0.0, format(text_color));
            }
            match after_open.find('}') {
                Some(close) => {
                    let (tok, tail) = after_open.split_at(close + 1);
                    let name = &tok[1..tok.len() - 1];
                    let color = trace_colors.get(name).copied().unwrap_or(text_color);
                    job.append(tok, 0.0, format(color));
                    rest = tail;
                }
                None => {
                    // Unterminated `{` — leave as plain text.
                    job.append(after_open, 0.0, format(text_color));
                    rest = "";
                }
            }
        }
        if !rest.is_empty() {
            job.append(rest, 0.0, format(text_color));
        }
        ui.fonts_mut(|f| f.layout_job(job))
    }
}
