use super::panel_trait::{Panel, PanelState};
use crate::data::scope::ScopeType;
use crate::data::{data::LivePlotData, traces::TraceRef};
use eframe::egui;
use egui::{Id, Ui};
use egui_phosphor::regular::DOTS_SIX_VERTICAL;
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};

use super::scope_settings_ui::{DragPayload, ScopeSettingsUiPanel};
use super::trace_look_ui::render_trace_look_editor;

pub struct TracesPanel {
    pub state: PanelState,
    pub look_editor_trace: Option<TraceRef>,
    pub look_editor_xy_pair: Option<(usize, usize)>,
    pub hover_trace: Option<TraceRef>,
    pub dragging_trace: Option<DragPayload>,

    scope_settings_ui: ScopeSettingsUiPanel,
}

impl Default for TracesPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Traces", "📈"),
            look_editor_trace: None,
            look_editor_xy_pair: None,
            hover_trace: None,
            dragging_trace: None,

            scope_settings_ui: ScopeSettingsUiPanel::default(),
        }
    }
}

impl Panel for TracesPanel {
    fn state(&self) -> &PanelState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut PanelState {
        &mut self.state
    }

    fn render_menu(&mut self, ui: &mut egui::Ui, data: &mut LivePlotData<'_>) {
        ui.menu_button(self.title_and_icon(), |ui| {
            // Show Traces: open the Traces panel and focus dock
            if ui.button("Show Traces").clicked() {
                let st = self.state_mut();
                st.visible = true;
                st.request_focus = true;
                ui.close();
            }

            ui.separator();

            // Data Points slider (mirror of panel control)
            ui.horizontal(|ui| {
                ui.label("Data Points:");
                ui.add(egui::Slider::new(
                    &mut data.traces.max_points,
                    data.traces.points_bounds.0..=data.traces.points_bounds.1,
                ));
            });

            ui.separator();

            // Visibility control: All Visible / All Hidden
            if ui.button("All Visible").clicked() {
                for (_name, tr) in data.traces.traces_iter_mut() {
                    tr.look.visible = true;
                }
                ui.close();
            }
            if ui.button("All Hidden").clicked() {
                for (_name, tr) in data.traces.traces_iter_mut() {
                    tr.look.visible = false;
                }
                ui.close();
            }

            ui.separator();

            if ui.button("X Clear All").clicked() {
                data.traces.clear_all();
                ui.close();
            }
        });
    }

    fn render_panel(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
        if data.scope_data.is_empty() {
            ui.label("No scopes available.");
            return;
        }

        self.render_scope_assignments(ui, data);
        ui.separator();

        ui.label("Data Points:");
        ui.add(egui::Slider::new(
            &mut data.traces.max_points,
            data.traces.points_bounds.0..=data.traces.points_bounds.1,
        ));

        ui.separator();

        ui.horizontal(|ui| {
            ui.button("All Visible").clicked().then(|| {
                for (_name, tr) in data.traces.traces_iter_mut() {
                    tr.look.visible = true;
                }
            });
            ui.button("All Hidden").clicked().then(|| {
                for (_name, tr) in data.traces.traces_iter_mut() {
                    tr.look.visible = false;
                }
            });
        });

        ui.separator();

        self.hover_trace = None;

        #[derive(Clone)]
        struct Row {
            name: TraceRef,
        }
        let rows: Vec<Row> = data
            .traces
            .all_trace_names()
            .into_iter()
            .map(|name| Row { name })
            .collect();

        struct TracesDelegate<'a> {
            traces: &'a mut crate::data::traces::TracesCollection,
            hover_out: &'a mut Option<TraceRef>,
            look_toggle: &'a mut Option<TraceRef>,
            drag_out: &'a mut Option<DragPayload>,
            rows: Vec<Row>,
        }
        impl<'a> TableDelegate for TracesDelegate<'a> {
            fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
                let col = cell.col_range.start;
                let text = match col {
                    0 => "",
                    1 => "",
                    2 => "Trace",
                    3 => "Visible",
                    4 => "Offset",
                    5 => "Info",
                    _ => "",
                };

