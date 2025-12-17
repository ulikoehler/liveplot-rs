use super::panel_trait::{Panel, PanelState};
use crate::data::{data::LivePlotData, traces::TraceRef};
use eframe::egui;
use egui::{Id, Ui};
use egui_dnd::dnd;
use egui_phosphor::regular::DOTS_SIX_VERTICAL;
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::trace_look_ui::render_trace_look_editor;

// Feature-gated debug logging for traces table widths.
// Enable prints with: cargo run --features traces_table_debug --example sine
// or for your binary accordingly. When the feature is disabled, logs are compiled out.
#[cfg(feature = "traces_table_debug")]
#[allow(unused_macros)]
macro_rules! traces_debug { ($($arg:tt)*) => { eprintln!($($arg)*); } }

#[cfg(not(feature = "traces_table_debug"))]
#[allow(unused_macros)]
macro_rules! traces_debug {
    ($($arg:tt)*) => {{ /* no-op */ }};
}

thread_local! {
    static LAST_AVAIL_W: Cell<f32> = const { Cell::new(0.0) };
    static LAST_COL_HDR_W: RefCell<[f32; 6]> = const { RefCell::new([0.0; 6]) };
    static LAST_COL_ROW0_W: RefCell<[f32; 6]> = const { RefCell::new([0.0; 6]) };
}

#[derive(Clone, Hash)]
enum ScopeListItem {
    Header { scope_id: usize, name: String },
    Trace { scope_id: usize, trace: TraceRef },
}

