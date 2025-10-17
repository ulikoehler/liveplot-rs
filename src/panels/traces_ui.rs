use egui::Ui;
use egui_table::{HeaderRow as EgHeaderRow, Table, TableDelegate};
use super::panel_trait::{Panel, PanelState};
use super::trace_look_ui::render_trace_look_editor;
use crate::data::DataContext;

pub struct TracesPanel {
    pub state: PanelState,
    pub look_editor_trace: Option<String>,
}
impl Default for TracesPanel { fn default() -> Self { Self { state: PanelState { visible: true, detached: false }, look_editor_trace: None } } }
impl Panel for TracesPanel {
    fn name(&self) -> &'static str { "Traces" }
    fn state(&self) -> &PanelState { &self.state }
    fn state_mut(&mut self) -> &mut PanelState { &mut self.state }
    fn render_panel(&mut self, ui: &mut Ui, data: &mut DataContext) {
        ui.label("Configure traces: marker selection, visibility, colors, offsets, Y axis options, and legend info.");
        ui.horizontal(|ui| {
            let mut v = data.traces.show_info_in_legend;
            if ui.checkbox(&mut v, "Show info in Legend").on_hover_text("Append each trace's info text to its legend label").changed() { data.traces.show_info_in_legend = v; }
        });
        ui.separator();

        ui.horizontal(|ui| {
            let mut ylog = data.traces.y_log;
            if ui.checkbox(&mut ylog, "Y axis log scale").on_hover_text("Use base-10 log of (value + offset). Non-positive values are omitted.").changed() { data.traces.y_log = ylog; }
            ui.label("Y unit:");
            let mut unit = data.traces.y_unit.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut unit).changed() { data.traces.y_unit = if unit.trim().is_empty() { None } else { Some(unit) }; }
        });
        ui.separator();

        // Build rows including a synthetic "Free" row for marker selection only
        #[derive(Clone)]
        struct Row { name: String, is_free: bool }
        let mut rows: Vec<Row> = Vec::new();
        rows.push(Row { name: "Free".to_string(), is_free: true });
        for n in data.traces.trace_order.iter() { rows.push(Row { name: n.clone(), is_free: false }); }

