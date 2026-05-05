use crate::data::data::LivePlotRequests;
use crate::data::scope::{AxisType, ScopeData, ScopeType, XDateFormat};
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;
use crate::data::traces::TracesCollection;
use eframe::egui;
use egui::{Color32, Id, Ui};
use egui_dnd::dnd;
use egui_phosphor::regular::DOTS_SIX_VERTICAL;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Clone, Debug)]
pub struct DragPayload {
    pub trace: TraceRef,
    pub origin_scope_id: Option<usize>,
}

pub struct ScopeSettingsResponse {
    pub type_changed: bool,
    pub scope_changed: bool,
    pub moved_from_scope: Option<(usize, TraceRef)>,
}

#[derive(Default)]
pub struct ScopeSettingsUiPanel {
    renaming_scope_id: Option<usize>,
    rename_buffer: String,
    rename_focus_scope: Option<usize>,
    // Used to keep the scope settings panel width stable when switching to XY mode.
    last_time_scope_width: HashMap<usize, f32>,
}

impl ScopeSettingsUiPanel {
    fn render_scope_settings(
        &mut self,
        ui: &mut Ui,
        scope: &mut ScopeData,
        can_remove_scope: bool,
        pending: &mut LivePlotRequests,
    ) -> ScopeSettingsResponse {
        let scope_id = scope.id;
        let prev_type = scope.scope_type;

        let is_renaming = self.renaming_scope_id == Some(scope_id);

        ui.horizontal(|ui| {
            if is_renaming {
                if self.rename_buffer.is_empty() {
                    self.rename_buffer = scope.name.clone();
                }

                let edit_id = Id::new(("scope_rename", scope_id));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.rename_buffer)
                        .id(edit_id)
                        .font(egui::TextStyle::Heading)
                        .desired_width(240.0),
                );

                if self.rename_focus_scope == Some(scope_id) {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                    self.rename_focus_scope = None;
                }

                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let has_focus = ui.memory(|m| m.has_focus(edit_id));
                let commit = (enter && has_focus) || r.lost_focus();
                if commit {
                    let new_name = self.rename_buffer.trim();
                    if !new_name.is_empty() {
                        scope.name = new_name.to_string();
                    }
                    self.renaming_scope_id = None;
                    self.rename_buffer.clear();
                }
            } else {
                let head_resp = ui.add(
                    egui::Label::new(egui::RichText::new(scope.name.clone()).heading())
                        .truncate()
                        .show_tooltip_when_elided(true)
                        .sense(egui::Sense::click()),
                );
                if head_resp.double_clicked() {
                    self.renaming_scope_id = Some(scope_id);
                    self.rename_buffer = scope.name.clone();
                    self.rename_focus_scope = Some(scope_id);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(can_remove_scope, egui::Button::new("🗑"))
                    .on_hover_text("Remove this scope from layout")
                    .clicked()
                {
                    pending.remove_scope = Some(scope_id);
                }

                if ui.small_button("✏").on_hover_text("Rename").clicked() {
                    self.renaming_scope_id = Some(scope_id);
                    self.rename_buffer = scope.name.clone();
                    self.rename_focus_scope = Some(scope_id);
                }
            });
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut scope.show_legend, "Legend")
                .on_hover_text("Show the plot legend");
            if !scope.show_legend {
                scope.show_info_in_legend = false;
            }
            ui.add_enabled_ui(scope.show_legend, |ui| {
                ui.checkbox(&mut scope.show_info_in_legend, "Info")
                    .on_hover_text("Append each trace's info text to its legend label");
            });
        });

        ui.horizontal(|ui| {
            let time_sel = scope.scope_type == ScopeType::TimeScope;
            if ui
                .selectable_label(time_sel, "Time-Scope")
                .on_hover_text("Time window / scrolling time axis")
                .clicked()
            {
                scope.scope_type = ScopeType::TimeScope;
                scope.x_axis.axis_type = AxisType::Time(XDateFormat::default());
                scope.x_axis.name = Some("Time".to_string());
                // Ensure X formatter follows the axis type so hover/readouts/traces
                // render the X value sensibly for time scopes.
                scope.x_axis.x_formatter = crate::data::x_formatter::XFormatter::Auto;
            }

            let xy_sel = scope.scope_type == ScopeType::XYScope;
            if ui
                .selectable_label(xy_sel, "XY-Scope")
                .on_hover_text("X/Y plot (trace pairing)")
                .clicked()
            {
                scope.scope_type = ScopeType::XYScope;
                scope.x_axis.axis_type = AxisType::Value(None);
                scope.x_axis.name = Some("X".to_string());
                if scope.y_axis.name.is_none() {
                    scope.y_axis.name = Some("Y".to_string());
                }
                // Make sure X formatter auto-selects a decimal formatter for XY scopes
                scope.x_axis.x_formatter = crate::data::x_formatter::XFormatter::Auto;
            }
        });

        ScopeSettingsResponse {
            type_changed: prev_type != scope.scope_type,
            scope_changed: false,
            moved_from_scope: None,
        }
    }

    fn reorder_trace_order_pairs_first(scope: &mut ScopeData) {
        let mut ordered: Vec<TraceRef> = Vec::new();
        for (x, y, _look) in scope.xy_pairs.iter() {
            let (Some(x), Some(y)) = (x.as_ref(), y.as_ref()) else {
                continue;
            };
            ordered.push(x.clone());
            ordered.push(y.clone());
        }
        for t in scope.trace_order.iter() {
            if !ordered.contains(t) {
                ordered.push(t.clone());
            }
        }
        scope.trace_order = ordered;
    }

    fn rebuild_xy_pairs_from_trace_order(scope: &mut ScopeData, traces: &TracesCollection) {
        let mut existing: HashMap<(TraceRef, TraceRef), TraceLook> = HashMap::new();
        for (x, y, look) in scope.xy_pairs.iter() {
            let (Some(x), Some(y)) = (x.as_ref(), y.as_ref()) else {
                continue;
            };
            existing.insert((x.clone(), y.clone()), look.clone());
        }

        let traces_in_order = scope.trace_order.clone();
        let mut rebuilt: Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)> = Vec::new();

        let mut i = 0usize;
        while i < traces_in_order.len() {
            let x = traces_in_order[i].clone();
            let y = traces_in_order.get(i + 1).cloned();

            let look = if let Some(y) = y.as_ref() {
                existing
                    .get(&(x.clone(), y.clone()))
                    .cloned()
                    .or_else(|| traces.get_trace(&x).map(|t| t.look.clone()))
                    .unwrap_or_else(TraceLook::default)
            } else {
                traces
                    .get_trace(&x)
                    .map(|t| t.look.clone())
                    .unwrap_or_else(TraceLook::default)
            };

            rebuilt.push((Some(x), y, look));
            i += 2;
        }
        scope.xy_pairs = rebuilt;
    }

    fn rebuild_trace_order_from_xy_pairs(scope: &mut ScopeData) {
        let mut ordered: Vec<TraceRef> = Vec::new();
        for (x, y, _look) in scope.xy_pairs.iter() {
            if let Some(x) = x.as_ref() {
                ordered.push(x.clone());
            }
            if let Some(y) = y.as_ref() {
                ordered.push(y.clone());
            }
        }
        for t in scope.trace_order.iter() {
            if !ordered.contains(t) {
                ordered.push(t.clone());
            }
        }
        scope.trace_order = ordered;
    }

    fn render_drop_slot(
        ui: &mut Ui,
        id_salt: impl Hash,
        dragging: Option<&DragPayload>,
        drag_active: bool,
        label: &str,
    ) -> Option<DragPayload> {
        let _id = Id::new(("drop_slot", id_salt));
        let ctx = ui.ctx().clone();

        let h = if drag_active { 18.0 } else { 14.0 };
        let (rect, _resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());

        let visuals = ui.visuals();
        let stroke = visuals.widgets.inactive.bg_stroke;
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            egui::CornerRadius::same(2),
            stroke,
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );

        if let Some(pointer) = ctx.pointer_latest_pos() {
            if drag_active && rect.contains(pointer) {
                ui.painter().rect_filled(
                    rect.shrink2(egui::Vec2::splat(1.0)),
                    egui::CornerRadius::same(2),
                    visuals.selection.bg_fill,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    visuals.strong_text_color(),
                );
                ctx.request_repaint();
                if let Some(payload) = dragging {
                    if ui.input(|i| i.pointer.any_released()) {
                        return Some(payload.clone());
                    }
                }
            }
        }

        None
    }

    fn render_trace_list(
        ui: &mut Ui,
        id_salt: impl Hash,
        title: &str,
        global_dragging: &mut Option<DragPayload>,
        origin_scope_id: Option<usize>,
        traces_collection: &mut TracesCollection,
        traces: &mut Vec<Option<TraceRef>>,
        empty_label: &str,
        preserve_slots: bool,
        color_chooser: Option<Vec<&mut Color32>>,
        open_look_editor: Option<&mut usize>,
    ) -> (bool, Option<(usize, DragPayload)>) {
        let before = traces.clone();
        let mut removed: HashSet<TraceRef> = HashSet::new();
        let mut pending_place: Option<(usize, DragPayload)> = None;
        let mut dropped_payload: Option<(usize, DragPayload)> = None;

        // We support dragging from two sources:
        // - the main traces table (sets `global_dragging`)
        // - a scope list itself (we populate `global_dragging` from ItemState::dragged)
        let mut drag_active = global_dragging.is_some();
        let mut dragging_payload: Option<DragPayload> = global_dragging.clone();

        let mut color_chooser = color_chooser;
        let mut open_look_editor = open_look_editor;

        ui.vertical(|ui| {
            ui.strong(title);
            ui.separator();

            let dnd_id = Id::new(("trace_list_dnd", &id_salt));
            let dnd_resp = dnd(ui, dnd_id).show_custom_vec(traces, |ui, traces, iter| {
                for (idx, tr) in traces.iter_mut().enumerate() {
                    let is_draggable = tr.is_some();

                    // Stable ID for actual traces; use (idx) for None placeholders.
                    let row_id = match tr.as_ref() {
                        Some(t) => Id::new(("trace_list_row", &id_salt, t.0.clone())),
                        None => Id::new(("trace_list_row_none", &id_salt, idx)),
                    };

                    iter.next(ui, row_id, idx, is_draggable, |ui, item_handle| {
                        item_handle.ui(ui, |ui, handle, state| {
                            if state.dragged {
                                if let Some(t) = tr.as_ref() {
                                    let p = DragPayload {
                                        trace: t.clone(),
                                        origin_scope_id,
                                    };
                                    dragging_payload = Some(p.clone());
                                    *global_dragging = Some(p);
                                    drag_active = true;
                                }
                            }

                            match tr {
                                Some(t) => {
                                    ui.horizontal(|ui| {
                                        handle.ui(ui, |ui| {
                                            ui.label(DOTS_SIX_VERTICAL);
                                        });

                                        if let Some(colors) = color_chooser.as_mut() {
                                            if let Some(slot_color) = colors.get_mut(idx) {
                                                ui.color_edit_button_srgba(*slot_color)
                                                    .on_hover_text("Change color");
                                            }
                                        }

                                        let resp = ui.add(
                                            egui::Label::new(t.0.clone())
                                                .truncate()
                                                .show_tooltip_when_elided(true),
                                        );
                                        if resp.hovered() {
                                            traces_collection.hover_trace = Some(t.clone());
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("🗑")
                                                    .on_hover_text("Remove")
                                                    .clicked()
                                                {
                                                    removed.insert(t.clone());
                                                }
                                                if open_look_editor.is_some() {
                                                    if ui
                                                        .small_button("🎨")
                                                        .on_hover_text("Edit trace style")
                                                        .clicked()
                                                    {
                                                        if let Some(open_out) =
                                                            open_look_editor.as_mut()
                                                        {
                                                            **open_out = idx;
                                                        }
                                                    }
                                                }
                                            },
                                        );
                                    });
                                }
                                None => {
                                    let dragging = dragging_payload.as_ref();
                                    if let Some(dropped) = Self::render_drop_slot(
                                        ui,
                                        ("slot", &id_salt, idx),
                                        dragging,
                                        drag_active,
                                        empty_label,
                                    ) {
                                        pending_place = Some((idx, dropped.clone()));
                                        dropped_payload = Some((idx, dropped.clone()));
                                        *global_dragging = None;
                                        *tr = Some(dropped.trace.clone());
                                    }
                                }
                            }
                        })
                    });
                }
            });

            if dnd_resp.is_dragging() {
                ui.ctx().request_repaint();
                drag_active = true;
            }

            let dragging = dragging_payload.as_ref();
            if let Some(t) = Self::render_drop_slot(
                ui,
                ("drop_end", &id_salt),
                dragging,
                drag_active,
                empty_label,
            ) {
                let idx = traces.len();
                dropped_payload = Some((idx, t.clone()));
                *global_dragging = None;

                for x in traces.iter_mut() {
                    if x.as_ref() == Some(&t.trace) {
                        *x = None;
                    }
                }
                traces.push(Some(t.trace.clone()));
            }
        });

        if !removed.is_empty() {
            if preserve_slots {
                for t in traces.iter_mut() {
                    if let Some(existing) = t.as_ref() {
                        if removed.contains(existing) {
                            *t = None;
                        }
                    }
                }
            } else {
                traces.retain(|t| t.as_ref().is_none_or(|t| !removed.contains(t)));
            }
        }

        if let Some((idx, dropped)) = pending_place {
            for (j, other) in traces.iter_mut().enumerate() {
                if j != idx && other.as_ref() == Some(&dropped.trace) {
                    *other = None;
                }
            }
        }

        if drag_active {
            if let Some(p) = dragging_payload {
                *global_dragging = Some(p);
            }
        }

        (*traces != before, dropped_payload)
    }

    /// Render a complete scope assignment block: header/settings + either trace list (time) or
    /// X/Y pairing UI (XY).
    pub fn render_scope_assignment(
        &mut self,
        ui: &mut Ui,
        scope: &mut ScopeData,
        can_remove_scope: bool,
        pending: &mut LivePlotRequests,
        global_dragging: &mut Option<DragPayload>,
        traces_collection: &mut TracesCollection,
        look_editor_out: &mut Option<TraceRef>,
        xy_pair_look_editor_out: &mut Option<(usize, usize)>,
    ) -> ScopeSettingsResponse {
        let mut resp = self.render_scope_settings(ui, scope, can_remove_scope, pending);
        let mut scope_changed = false;

        if resp.type_changed {
            match scope.scope_type {
                ScopeType::XYScope => {
                    Self::rebuild_xy_pairs_from_trace_order(scope, traces_collection);
                }
                ScopeType::TimeScope => {
                    Self::rebuild_trace_order_from_xy_pairs(scope);
                }
            }
            Self::reorder_trace_order_pairs_first(scope);
        }

        match scope.scope_type {
            ScopeType::TimeScope => {
                self.last_time_scope_width
                    .insert(scope.id, ui.available_width());
                let mut tmp: Vec<Option<TraceRef>> =
                    scope.trace_order.iter().cloned().map(Some).collect();

                // Build a trace→color map keyed by TraceRef so that removals
                // and reorders inside render_trace_list don't misalign colors.
                let color_map: HashMap<TraceRef, Color32> = tmp
                    .iter()
                    .filter_map(|t| t.as_ref())
                    .filter_map(|t| {
                        traces_collection
                            .get_trace(t)
                            .map(|tr| (t.clone(), tr.look.color))
                    })
                    .collect();

                let mut colors: Vec<Color32> = tmp
                    .iter()
                    .map(|t| {
                        t.as_ref()
                            .and_then(|t| color_map.get(t).copied())
                            .unwrap_or(Color32::WHITE)
                    })
                    .collect();

                // Snapshot the trace order before rendering so we can map
                // color-chooser edits (indexed on the original order) back
                // to traces by identity after render_trace_list may have
                // removed or reordered entries.
                let pre_render_order: Vec<Option<TraceRef>> = tmp.clone();

                let color_refs: Vec<&mut Color32> = colors.iter_mut().collect();

                let mut open_editor_idx = usize::MAX;

                let (changed, dropped) = Self::render_trace_list(
                    ui,
                    ("time_scope", scope.id),
                    "Trace",
                    global_dragging,
                    Some(scope.id),
                    traces_collection,
                    &mut tmp,
                    "Drop trace here",
                    false,
                    Some(color_refs),
                    Some(&mut open_editor_idx),
                );

                // Apply edited colors back by TraceRef identity.
                // The colors vec is indexed by the *pre-render* order
                // (before any removals), so pair each color with the
                // trace that was at that position before rendering.
                let edited_colors: HashMap<TraceRef, Color32> = pre_render_order
                    .iter()
                    .zip(colors.iter())
                    .filter_map(|(t, &c)| t.as_ref().map(|t| (t.clone(), c)))
                    .collect();
                for t in tmp.iter().flatten() {
                    if let Some(&color) = edited_colors.get(t) {
                        if let Some(tr_state) = traces_collection.get_trace_mut(t) {
                            tr_state.look.color = color;
                        }
                    }
                }

                if open_editor_idx != usize::MAX {
                    if let Some(Some(tr)) = tmp.get(open_editor_idx) {
                        *xy_pair_look_editor_out = None;
                        *look_editor_out = Some(tr.clone());
                    }
                }

                if changed {
                    scope.trace_order = tmp.into_iter().flatten().collect();
                }
                scope_changed |= changed;

                if let Some(dropped) = dropped {
                    if let Some(src) = dropped.1.origin_scope_id {
                        if src != scope.id {
                            resp.moved_from_scope = Some((src, dropped.1.trace));
                        }
                    }
                }
            }
            ScopeType::XYScope => {
                let target_w = self
                    .last_time_scope_width
                    .get(&scope.id)
                    .copied()
                    .unwrap_or_else(|| ui.available_width())
                    .min(ui.available_width());

                // Keep your per-column lists, but make them actually apply changes back.
                // Note: empty TraceRef ("") is treated as None.

                let mut x_vec: Vec<Option<TraceRef>> =
                    scope.xy_pairs.iter().map(|(x, _, _)| x.clone()).collect();
                let mut y_vec: Vec<Option<TraceRef>> =
                    scope.xy_pairs.iter().map(|(_, y, _)| y.clone()).collect();
                let mut look_vec: Vec<TraceLook> =
                    scope.xy_pairs.iter().map(|(_, _, l)| l.clone()).collect();

                let x_before = x_vec.clone();
                let y_before = y_vec.clone();

                ui.allocate_ui(egui::vec2(target_w, 0.0), |ui| {
                    ui.set_width(target_w);
                    // Two fixed-width columns to avoid the panel expanding in XY mode.
                    let col_w = ((target_w - 8.0) / 2.0).max(120.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.allocate_ui(egui::vec2(col_w, 0.0), |ui| {
                            let mut colors: Vec<Color32> =
                                look_vec.iter().map(|l| l.color).collect();
                            let color_refs: Vec<&mut Color32> = colors.iter_mut().collect();

                            let (changed, dropped) = Self::render_trace_list(
                                ui,
                                ("xy_scope_x", scope.id),
                                "X Traces",
                                global_dragging,
                                Some(scope.id),
                                traces_collection,
                                &mut x_vec,
                                "Drop X trace here",
                                true,
                                Some(color_refs),
                                None,
                            );
                            scope_changed |= changed;

                            // Apply edited colors back to pair looks.
                            if colors.len() > look_vec.len() {
                                look_vec.resize_with(colors.len(), TraceLook::default);
                            }
                            for (i, c) in colors.into_iter().enumerate() {
                                if let Some(look) = look_vec.get_mut(i) {
                                    look.color = c;
                                }
                            }

                            if let Some(dropped) = dropped {
                                let (idx, payload) = dropped;
                                let was_empty =
                                    x_before.get(idx).and_then(|v| v.as_ref()).is_none()
                                        && y_before.get(idx).and_then(|v| v.as_ref()).is_none();

                                if was_empty {
                                    while look_vec.len() <= idx {
                                        look_vec.push(TraceLook::default());
                                    }
                                    if let Some(tr) = traces_collection.get_trace(&payload.trace) {
                                        look_vec[idx] = tr.look.clone();
                                    }
                                }

                                if let Some(src) = payload.origin_scope_id {
                                    if src != scope.id {
                                        resp.moved_from_scope = Some((src, payload.trace));
                                    }
                                }
                            }
                        });

                        ui.separator();

                        ui.allocate_ui(egui::vec2(col_w, 0.0), |ui| {
                            let mut open_editor_idx = usize::MAX;
                            let (changed, dropped) = Self::render_trace_list(
                                ui,
                                ("xy_scope_y", scope.id),
                                "Y Traces",
                                global_dragging,
                                Some(scope.id),
                                traces_collection,
                                &mut y_vec,
                                "Drop Y trace here",
                                true,
                                None,
                                Some(&mut open_editor_idx),
                            );
                            scope_changed |= changed;

                            if open_editor_idx != usize::MAX {
                                *look_editor_out = None;
                                *xy_pair_look_editor_out = Some((scope.id, open_editor_idx));
                            }

                            if let Some(dropped) = dropped {
                                let (idx, payload) = dropped;
                                let was_empty =
                                    x_before.get(idx).and_then(|v| v.as_ref()).is_none()
                                        && y_before.get(idx).and_then(|v| v.as_ref()).is_none();

                                if was_empty {
                                    while look_vec.len() <= idx {
                                        look_vec.push(TraceLook::default());
                                    }
                                    if let Some(tr) = traces_collection.get_trace(&payload.trace) {
                                        look_vec[idx] = tr.look.clone();
                                    }
                                }

                                if let Some(src) = payload.origin_scope_id {
                                    if src != scope.id {
                                        resp.moved_from_scope = Some((src, payload.trace));
                                    }
                                }
                            }
                        });
                    });
                });

                if scope_changed {
                    let max_len = x_vec.len().max(y_vec.len()).max(look_vec.len());
                    let mut rebuilt: Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)> =
                        Vec::new();
                    rebuilt.reserve(max_len);

                    for i in 0..max_len {
                        let x = x_vec.get(i).cloned().unwrap_or(None);
                        let y = y_vec.get(i).cloned().unwrap_or(None);
                        let look = look_vec.get(i).cloned().unwrap_or_else(TraceLook::default);
                        if x.is_none() && y.is_none() {
                            continue;
                        }
                        rebuilt.push((x, y, look));
                    }

                    // Keep at most one incomplete pair (None/Some or Some/None).
                    let mut seen_incomplete = false;
                    rebuilt.retain(|(x, y, _)| {
                        if x.is_none() ^ y.is_none() {
                            if seen_incomplete {
                                return false;
                            }
                            seen_incomplete = true;
                        }
                        true
                    });

                    scope.xy_pairs = rebuilt;
                }

                // Write back pair looks (color/style edits) even if trace membership didn't change.
                if scope.xy_pairs.len() == look_vec.len() {
                    for (i, (_x, _y, look)) in scope.xy_pairs.iter_mut().enumerate() {
                        *look = look_vec[i].clone();
                    }
                }
            }
        }

        resp.scope_changed = scope_changed;
        resp
    }
}