#[derive(Hash, PartialEq, Eq, Clone)]
enum ScopeItemKind {
    Header,
    Trace(String),
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ScopeItemId {
    scope_id: usize,
    kind: ScopeItemKind,
}

impl ScopeListItem {
    fn dnd_id(&self) -> ScopeItemId {
        match self {
            ScopeListItem::Header { scope_id, .. } => ScopeItemId {
                scope_id: *scope_id,
                kind: ScopeItemKind::Header,
            },
            ScopeListItem::Trace { scope_id, trace } => ScopeItemId {
                scope_id: *scope_id,
                kind: ScopeItemKind::Trace(trace.0.clone()),
            },
        }
    }
}

pub struct TracesPanel {
    pub state: PanelState,
    pub look_editor_trace: Option<TraceRef>,
    pub hover_trace: Option<TraceRef>,
    pub dragging_trace: Option<TraceRef>,
}

impl Default for TracesPanel {
    fn default() -> Self {
        Self {
            state: PanelState::new("Traces", "📈"),
            look_editor_trace: None,
            hover_trace: None,
            dragging_trace: None,
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
        ui.label("Configure traces: marker selection, visibility, colors, offsets, Y axis options, and legend info.");

        let scope_meta: Vec<(usize, String)> = data
            .scope_data
            .iter()
            .map(|s| (s.id, s.name.clone()))
            .collect();

        if data.scope_data.is_empty() {
            ui.label("No scopes available.");
            return;
        }

        if !scope_meta.is_empty() {
            self.render_scope_assignments(ui, data, &scope_meta);
            ui.separator();
        }

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
            drag_out: &'a mut Option<TraceRef>,
            rows: Vec<Row>,
            col_w: [f32; 6],
        }
        impl<'a> TableDelegate for TracesDelegate<'a> {
            fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
                let col = cell.col_range.start;
                // Reserve exact width for this column and render content within
                let (rect, _resp) =
                    ui.allocate_exact_size(egui::vec2(self.col_w[col], 20.0), egui::Sense::hover());
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |inner| {
                        // Debug: actual header cell allocated width per column
                        let w = inner.max_rect().width();
                        LAST_COL_HDR_W.with(|arr| {
                            let mut a = arr.borrow_mut();
                            if (a[col] - w).abs() > 0.5 {
                                a[col] = w;
                                traces_debug!("[traces_ui] header col{} width={:.1}", col, w);
                            }
                        });
                        let text = match col {
                            0 => "",
                            1 => "Trace",
                            2 => "Visible",
                            3 => "Offset",
                            4 => "Info",
                            _ => "",
                        };
                        // Center certain headers; keep Name/Info left-aligned
                        let centered_cols = [0usize, 2, 3, 4];
                        if centered_cols.contains(&col) {
                            // Use a full-width centered-and-justified layout to ensure true horizontal centering
                            let layout =
                                egui::Layout::centered_and_justified(egui::Direction::LeftToRight);
                            inner.allocate_ui_with_layout(inner.max_rect().size(), layout, |ui2| {
                                ui2.strong(text);
                            });
                        } else {
                            inner.add_space(4.0);
                            inner.strong(text);
                        }
                    },
                );
            }
            fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
                let row = cell.row_nr as usize;
                let col = cell.col_nr;
                if row >= self.rows.len() {
                    return;
                }
                let r = &self.rows[row];
                // Reserve exact width and render within this rect
                let (rect, _resp) =
                    ui.allocate_exact_size(egui::vec2(self.col_w[col], 20.0), egui::Sense::hover());
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |inner| {
                        // Debug: first row cell allocated width per column
                        if row == 0 {
                            let w = inner.max_rect().width();
                            LAST_COL_ROW0_W.with(|arr| {
                                let mut a = arr.borrow_mut();
                                if (a[col] - w).abs() > 0.5 {
                                    a[col] = w;
                                    traces_debug!("[traces_ui] row0 col{} width={:.1}", col, w);
                                }
                            });
                        }
                        match col {
                            0 => {
                                // Color editor centered (moved from former column 5)
                                inner.with_layout(
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |cui| {
                                        if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                            let mut c = tr.look.color;
                                            let resp = cui
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
                            1 => {
                                inner.add_space(4.0);
                                let resp = inner.add(
                                    egui::Label::new(r.name.0.clone())
                                        .truncate()
                                        .show_tooltip_when_elided(true)
                                        .sense(egui::Sense::click_and_drag()),
                                );
                                if resp.hovered() {
                                    *self.hover_out = Some(r.name.clone());
                                }
                                if resp.drag_started() || resp.dragged() {
                                    *self.drag_out = Some(r.name.clone());
                                    inner
                                        .output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                                }
                                if resp.clicked() {
                                    *self.look_toggle = Some(r.name.clone());
                                }
                            }
                            2 => {
                                // Visible checkbox centered
                                inner.with_layout(
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |cui| {
                                        if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                            let mut vis = tr.look.visible;
                                            let resp = cui.checkbox(&mut vis, "");
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
                            3 => {
                                // Offset DragValue centered
                                inner.with_layout(
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |cui| {
                                        if let Some(tr) = self.traces.get_trace_mut(&r.name) {
                                            let mut off = tr.offset;
                                            let resp = cui.add(
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
                            4 => {
                                inner.add_space(4.0);
                                if let Some(tr) = self.traces.get_trace(&r.name) {
                                    let text = tr.info.clone();
                                    let resp = inner.add(
                                        egui::Label::new(text.clone())
                                            .truncate()
                                            .show_tooltip_when_elided(true)
                                            .sense(egui::Sense::click()),
                                    );
                                    if resp.hovered() {
                                        *self.hover_out = Some(r.name.clone());
                                    }
                                    if resp.clicked() {
                                        inner.ctx().copy_text(text.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    },
                );
            }
        }

        // Compute dynamic column widths
        // Policy:
        // - All columns have a minimum width (min_w)
        // - If available width is less than the sum of minima, we DO NOT shrink below minima;
        //   instead we enable horizontal scrolling so fixed columns never get smaller.
        // - If there's extra space, only Name (1) and Info (5) expand using weights.
        let avail_w = ui.available_width();
        let avail_w_f32: f32 = avail_w;
        // Preferred minima for each column [0..5]
        let min_w = [12.0, 70.0, 42.0, 42.0, 32.0, 300.0];
        let mut w = min_w;
        // Current total at minima
        let sum_min: f32 = w.iter().sum();
        let name_weight = 0.45_f32;
        let info_weight = 0.55_f32;
        let weight_sum = name_weight + info_weight;
        if avail_w_f32 > sum_min {
            // We have extra space beyond all minima: distribute only to Name/Info by weights
            let extra = avail_w_f32 - sum_min;
            w[1] = min_w[1] + extra * (name_weight / weight_sum);
            w[5] = min_w[5] + extra * (info_weight / weight_sum);
            // Optional: ensure we fill the available width exactly (avoid tiny rounding gaps)
            let sum_now: f32 = w.iter().sum();
            let delta = avail_w_f32 - sum_now;
            if delta.abs() > 0.5 {
                w[5] = (w[5] + delta).max(0.0);
            }
        } else {
            // Narrow panel: keep all columns at their minima; we'll scroll horizontally.
            // Intentionally do nothing here so total width remains sum_min.
        }

        let cols = vec![
            egui_table::Column::new(w[0]), // color edit
            egui_table::Column::new(w[1]), // name (stretches)
            egui_table::Column::new(w[2]), // marker
            egui_table::Column::new(w[3]), // visible
            egui_table::Column::new(w[4]), // offset
            egui_table::Column::new(w[5]), // info (stretches)
        ];
        // Compute a preferred height for the table; size it relative to available height
        let header_h = 24.0_f32;
        let row_h = 22.0_f32;
        let rows_len = rows.len() as f32;
        let editor_open = self.look_editor_trace.is_some();
        let preferred = header_h + row_h * rows_len + 8.0;
        let avail_h = ui.available_height();
        // With the editor placed below the table, give the table a larger share when open.
        let max_h = if editor_open {
            (avail_h * 0.65).max(200.0)
        } else {
            (avail_h * 0.85).max(200.0)
        };
        let table_h = preferred.clamp(120.0, max_h);

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Clone rows and draw the table first (style editor is rendered below)
                let rows_clone = rows.clone();
                {
                    let mut hover_tmp: Option<TraceRef> = None;
                    let mut look_toggle_req: Option<TraceRef> = None;
                    let mut drag_from_table: Option<TraceRef> = None;
                    // Borrow traces mutably for the table drawing scope only
                    let traces_ref = &mut *data.traces;
                    let mut delegate = TracesDelegate {
                        traces: traces_ref,
                        hover_out: &mut hover_tmp,
                        look_toggle: &mut look_toggle_req,
                        drag_out: &mut drag_from_table,
                        rows: rows_clone,
                        col_w: w,
                    };
                    let (rect, _resp) =
                        ui.allocate_exact_size(egui::vec2(avail_w, table_h), egui::Sense::hover());
                    let ui_builder = egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Min));
                    let mut table_ui = ui.new_child(ui_builder);
                    Table::new()
                        .id_salt(("traces_table", avail_w.to_bits()))
                        .num_rows(delegate.rows.len() as u64)
                        .columns(cols)
                        .headers(vec![EgHeaderRow::new(24.0)])
                        .show(&mut table_ui, &mut delegate);
                    // Write back hover and selection state after drawing table
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

                // Render inline style editor beneath the table
                if let Some(tn) = self.look_editor_trace.clone() {
                    if let Some(tr) = data.traces.get_trace_mut(&tn) {
                        ui.add_space(8.0);
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(format!("Style: {}", tn));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("✖ Close").clicked() {
                                            self.look_editor_trace = None;
                                        }
                                    },
                                );
                            });
                            ui.separator();
                            render_trace_look_editor(&mut tr.look, ui, true);
                        });
                    } else {
                        ui.label("Trace not found.");
                        self.look_editor_trace = None;
                    }
                }
            });

        if let Some(hover_trace) = self.hover_trace.clone() {
            data.traces.hover_trace = Some(hover_trace.clone());
        }

        if ui.ctx().input(|i| i.pointer.any_released()) {
            self.dragging_trace = None;
        }
    }
}

impl TracesPanel {
    fn render_scope_assignments(
        &mut self,
        ui: &mut Ui,
        data: &mut LivePlotData<'_>,
        scope_meta: &[(usize, String)],
    ) {
        let can_remove_scope = data.scope_data.len() > 1;
        if ui.button("➕ Add scope").clicked() {
            data.pending_requests.add_scope = true;
        }
        ui.add_space(4.0);

        let mut id_to_idx: HashMap<usize, usize> = HashMap::new();
        for (idx, scope) in data.scope_data.iter().enumerate() {
            id_to_idx.insert(scope.id, idx);
        }

        let mut items: Vec<ScopeListItem> = Vec::new();
        for (idx, (id, name)) in scope_meta.iter().enumerate() {
            items.push(ScopeListItem::Header {
                scope_id: *id,
                name: name.clone(),
            });
            if let Some(scope) = data.scope_data.get(idx) {
                for tr in scope.trace_order.iter() {
                    items.push(ScopeListItem::Trace {
                        scope_id: scope.id,
                        trace: tr.clone(),
                    });
                }
            }
        }

        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let pointer_released = ui.input(|i| i.pointer.any_released());
        let dragging = self.dragging_trace.clone();
        let mut drop_request: Option<(usize, TraceRef)> = None;
        let mut removed: HashSet<(usize, TraceRef)> = HashSet::new();

        dnd(ui, "scope_traces_dnd").show_custom_vec(&mut items, |ui, items, iter| {
            for (idx, item) in items.iter_mut().enumerate() {
                let draggable = matches!(item, ScopeListItem::Trace { .. });
                let id = Id::new(item.dnd_id());
                iter.next(ui, id, idx, draggable, |ui, item_handle| match item {
                    ScopeListItem::Header { name, scope_id } => {
                        item_handle.ui(ui, |ui, _handle, _state| {
                            ui.separator();
                            ui.add_space(4.0);
                            ui.heading(format!("Scope {}:", *scope_id + 1));
                            let resp = ui.horizontal(|ui| {
                                if ui.text_edit_singleline(name).changed() {
                                    if let Some(idx) = id_to_idx.get(scope_id) {
                                        if let Some(scope) = data.scope_data.get_mut(*idx) {
                                            scope.name = name.clone();
                                        }
                                    }
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let remove_enabled = can_remove_scope;
                                        if ui
                                            .add_enabled(
                                                remove_enabled,
                                                egui::Button::new("🗑 Remove"),
                                            )
                                            .on_hover_text("Remove this scope from layout")
                                            .clicked()
                                        {
                                            data.pending_requests.remove_scope = Some(*scope_id);
                                        }
                                    },
                                );
                            });
                            if let (Some(pos), Some(trace_name)) = (pointer_pos, dragging.clone()) {
                                if resp.response.rect.contains(pos) {
                                    ui.painter().rect_stroke(
                                        resp.response.rect.expand(2.0),
                                        egui::CornerRadius::same(2),
                                        ui.visuals().selection.stroke,
                                        egui::StrokeKind::Outside,
                                    );
                                    if drop_request.is_none() && pointer_released {
                                        drop_request = Some((*scope_id, trace_name));
                                    }
                                }
                            }
                        })
                    }
                    ScopeListItem::Trace { trace, scope_id } => {
                        item_handle.ui(ui, |ui, handle, _state| {
                            let resp = ui.horizontal(|ui| {
                                handle.ui(ui, |ui| {
                                    ui.label(DOTS_SIX_VERTICAL);
                                });
                                let name_resp = ui.add(
                                    egui::Label::new(trace.0.clone())
                                        .truncate()
                                        .show_tooltip_when_elided(true)
                                        .sense(egui::Sense::click()),
                                );
                                if name_resp.hovered() {
                                    data.traces.hover_trace = Some(trace.clone());
                                }
                                if let Some(tr_data) = data.traces.get_trace(trace) {
                                    let info = tr_data.info.clone();
                                    if !info.is_empty() {
                                        let info_resp = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(info.clone()).small().weak(),
                                            )
                                            .truncate()
                                            .show_tooltip_when_elided(true),
                                        );
                                        if info_resp.hovered() {
                                            data.traces.hover_trace = Some(trace.clone());
                                        }
                                    }
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("Remove")
                                            .on_hover_text("Remove from this scope")
                                            .clicked()
                                        {
                                            removed.insert((*scope_id, trace.clone()));
                                        }
                                    },
                                );
                            });
                            if resp.response.hovered() {
                                data.traces.hover_trace = Some(trace.clone());
                            }
                            if let (Some(pos), Some(trace_name)) = (pointer_pos, dragging.clone()) {
                                if resp.response.rect.contains(pos) {
                                    ui.painter().rect_stroke(
                                        resp.response.rect.expand(2.0),
                                        egui::CornerRadius::same(2),
                                        ui.visuals().selection.stroke,
                                        egui::StrokeKind::Outside,
                                    );
                                    if drop_request.is_none() && pointer_released {
                                        drop_request = Some((*scope_id, trace_name));
                                    }
                                }
                            }
                        })
                    }
                });
            }
        });