        // Table delegate that renders headers and cells
        struct TracesDelegate<'a> {
            data: &'a mut DataContext,
            rows: Vec<Row>,
            col_w: [f32; 6],
            look_editor_trace: &'a mut Option<String>,
        }
        impl<'a> TableDelegate for TracesDelegate<'a> {
            fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
                let col = cell.col_range.start;
                let (rect, _resp) = ui.allocate_exact_size(egui::vec2(self.col_w[col], 20.0), egui::Sense::hover());
                let builder = egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight));
                let mut inner = ui.new_child(builder);
                let text = match col { 0 => "", 1 => "Trace", 2 => "Marker", 3 => "Visible", 4 => "Offset", 5 => "Info", _ => "" };
                if !text.is_empty() { inner.strong(text); }
            }
            fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
                let row = cell.row_nr as usize;
                let col = cell.col_nr;
                if row >= self.rows.len() { return; }
                let r = &self.rows[row];
                let (rect, _resp) = ui.allocate_exact_size(egui::vec2(self.col_w[col], 20.0), egui::Sense::click());
                let builder = egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight));
                let mut inner = ui.new_child(builder);
                match col {
                    0 => {
                        if r.is_free { inner.label(""); }
                        else if let Some(tr) = self.data.traces.traces.get_mut(&r.name) {
                            let mut c = tr.look.color;
                            if inner.color_edit_button_srgba(&mut c).on_hover_text("Change trace color").changed() { tr.look.color = c; }
                        }
                    }
                    1 => {
                        let resp = inner.add(egui::Label::new(&r.name).truncate().show_tooltip_when_elided(true).sense(egui::Sense::click()));
                        if resp.clicked() { if !r.is_free { let cur = self.look_editor_trace.clone(); *self.look_editor_trace = if cur.as_deref() == Some(&r.name) { None } else { Some(r.name.clone()) }; } }
                    }
                    2 => {
                        let mut sel = self.data.traces.selection_trace.clone();
                        let is_selected = (r.is_free && sel.is_none()) || (!r.is_free && sel.as_ref() == Some(&r.name));
                        let resp = inner.selectable_label(is_selected, if r.is_free { "Free" } else { "Use" });
                        if resp.clicked() { sel = if r.is_free { None } else { Some(r.name.clone()) }; self.data.traces.selection_trace = sel; }
                    }
                    3 => {
                        if r.is_free { inner.label(""); }
                        else if let Some(tr) = self.data.traces.traces.get_mut(&r.name) { let mut vis = tr.look.visible; if inner.checkbox(&mut vis, "").changed() { tr.look.visible = vis; } }
                    }
                    4 => {
                        if r.is_free { inner.label(""); }
                        else if let Some(tr) = self.data.traces.traces.get_mut(&r.name) { let mut off = tr.offset; if inner.add(egui::DragValue::new(&mut off).speed(0.01).range(-1.0e12..=1.0e12)).changed() { tr.offset = off; } }
                    }
                    5 => {
                        if r.is_free { inner.label(""); }
                        else if let Some(tr) = self.data.traces.traces.get(&r.name) {
                            let text = tr.info.clone();
                            let resp = inner.add(egui::Label::new(text.clone()).truncate().show_tooltip_when_elided(true).sense(egui::Sense::click()));
                            if resp.clicked() { inner.ctx().copy_text(text.clone()); }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Compute dynamic column widths
        let avail_w = ui.available_width();
        // Make the color column wide enough to fit the color button cleanly
        let min_color_w = (ui.spacing().interact_size.y + 6.0).ceil().max(18.0);
        let min_w = [min_color_w, 80.0, 48.0, 48.0, 64.0, 260.0];
        let mut w = min_w;
        let sum_min: f32 = w.iter().sum();
        let name_weight = 0.45_f32;
        let info_weight = 0.55_f32;
        let weight_sum = name_weight + info_weight;
        if avail_w > sum_min {
            let extra = avail_w - sum_min;
            // Only Trace (1) and Info (5) stretch with available space
            w[1] = min_w[1] + extra * (name_weight / weight_sum);
            w[5] = min_w[5] + extra * (info_weight / weight_sum);
        }

        let cols = vec![
            egui_table::Column::new(w[0]),
            egui_table::Column::new(w[1]),
            egui_table::Column::new(w[2]),
            egui_table::Column::new(w[3]),
            egui_table::Column::new(w[4]),
            egui_table::Column::new(w[5]),
        ];

        let header_h = 24.0_f32;
        let row_h = 22.0_f32;
        let rows_len = rows.len() as f32;
        let editor_open = self.look_editor_trace.is_some();
        let preferred = header_h + row_h * rows_len + 8.0;
        let avail_h = ui.available_height();
        let max_h = if editor_open { (avail_h * 0.65).max(200.0) } else { (avail_h * 0.85).max(200.0) };
        let table_h = preferred.clamp(120.0, max_h);

        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            // draw the table
            let (rect, _resp) = ui.allocate_exact_size(egui::vec2(avail_w, table_h), egui::Sense::hover());
            let ui_builder = egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Min));
            let mut table_ui = ui.new_child(ui_builder);
            let mut delegate = TracesDelegate { data, rows: rows.clone(), col_w: w, look_editor_trace: &mut self.look_editor_trace };
            Table::new()
                .id_salt(("traces_table", avail_w.to_bits()))
                .num_rows(delegate.rows.len() as u64)
                .columns(cols)
                .headers(vec![EgHeaderRow::new(24.0)])
                .show(&mut table_ui, &mut delegate);

            // Inline style editor beneath
            if let Some(tn) = self.look_editor_trace.clone() {
                if let Some(tr) = data.traces.traces.get_mut(&tn) {
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.strong(format!("Style: {}", tn));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Close").clicked() { self.look_editor_trace = None; }
                            });
                        });
                        ui.separator();
                        render_trace_look_editor(&mut tr.look, ui, true);
                    });
                } else {
                    self.look_editor_trace = None;
                }
            }
        });
    }
}
