use eframe::egui;
use super::panel::{DockPanel, DockState};
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};
use std::cell::{Cell, RefCell};

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
    static LAST_AVAIL_W: Cell<f32> = Cell::new(0.0);
    static LAST_COL_HDR_W: RefCell<[f32; 6]> = RefCell::new([0.0; 6]);
    static LAST_COL_ROW0_W: RefCell<[f32; 6]> = RefCell::new([0.0; 6]);
}

use super::app::ScopeAppMulti;

pub(super) fn traces_panel_contents(app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
    ui.label("Configure traces: marker selection, visibility, colors, offsets, Y axis options, and legend info.");
    ui.horizontal(|ui| {
        let mut v = app.show_info_in_legend;
        if ui
            .checkbox(&mut v, "Show info in Legend")
            .on_hover_text("Append each trace's info text to its legend label")
            .changed()
        {
            app.show_info_in_legend = v;
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        let mut ylog = app.y_log;
        if ui
            .checkbox(&mut ylog, "Y axis log scale")
            .on_hover_text("Use base-10 log of (value + offset). Non-positive values are omitted.")
            .changed()
        {
            app.y_log = ylog;
        }
        ui.label("Y unit:");
        let mut unit = app.y_unit.clone().unwrap_or_default();
        if ui.text_edit_singleline(&mut unit).changed() {
            app.y_unit = if unit.trim().is_empty() {
                None
            } else {
                Some(unit)
            };
        }
    });
    ui.separator();

    // Build rows: include a synthetic "Free" row for marker selection only
    #[derive(Clone)]
    struct Row {
        name: String,
        is_free: bool,
    }
    let mut rows: Vec<Row> = Vec::new();
    rows.push(Row {
        name: "Free".to_string(),
        is_free: true,
    });
    for n in app.trace_order.iter() {
        rows.push(Row {
            name: n.clone(),
            is_free: false,
        });
    }

    // Delegate for table rendering
    struct TracesDelegate<'a> {
        app: &'a mut ScopeAppMulti,
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
                        2 => "Marker",
                        3 => "Visible",
                        4 => "Offset",
                        5 => "Info",
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
                                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                                |cui| {
                                    if r.is_free {
                                        cui.label("");
                                    } else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                                        let mut c = tr.look.color;
                                        let resp = cui
                                            .color_edit_button_srgba(&mut c)
                                            .on_hover_text("Change trace color");
                                        if resp.hovered() {
                                            self.app.hover_trace = Some(r.name.clone());
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
                                egui::Label::new(&r.name)
                                    .truncate()
                                    .show_tooltip_when_elided(true)
                                    .sense(egui::Sense::click()),
                            );
                            if resp.hovered() {
                                if !r.is_free {
                                    self.app.hover_trace = Some(r.name.clone());
                                }
                            }
                            if resp.clicked() {
                                if !r.is_free {
                                    let cur = self.app.traces_panel.look_editor_trace.clone();
                                    self.app.traces_panel.look_editor_trace = if cur.as_deref() == Some(&r.name) { None } else { Some(r.name.clone()) };
                                    // Clear hover so highlight doesn't obscure editor
                                    self.app.hover_trace = None;
                                }
                            }
                        }
                        2 => {
                            // Marker radio centered: Free or exactly one name
                            inner.with_layout(
                                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                                |cui| {
                                    let mut sel = self.app.selection_trace.clone();
                                    let is_selected = (r.is_free && sel.is_none())
                                        || (!r.is_free && sel.as_ref() == Some(&r.name));
                                    let resp = cui.selectable_label(
                                        is_selected,
                                        if r.is_free { "Free" } else { "Use" },
                                    );
                                    if resp.hovered() && !r.is_free {
                                        self.app.hover_trace = Some(r.name.clone());
                                    }
                                    if resp.clicked() {
                                        sel = if r.is_free {
                                            None
                                        } else {
                                            Some(r.name.clone())
                                        };
                                        self.app.selection_trace = sel;
                                    }
                                },
                            );
                        }
                        3 => {
                            // Visible checkbox centered
                            inner.with_layout(
                                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                                |cui| {
                                    if r.is_free {
                                        cui.label("");
                                    } else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                                        let mut vis = tr.look.visible;
                                        let resp = cui.checkbox(&mut vis, "");
                                        if resp.hovered() {
                                            self.app.hover_trace = Some(r.name.clone());
                                        }
                                        if resp.changed() {
                                            tr.look.visible = vis;
                                        }
                                    }
                                },
                            );
                        }
                        4 => {
                            // Offset DragValue centered
                            inner.with_layout(
                                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                                |cui| {
                                    if r.is_free {
                                        cui.label("");
                                    } else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                                        let mut off = tr.offset;
                                        let resp = cui.add(
                                            egui::DragValue::new(&mut off)
                                                .speed(0.01)
                                                .range(-1.0e12..=1.0e12),
                                        );
                                        if resp.hovered() {
                                            self.app.hover_trace = Some(r.name.clone());
                                        }
                                        if resp.changed() {
                                            tr.offset = off;
                                        }
                                    }
                                },
                            );
                        }
                        5 => {
                            inner.add_space(4.0);
                            if r.is_free {
                                inner.label("");
                            } else if let Some(tr) = self.app.traces.get(&r.name) {
                                let text = tr.info.clone();
                                let resp = inner.add(
                                    egui::Label::new(text.clone())
                                        .truncate()
                                        .show_tooltip_when_elided(true)
                                        .sense(egui::Sense::click()),
                                );
                                if resp.hovered() {
                                    self.app.hover_trace = Some(r.name.clone());
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

    // // Debug: print when width changes notably
    // LAST_AVAIL_W.with(|c| {
    //     let prev = c.get();
    //     if (avail_w_f32 - prev).abs() > 1.0 {
    //         c.set(avail_w_f32);
    //         traces_debug!(
    //             "[traces_ui] avail_w={:.1} sum_min={:.1} widths={:?}",
    //             avail_w_f32,
    //             min_w.iter().sum::<f32>(),
    //             &w
    //         );
    //     }
    // });

    // Reset hover before drawing; cells will set it when hovered
    app.hover_trace = None;

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
    let editor_open = app.traces_panel.look_editor_trace.is_some();
    let preferred = header_h + row_h * rows_len + 8.0;
    let avail_h = ui.available_height();
    // With the editor placed below the table, give the table a larger share when open.
    let max_h = if editor_open { (avail_h * 0.65).max(200.0) } else { (avail_h * 0.85).max(200.0) };
    let table_h = preferred.clamp(120.0, max_h);

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            // Clone rows and draw the table first (style editor is rendered below)
            let rows_clone = rows.clone();
            {
                let mut delegate = TracesDelegate { app, rows: rows_clone, col_w: w };
                let (rect, _resp) = ui.allocate_exact_size(
                    egui::vec2(avail_w, table_h),
                    egui::Sense::hover(),
                );
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
            }

            // Render inline style editor beneath the table
            if let Some(tn) = app.traces_panel.look_editor_trace.clone() {
                if let Some(tr) = app.traces.get_mut(&tn) {
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.strong(format!("Style: {}", tn));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Close").clicked() {
                                    app.traces_panel.look_editor_trace = None;
                                }
                            });
                        });
                        ui.separator();
                        tr.look.render_editor(ui, true, None, false, None);
                    });
                } else {
                    ui.label("Trace not found.");
                    app.traces_panel.look_editor_trace = None;
                }
            }
        });
}

// Removed unused show_traces_dialog helper; dialogs are shown via DockPanel::show_detached_dialog

#[derive(Debug, Clone)]
pub struct TracesPanel {
    pub dock: DockState,
    pub look_editor_trace: Option<String>,
}

impl Default for TracesPanel {
    fn default() -> Self {
        Self { dock: DockState::new("📈 Traces"), look_editor_trace: None }
    }
}

impl DockPanel for TracesPanel {
    fn dock_mut(&mut self) -> &mut DockState { &mut self.dock }
    fn panel_contents(&mut self, app: &mut ScopeAppMulti, ui: &mut egui::Ui) {
        traces_panel_contents(app, ui);
    }
}