                // Center certain headers; keep Trace/Info left-aligned.
                let centered_cols = [0usize, 1, 3, 4];
                if centered_cols.contains(&col) {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.strong(text);
                        },
                    );
                } else {
                    ui.add_space(4.0);
                    ui.strong(text);
                }
            }
            fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
                let row = cell.row_nr as usize;
                let col = cell.col_nr;
                if row >= self.rows.len() {
                    return;
                }
                let r = &self.rows[row];

                match col {
                    0 => {
                        // Drag handle centered.
                        ui.with_layout(
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                let resp = ui
                                    .add(
                                        egui::Label::new(DOTS_SIX_VERTICAL)
                                            .sense(egui::Sense::click_and_drag()),
                                    )
                                    .on_hover_text("Drag into a scope / XY slot");

                                if resp.hovered() {
                                    *self.hover_out = Some(r.name.clone());
                                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                                }
                                if resp.drag_started() || resp.dragged() {
                                    *self.drag_out = Some(DragPayload {
                                        trace: r.name.clone(),
                                        origin_scope_id: None,
                                    });
                                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                                }
                            },
                        );
                    }
                    1 => {
                        // Color editor centered.
                        ui.with_layout(
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                    let mut c = tr.look.color;
                                    let resp = ui
                                        .color_edit_button_srgba(&mut c)
                                        .on_hover_text("Change trace color");
                                    if resp.hovered() {
                                        *self.hover_out = Some(r.name.clone());
                                    }
                                    if resp.changed() {
                                        tr.look.color = c;
                                    }
                                }
                            },
                        );
                    }
                    2 => {
                        ui.add_space(4.0);
                        let resp = ui.add(
                            egui::Label::new(r.name.0.clone())
                                .truncate()
                                .show_tooltip_when_elided(true)
                                .sense(egui::Sense::click_and_drag()),
                        );
                        if resp.hovered() {
                            *self.hover_out = Some(r.name.clone());
                        }
                        if resp.drag_started() || resp.dragged() {
                            *self.drag_out = Some(DragPayload {
                                trace: r.name.clone(),
                                origin_scope_id: None,
                            });
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                        }
                        if resp.clicked() {
                            *self.look_toggle = Some(r.name.clone());
                        }
                    }
                    3 => {
                        // Visible checkbox centered.
                        ui.with_layout(
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                    let mut vis = tr.look.visible;
                                    let resp = ui.checkbox(&mut vis, "");
                                    if resp.hovered() {
                                        *self.hover_out = Some(r.name.clone());
                                    }
                                    if resp.changed() {
                                        tr.look.visible = vis;
                                    }
                                }
                            },
                        );
                    }
                    4 => {
                        // Offset DragValue centered.
                        ui.with_layout(
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                    let mut off = tr.offset;
                                    let resp = ui.add(
                                        egui::DragValue::new(&mut off)
                                            .speed(0.01)
                                            .range(-1.0e12..=1.0e12),
                                    );
                                    if resp.hovered() {
                                        *self.hover_out = Some(r.name.clone());
                                    }
                                    if resp.changed() {
                                        tr.offset = off;
                                    }
                                }
                            },
                        );
                    }
                    5 => {
                        ui.add_space(4.0);
                        if let Some(tr) = self.traces.get_trace(&r.name) {
                            let text = tr.info.clone();
                            let resp = ui.add(
                                egui::Label::new(text.clone())
                                    .truncate()
                                    .show_tooltip_when_elided(true)
                                    .sense(egui::Sense::click()),
                            );
                            if resp.hovered() {
                                *self.hover_out = Some(r.name.clone());
                            }
                            if resp.clicked() {
                                ui.ctx().copy_text(text);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Auto-size columns, but constrain them with min/max ranges.
        // Columns: Drag, Color, Trace, Visible, Offset, Info.
        let cols = vec![
            // Drag handle: very compact
            egui_table::Column::new(18.0).range(egui::Rangef::new(18.0, 18.0)),
            // Color editor: compact
            egui_table::Column::new(28.0).range(egui::Rangef::new(22.0, 40.0)),
            // Trace name: flexible
            egui_table::Column::new(140.0).range(egui::Rangef::new(80.0, 600.0)),
            // Visible checkbox: small
            egui_table::Column::new(50.0).range(egui::Rangef::new(50.0, 50.0)),
            // Offset: medium
            egui_table::Column::new(50.0).range(egui::Rangef::new(50.0, 50.0)),
            // Info: flexible (large max so the table can still fill wide panels)
            egui_table::Column::new(260.0).range(egui::Rangef::new(100.0, 2000.0)),
        ];
        // Compute a preferred height for the table; size it relative to available height
        let header_h = 24.0_f32;
        let row_h = 22.0_f32;
        let rows_len = rows.len() as f32;
        let editor_open = self.look_editor_trace.is_some() || self.look_editor_xy_pair.is_some();
        let preferred = header_h + row_h * rows_len + 8.0;
        let avail_h = ui.available_height();
        // With the editor placed below the table, give the table a larger share when open.
        let max_h = if editor_open {
            (avail_h * 0.65).max(200.0)
        } else {
            (avail_h * 0.85).max(200.0)
        };
        let table_h = preferred.clamp(120.0, max_h);

        // Draw the table first (style editor is rendered below).
        let rows_clone = rows.clone();
        {
            let mut hover_tmp: Option<TraceRef> = None;
            let mut look_toggle_req: Option<TraceRef> = None;
            let mut drag_from_table: Option<DragPayload> = None;
            // Borrow traces mutably for the table drawing scope only.
            let traces_ref = &mut *data.traces;
            let mut delegate = TracesDelegate {
                traces: traces_ref,
                hover_out: &mut hover_tmp,
                look_toggle: &mut look_toggle_req,
                drag_out: &mut drag_from_table,
                rows: rows_clone,
            };

            let avail_w = ui.available_width();
            let (rect, _resp) =
                ui.allocate_exact_size(egui::vec2(avail_w, table_h), egui::Sense::hover());
            let ui_builder = egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Min));
            let mut table_ui = ui.new_child(ui_builder);

            // Avoid text selection/marking during drag gestures inside the table.
            table_ui.style_mut().interaction.selectable_labels = false;

            Table::new()
                .id_salt("traces_table")
                .auto_size_mode(egui_table::AutoSizeMode::OnParentResize)
                .num_rows(delegate.rows.len() as u64)
                .columns(cols)
                .headers(vec![EgHeaderRow::new(24.0)])
                .show(&mut table_ui, &mut delegate);

            // Write back hover and selection state after drawing table.
            self.hover_trace = hover_tmp;
            if let Some(dragged) = drag_from_table {
                self.dragging_trace = Some(dragged);
            }
            if let Some(tn) = look_toggle_req {
                if self.look_editor_trace.as_deref() == Some(tn.as_str()) {
                    self.look_editor_trace = None;
                } else {
                    self.look_editor_trace = Some(tn);
                    self.hover_trace = None;
                }
            }
        }

        // Drag preview for drags originating from the main traces list.
        if let Some(dragging) = self.dragging_trace.clone() {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(Id::new(("trace_drag_preview", dragging.trace.0.clone())))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos + egui::vec2(16.0, 16.0))
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(DOTS_SIX_VERTICAL);
                                ui.label(dragging.trace.0.clone());
                            });
                        });
                    });
            }
        }

        // Render inline style editor beneath the table.
        if let Some(tn) = self.look_editor_trace.clone() {
            self.look_editor_xy_pair = None;
            if let Some(tr) = data.traces.get_trace_mut(&tn) {
                ui.add_space(8.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Style: {}", tn));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖ Close").clicked() {
                                self.look_editor_trace = None;
                            }
                        });
                    });
                    ui.separator();
                    render_trace_look_editor(&mut tr.look, ui, true);
                });
            } else {
                ui.label("Trace not found.");
                self.look_editor_trace = None;
            }
        } else if let Some((scope_id, pair_idx)) = self.look_editor_xy_pair {
            ui.add_space(8.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("XY Pair Style: #{}", pair_idx + 1));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖ Close").clicked() {
                            self.look_editor_xy_pair = None;
                        }
                    });
                });
                ui.separator();

                if let Some(scope) = data.scope_data.iter_mut().find(|s| s.id == scope_id) {
                    if let Some((_x, _y, look)) = scope.xy_pairs.get_mut(pair_idx) {
                        render_trace_look_editor(look, ui, true);
                    } else {
                        ui.label("XY pair not found.");
                    }
                } else {
                    ui.label("Scope not found.");
                }
            });
        }

        if let Some(hover_trace) = self.hover_trace.clone() {
            data.traces.hover_trace = Some(hover_trace.clone());
        }

        if ui.ctx().input(|i| i.pointer.any_released()) {
            self.dragging_trace = None;
        }
    }
}

