use eframe::egui;
use egui::Color32;
use egui_table::{Table, TableDelegate, HeaderRow as EgHeaderRow};

use super::app::ScopeAppMulti;

pub(super) fn show_traces_dialog(app: &mut ScopeAppMulti, ctx: &egui::Context) {
    let mut show_flag = app.show_traces_dialog;
    egui::Window::new("Traces").open(&mut show_flag).show(ctx, |ui| {
        ui.label("Configure traces: marker selection, visibility, colors, offsets, and Y axis options.");
        ui.separator();

        ui.horizontal(|ui| {
            let mut ylog = app.y_log;
            if ui.checkbox(&mut ylog, "Y axis log scale").on_hover_text("Use base-10 log of (value + offset). Non-positive values are omitted.").changed() {
                app.y_log = ylog;
            }
            ui.label("Y unit:");
            let mut unit = app.y_unit.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut unit).changed() {
                app.y_unit = if unit.trim().is_empty() { None } else { Some(unit) };
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
        rows.push(Row { name: "Free".to_string(), is_free: true });
        for n in app.trace_order.iter() { rows.push(Row { name: n.clone(), is_free: false }); }

        // Delegate for table rendering
        struct TracesDelegate<'a> { app: &'a mut ScopeAppMulti, rows: Vec<Row> }
        impl<'a> TableDelegate for TracesDelegate<'a> {
            fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
                let col = cell.col_range.start;
                let text = match col {
                    0 => "",
                    1 => "Trace",
                    2 => "Marker",
                    3 => "Visible",
                    4 => "Points",
                    5 => "Color",
                    6 => "Offset",
                    _ => "",
                };
                ui.add_space(4.0);
                ui.strong(text);
            }
            fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
                let row = cell.row_nr as usize;
                let col = cell.col_nr;
                if row >= self.rows.len() { return; }
                let r = &self.rows[row];
                ui.add_space(4.0);
                match col {
                    0 => {
                        // Color dot
                        if r.is_free { ui.label(""); }
                        else if let Some(tr) = self.app.traces.get(&r.name) {
                            let (w, h) = (12.0, 12.0);
                            let (rr, gg, bb, aa) = (tr.color.r(), tr.color.g(), tr.color.b(), tr.color.a());
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
                            ui.painter().rect_filled(rect, h*0.5, Color32::from_rgba_unmultiplied(rr, gg, bb, aa));
                            if resp.hovered() { if !r.is_free { self.app.hover_trace = Some(r.name.clone()); } }
                        }
                    }
                    1 => {
                        let resp = ui.add(egui::Label::new(&r.name).sense(egui::Sense::hover()));
                        if resp.hovered() { if !r.is_free { self.app.hover_trace = Some(r.name.clone()); } }
                    }
                    2 => {
                        // Marker radio: Free or exactly one name
                        let mut sel = self.app.selection_trace.clone();
                        let is_selected = (r.is_free && sel.is_none()) || (!r.is_free && sel.as_ref() == Some(&r.name));
                        if ui.selectable_label(is_selected, if r.is_free { "Free" } else { "Use" }).clicked() {
                            sel = if r.is_free { None } else { Some(r.name.clone()) };
                            self.app.selection_trace = sel;
                        }
                    }
                    3 => {
                        if r.is_free { ui.label(""); }
                        else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                            let mut vis = tr.visible;
                            if ui.checkbox(&mut vis, "").changed() { tr.visible = vis; }
                        }
                    }
                    4 => {
                        if r.is_free { ui.label(""); }
                        else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                            let mut sp = tr.show_points;
                            if ui.checkbox(&mut sp, "").on_hover_text("Show point markers").changed() { tr.show_points = sp; }
                        }
                    }
                    5 => {
                        if r.is_free { ui.label(""); }
                        else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                            let mut c = tr.color;
                            if ui.color_edit_button_srgba(&mut c).changed() {
                                tr.color = c;
                            }
                        }
                    }
                    6 => {
                        if r.is_free { ui.label(""); }
                        else if let Some(tr) = self.app.traces.get_mut(&r.name) {
                            let mut off = tr.offset;
                            let resp = ui.add(egui::DragValue::new(&mut off).speed(0.01).range(-1.0e12..=1.0e12));
                            if resp.changed() { tr.offset = off; }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Reset hover before drawing; cells will set it when hovered
        app.hover_trace = None;
        let mut delegate = TracesDelegate { app, rows };
        let cols = vec![
            egui_table::Column::new(26.0),  // color dot
            egui_table::Column::new(220.0), // name
            egui_table::Column::new(80.0),  // marker
            egui_table::Column::new(80.0),  // visible
            egui_table::Column::new(70.0),  // points toggle
            egui_table::Column::new(180.0), // color edit
            egui_table::Column::new(120.0), // offset
        ];
        let avail_w = ui.available_width();
        let table_h = 300.0;
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(avail_w, table_h), egui::Sense::hover());
        let ui_builder = egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min));
        let mut table_ui = ui.new_child(ui_builder);
        Table::new()
            .id_salt("traces_table")
            .num_rows(delegate.rows.len() as u64)
            .columns(cols)
            .headers(vec![EgHeaderRow::new(24.0)])
            .show(&mut table_ui, &mut delegate);
    });
    app.show_traces_dialog = show_flag;
}