        let mut current_scope: Option<usize> = None;
        for item in items.iter_mut() {
            match item {
                ScopeListItem::Header { scope_id, .. } => current_scope = Some(*scope_id),
                ScopeListItem::Trace { scope_id, .. } => {
                    if let Some(cs) = current_scope {
                        *scope_id = cs;
                    }
                }
            }
        }

        let mut new_orders: Vec<Vec<TraceRef>> = vec![Vec::new(); data.scope_data.len()];

        for item in items.into_iter() {
            if let ScopeListItem::Trace { scope_id, trace } = item {
                if removed.contains(&(scope_id, trace.clone())) {
                    continue;
                }
                if let Some(idx) = id_to_idx.get(&scope_id) {
                    let slot = &mut new_orders[*idx];
                    if !slot.contains(&trace) {
                        slot.push(trace);
                    }
                }
            }
        }

        if let Some((scope_id, trace)) = drop_request {
            if let Some(idx) = id_to_idx.get(&scope_id) {
                let slot = &mut new_orders[*idx];
                if !slot.contains(&trace) {
                    slot.push(trace);
                }
            }
        }

        for (idx, traces) in new_orders.into_iter().enumerate() {
            if let Some(scope) = data.scope_data.get_mut(idx) {
                scope.trace_order = traces;
            }
        }
    }
}