impl TracesPanel {
    fn render_scope_assignments(&mut self, ui: &mut Ui, data: &mut LivePlotData<'_>) {
        let can_remove_scope = data.scope_data.len() > 1;
        if ui.button("➕ Add scope").clicked() {
            data.pending_requests.add_scope = true;
        }
        ui.add_space(4.0);
        let mut moved: Vec<(usize, TraceRef)> = Vec::new();
        for scope in data.scope_data.iter_mut() {
            ui.separator();
            ui.add_space(4.0);

            let settings_resp = self.scope_settings_ui.render_scope_assignment(
                ui,
                scope,
                can_remove_scope,
                &mut data.pending_requests,
                &mut self.dragging_trace,
                &mut *data.traces,
                &mut self.look_editor_trace,
                &mut self.look_editor_xy_pair,
            );

            if let Some(m) = settings_resp.moved_from_scope {
                moved.push(m);
            }

            if settings_resp.type_changed {
                match scope.scope_type {
                    ScopeType::TimeScope => scope.fit_y_bounds(&*data.traces),
                    ScopeType::XYScope => scope.fit_bounds(&*data.traces),
                }
            }

            if settings_resp.type_changed || settings_resp.scope_changed {
                ui.ctx().request_repaint();
            }
        }

        if !moved.is_empty() {
            for (src_id, tr) in moved {
                if let Some(src_scope) = data.scope_data.iter_mut().find(|s| s.id == src_id) {
                    src_scope.remove_trace(&tr);
                }
            }
            ui.ctx().request_repaint();
        }
    }
}
