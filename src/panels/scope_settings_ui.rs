use crate::data::data::LivePlotRequests;
use crate::data::scope::{AxisType, LegendPosition, ScopeData, ScopeType, TimeFormat, ValueFormat};
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;
use crate::data::traces::TracesCollection;
use eframe::egui;
use egui::{Color32, Id, Ui};
use egui_dnd::dnd;
use egui_phosphor_icons::icons::{CARET_DOWN, CARET_UP, DOTS_SIX_VERTICAL, PALETTE, PENCIL, TRASH};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// One draggable unit: either a single trace (time scope / traces table) or a
/// complete XY pair (XY scope).  Pairs move as a unit so X/Y association and
/// the pair look are preserved across scopes.
#[derive(Clone, Debug)]
pub enum DragItem {
    Trace(TraceRef),
    XyPair {
        x: Option<TraceRef>,
        y: Option<TraceRef>,
        look: TraceLook,
    },
}

impl DragItem {
    /// Member traces of this item (x then y for pairs; empty for empty pairs).
    pub fn trace_refs(&self) -> Vec<TraceRef> {
        match self {
            DragItem::Trace(t) => vec![t.clone()],
            DragItem::XyPair { x, y, .. } => x.iter().chain(y.iter()).cloned().collect(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            DragItem::Trace(t) => t.0.clone(),
            DragItem::XyPair { x, y, .. } => match (x, y) {
                (Some(x), Some(y)) => format!("{} vs {}", y.0, x.0),
                (Some(x), None) => format!("(empty) vs {}", x.0),
                (None, Some(y)) => format!("{} vs (empty)", y.0),
                (None, None) => "(empty pair)".to_string(),
            },
        }
    }
}

/// Drag payload carrying one or more items (from a multi-selection).
#[derive(Clone, Debug)]
pub struct DragPayload {
    pub items: Vec<DragItem>,
    /// Scope the drag originated from, if dragged out of a scope list.
    /// `None` for drags from the main traces table (copy semantics).
    pub origin_scope_id: Option<usize>,
}

impl DragPayload {
    pub fn single(trace: TraceRef, origin_scope_id: Option<usize>) -> Self {
        Self {
            items: vec![DragItem::Trace(trace)],
            origin_scope_id,
        }
    }

    /// All member traces of the payload (pairs contribute x and y).
    pub fn trace_refs(&self) -> Vec<TraceRef> {
        self.items
            .iter()
            .flat_map(|item| item.trace_refs())
            .collect()
    }

    /// Short label for the drag preview (`name` or `name +N`).
    pub fn preview_label(&self) -> String {
        let first = self
            .items
            .first()
            .map(|i| i.label())
            .unwrap_or_else(|| "?".to_string());
        if self.items.len() > 1 {
            format!("{} +{} more", first, self.items.len() - 1)
        } else {
            first
        }
    }
}

/// Identifies which list a selection/anchor belongs to — Ctrl/Shift range
/// selection only applies within one list.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TraceListKey {
    /// The main traces table.
    Table,
    /// A time scope's flat trace list (scope id).
    ScopeTraces(usize),
    /// An XY scope's pair list (scope id); both X and Y cells share it.
    ScopePairs(usize),
}

/// Multi-selection state shared between the traces table and all scope
/// assignment lists (owned by `TracesPanel`).
#[derive(Default)]
pub struct TraceSelection {
    /// Selected trace names.  An XY pair counts as selected when either
    /// member is in this set.
    pub traces: HashSet<TraceRef>,
    /// Shift-click anchor: the last non-shift-clicked item and its list.
    pub anchor: Option<(TraceListKey, TraceRef)>,
}

impl TraceSelection {
    pub fn clear(&mut self) {
        self.traces.clear();
        self.anchor = None;
    }

    pub fn contains(&self, t: &TraceRef) -> bool {
        self.traces.contains(t)
    }

    /// Plain click: select only this item and set the anchor.
    pub fn select_only(&mut self, key: TraceListKey, t: TraceRef) {
        self.traces.clear();
        self.traces.insert(t.clone());
        self.anchor = Some((key, t));
    }

    /// Ctrl-click: toggle membership and set the anchor.
    pub fn toggle(&mut self, key: TraceListKey, t: TraceRef) {
        if !self.traces.remove(&t) {
            self.traces.insert(t.clone());
        }
        self.anchor = Some((key, t));
    }

    /// Shift-click: select the contiguous range between the anchor and the
    /// clicked item in `ordered` (the list's current order).  Falls back to a
    /// plain select when the anchor is missing or belongs to another list.
    /// `extend` (Ctrl+Shift) adds the range to the selection instead of
    /// replacing it.
    pub fn select_range(
        &mut self,
        key: TraceListKey,
        clicked: &TraceRef,
        ordered: &[TraceRef],
        extend: bool,
    ) {
        let anchor_t = match &self.anchor {
            Some((k, t)) if *k == key => t.clone(),
            _ => {
                self.select_only(key, clicked.clone());
                return;
            }
        };
        let a = ordered.iter().position(|t| t == &anchor_t);
        let b = ordered.iter().position(|t| t == clicked);
        let (Some(a), Some(b)) = (a, b) else {
            self.select_only(key, clicked.clone());
            return;
        };
        let (lo, hi) = (a.min(b), a.max(b));
        if !extend {
            self.traces.clear();
        }
        for t in &ordered[lo..=hi] {
            self.traces.insert(t.clone());
        }
    }

    /// Drop selected names that are no longer present in `valid`.
    pub fn retain(&mut self, valid: &HashSet<TraceRef>) {
        self.traces.retain(|t| valid.contains(t));
    }
}

pub struct ScopeSettingsResponse {
    pub type_changed: bool,
    pub scope_changed: bool,
    /// Traces that were dragged in from another scope and must be removed
    /// there: (source scope id, moved trace refs).
    pub moved_from_scope: Option<(usize, Vec<TraceRef>)>,
    pub recolor_requested: bool,
    pub sort_requested: bool,
    pub sort_ascending: bool,
}

struct TraceListArgs<'a, S: Hash + std::fmt::Debug> {
    id_salt: S,
    title: &'a str,
    global_dragging: &'a mut Option<DragPayload>,
    origin_scope_id: Option<usize>,
    traces_collection: &'a mut TracesCollection,
    traces: &'a mut Vec<Option<TraceRef>>,
    empty_label: &'a str,
    preserve_slots: bool,
    color_chooser: Option<Vec<&'a mut Color32>>,
    open_look_editor: Option<&'a mut usize>,
    selection: &'a mut TraceSelection,
    list_key: TraceListKey,
}

/// Which cell of an XY pair row a drop landed on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PairCell {
    X,
    Y,
}

/// Bundled arguments for [`ScopeSettingsUiPanel::render_scope_assignment`].
pub struct ScopeAssignmentArgs<'a> {
    pub scope: &'a mut ScopeData,
    pub can_remove_scope: bool,
    pub pending: &'a mut LivePlotRequests,
    pub global_dragging: &'a mut Option<DragPayload>,
    pub traces_collection: &'a mut TracesCollection,
    pub look_editor_out: &'a mut Option<TraceRef>,
    pub xy_pair_look_editor_out: &'a mut Option<(usize, usize)>,
    pub selection: &'a mut TraceSelection,
}

#[derive(Default)]
pub struct ScopeSettingsUiPanel {
    renaming_scope_id: Option<usize>,
    rename_buffer: String,
    rename_focus_scope: Option<usize>,
    sort_ascending: bool,
}

fn trace_tooltip(traces_collection: &TracesCollection, trace: &TraceRef) -> String {
    let info = traces_collection
        .get_trace(trace)
        .map(|tr| tr.info.as_str())
        .unwrap_or("");
    if info.trim().is_empty() {
        trace.0.clone()
    } else {
        format!("{}\n{}", trace.0, info)
    }
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
        let mut resp = ScopeSettingsResponse {
            type_changed: false,
            scope_changed: false,
            moved_from_scope: None,
            recolor_requested: false,
            sort_requested: false,
            sort_ascending: self.sort_ascending,
        };

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
                    .add_enabled(can_remove_scope, egui::Button::new(TRASH.as_str()))
                    .on_hover_text("Remove this scope from layout")
                    .clicked()
                {
                    pending.remove_scope = Some(scope_id);
                }

                if ui
                    .small_button(PENCIL.as_str())
                    .on_hover_text("Rename")
                    .clicked()
                {
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

                let positions = [
                    (LegendPosition::LeftTop, "Left Top"),
                    (LegendPosition::RightTop, "Right Top"),
                    (LegendPosition::LeftBottom, "Left Bottom"),
                    (LegendPosition::RightBottom, "Right Bottom"),
                ];
                let selected_label = positions
                    .iter()
                    .find(|(pos, _)| *pos == scope.legend_position)
                    .map(|(_, label)| *label)
                    .unwrap_or("Left Top");
                ui.horizontal(|ui| {
                    ui.label("Position");
                    let _ = egui::ComboBox::from_id_salt(("legend_position", scope_id))
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            for (pos, label) in positions {
                                ui.selectable_value(&mut scope.legend_position, pos, label);
                            }
                        });
                });
            });
        });

        ui.horizontal(|ui| {
            if ui
                .small_button(PALETTE.as_str())
                .on_hover_text("Recolor traces to match legend order")
                .clicked()
            {
                resp.recolor_requested = true;
            }

            let sort_icon = if self.sort_ascending {
                CARET_UP
            } else {
                CARET_DOWN
            };
            if ui
                .small_button(sort_icon)
                .on_hover_text(if self.sort_ascending {
                    "Sort traces alphabetically (ascending)"
                } else {
                    "Sort traces alphabetically (descending)"
                })
                .clicked()
            {
                resp.sort_requested = true;
                resp.sort_ascending = self.sort_ascending;
                self.sort_ascending = !self.sort_ascending;
            }

            let time_sel = scope.scope_type == ScopeType::TimeScope;
            if ui
                .selectable_label(time_sel, "Time-Scope")
                .on_hover_text("Time window / scrolling time axis")
                .clicked()
            {
                scope.scope_type = ScopeType::TimeScope;
                scope.x_axis.axis_type = AxisType::Time(TimeFormat::default());
                scope.x_axis.name = None;
                scope.y_axis.name = None;
                scope.x_axis.show_label = false;
                scope.y_axis.show_label = false;
            }

            let xy_sel = scope.scope_type == ScopeType::XYScope;
            if ui
                .selectable_label(xy_sel, "XY-Scope")
                .on_hover_text("X/Y plot (trace pairing)")
                .clicked()
            {
                scope.scope_type = ScopeType::XYScope;
                scope.x_axis.axis_type = AxisType::Value(ValueFormat::default());
                scope.x_axis.name = None;
                scope.y_axis.name = None;
                scope.x_axis.show_label = true;
                scope.y_axis.show_label = true;
            }
        });

        resp.type_changed = prev_type != scope.scope_type;
        resp
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

    fn render_drop_slot(
        ui: &mut Ui,
        id_salt: impl Hash + std::fmt::Debug,
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

    /// After egui_dnd moved the dragged row to its drop index, pull the other
    /// selected rows along so the whole group stays contiguous, preserving the
    /// pre-drag relative order around the dragged item.
    fn group_move(
        list: &mut Vec<Option<TraceRef>>,
        start: &[Option<TraceRef>],
        dragged_idx: usize,
        moved: &HashSet<TraceRef>,
    ) {
        let Some(dragged) = list.get(dragged_idx).and_then(|t| t.clone()) else {
            return;
        };
        let orig_pos: HashMap<TraceRef, usize> = start
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.as_ref().map(|t| (t.clone(), i)))
            .collect();
        let Some(drag_orig) = orig_pos.get(&dragged).copied() else {
            return;
        };
        let mut before_part = Vec::new();
        let mut after_part = Vec::new();
        for t in start.iter().flatten() {
            if t == &dragged || !moved.contains(t) {
                continue;
            }
            if orig_pos[t] < drag_orig {
                before_part.push(t.clone());
            } else {
                after_part.push(t.clone());
            }
        }
        if before_part.is_empty() && after_part.is_empty() {
            return;
        }
        let others: HashSet<&TraceRef> = before_part.iter().chain(after_part.iter()).collect();
        list.retain(|t| t.as_ref().is_none_or(|t| !others.contains(t)));
        let Some(idx) = list.iter().position(|t| t.as_ref() == Some(&dragged)) else {
            return;
        };
        for (i, t) in after_part.into_iter().enumerate() {
            list.insert(idx + 1 + i, Some(t));
        }
        for (i, t) in before_part.into_iter().enumerate() {
            list.insert(idx + i, Some(t));
        }
    }

    /// Same grouped-move for XY pair rows, matching items by member identity.
    fn group_move_pairs(
        list: &mut Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>,
        start: &[(Option<TraceRef>, Option<TraceRef>, TraceLook)],
        dragged_idx: usize,
        moved: &HashSet<TraceRef>,
    ) {
        let is_moved = |p: &(Option<TraceRef>, Option<TraceRef>, TraceLook)| {
            p.0.as_ref().is_some_and(|t| moved.contains(t))
                || p.1.as_ref().is_some_and(|t| moved.contains(t))
        };
        let pair_eq = |a: &(Option<TraceRef>, Option<TraceRef>, TraceLook),
                       b: &(Option<TraceRef>, Option<TraceRef>, TraceLook)| {
            a.0 == b.0 && a.1 == b.1
        };
        let Some(dragged) = list.get(dragged_idx).cloned() else {
            return;
        };
        let Some(drag_orig) = start.iter().position(|p| pair_eq(p, &dragged)) else {
            return;
        };
        let mut before_part = Vec::new();
        let mut after_part = Vec::new();
        for (i, p) in start.iter().enumerate() {
            if pair_eq(p, &dragged) || !is_moved(p) {
                continue;
            }
            if i < drag_orig {
                before_part.push(p.clone());
            } else {
                after_part.push(p.clone());
            }
        }
        if before_part.is_empty() && after_part.is_empty() {
            return;
        }
        list.retain(|p| !(is_moved(p) && !pair_eq(p, &dragged)));
        let Some(idx) = list.iter().position(|p| pair_eq(p, &dragged)) else {
            return;
        };
        for (i, p) in after_part.into_iter().enumerate() {
            list.insert(idx + 1 + i, p);
        }
        for (i, p) in before_part.into_iter().enumerate() {
            list.insert(idx + i, p);
        }
    }

    /// Fill slot `idx` of a slotted list with the payload's first member trace,
    /// then spill remaining members into following empty slots or the list end.
    /// Moved members are removed from any other slot first (move semantics).
    fn apply_slot_drop(traces: &mut Vec<Option<TraceRef>>, idx: usize, payload: &DragPayload) {
        let flat = payload.trace_refs();
        if flat.is_empty() {
            return;
        }
        for (j, other) in traces.iter_mut().enumerate() {
            if j != idx && flat.iter().any(|t| other.as_ref() == Some(t)) {
                *other = None;
            }
        }
        let mut it = flat.into_iter();
        if let Some(first) = it.next() {
            if idx < traces.len() {
                traces[idx] = Some(first);
            } else {
                traces.push(Some(first));
            }
        }
        for t in it {
            if let Some(slot) = traces.iter_mut().skip(idx + 1).find(|s| s.is_none()) {
                *slot = Some(t);
            } else {
                traces.push(Some(t));
            }
        }
    }

    fn render_trace_list<S: Hash + std::fmt::Debug>(
        ui: &mut Ui,
        args: TraceListArgs<'_, S>,
    ) -> (bool, Option<(usize, DragPayload)>) {
        let TraceListArgs {
            id_salt,
            title,
            global_dragging,
            origin_scope_id,
            traces_collection,
            traces,
            empty_label,
            preserve_slots,
            color_chooser,
            open_look_editor,
            selection,
            list_key,
        } = args;
        let before = traces.clone();
        let mut removed: HashSet<TraceRef> = HashSet::new();
        let mut pending_place: Option<(usize, DragPayload)> = None;
        let mut dropped_payload: Option<(usize, DragPayload)> = None;
        let mut sel_click: Option<(TraceRef, usize, egui::Modifiers)> = None;
        // Payload this list itself is dragging (subset of `dragging_payload`
        // which may also come from the traces table or another list).
        let mut own_drag: Option<DragPayload> = None;

        // We support dragging from two sources:
        // - the main traces table (sets `global_dragging`)
        // - a scope list itself (we populate `global_dragging` from ItemState::dragged)
        let mut drag_active = global_dragging.is_some();
        let mut dragging_payload: Option<DragPayload> = global_dragging.clone();

        let mut color_chooser = color_chooser;
        let mut open_look_editor = open_look_editor;
        let start_order_key = Id::new(("dnd_start_order", &id_salt));

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
                                    // Dragging a selected row drags the whole
                                    // selection (in list order); an unselected
                                    // row drags alone.
                                    let mut items: Vec<DragItem> = if selection.traces.contains(t) {
                                        before
                                            .iter()
                                            .flatten()
                                            .filter(|b| selection.traces.contains(*b))
                                            .cloned()
                                            .map(DragItem::Trace)
                                            .collect()
                                    } else {
                                        Vec::new()
                                    };
                                    if items.is_empty() {
                                        items.push(DragItem::Trace(t.clone()));
                                    }
                                    let p = DragPayload {
                                        items,
                                        origin_scope_id,
                                    };
                                    ui.ctx().data_mut(|d| {
                                        if d.get_temp::<Vec<Option<TraceRef>>>(start_order_key)
                                            .is_none()
                                        {
                                            d.insert_temp(start_order_key, before.clone());
                                        }
                                    });
                                    own_drag = Some(p.clone());
                                    dragging_payload = Some(p.clone());
                                    *global_dragging = Some(p);
                                    drag_active = true;
                                }
                            }

                            match tr {
                                Some(t) => {
                                    let selected = selection.traces.contains(t);
                                    let frame = egui::Frame::new()
                                        .inner_margin(egui::Margin::symmetric(2, 0))
                                        .corner_radius(egui::CornerRadius::same(3));
                                    let frame = if selected {
                                        frame.fill(
                                            ui.visuals().selection.bg_fill.linear_multiply(0.35),
                                        )
                                    } else {
                                        frame
                                    };
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            handle.ui(ui, |ui| {
                                                ui.label(DOTS_SIX_VERTICAL);
                                            });

                                            if let Some(colors) = color_chooser.as_mut() {
                                                if let Some(slot_color) = colors.get_mut(idx) {
                                                    ui.color_edit_button_srgba(slot_color)
                                                        .on_hover_text("Change color");
                                                }
                                            }

                                            let resp = ui
                                                .add(
                                                    egui::Label::new(t.0.clone())
                                                        .truncate()
                                                        .show_tooltip_when_elided(true)
                                                        .sense(egui::Sense::click()),
                                                )
                                                .on_hover_text(trace_tooltip(traces_collection, t));
                                            if resp.hovered() {
                                                traces_collection.hover_trace =
                                                    Some(vec![t.clone()]);
                                            }
                                            if resp.clicked() {
                                                sel_click = Some((
                                                    t.clone(),
                                                    idx,
                                                    ui.input(|i| i.modifiers),
                                                ));
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .small_button(
                                                            egui_phosphor_icons::icons::TRASH,
                                                        )
                                                        .on_hover_text("Remove")
                                                        .clicked()
                                                    {
                                                        removed.insert(t.clone());
                                                    }
                                                    if open_look_editor.is_some()
                                                        && ui
                                                            .small_button(PALETTE.as_str())
                                                            .on_hover_text("Edit trace style")
                                                            .clicked()
                                                    {
                                                        if let Some(open_out) =
                                                            open_look_editor.as_mut()
                                                        {
                                                            **open_out = idx;
                                                        }
                                                    }
                                                },
                                            );
                                        });
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
            } else {
                ui.ctx()
                    .data_mut(|d| d.remove::<Vec<Option<TraceRef>>>(start_order_key));
            }

            // Grouped in-list reorder: pull the other selected rows along once
            // the drag finishes (slot drops handle placement themselves).
            if pending_place.is_none() {
                if let Some(update) = dnd_resp.final_update() {
                    if let Some(p) = &own_drag {
                        if p.items.len() > 1 {
                            let start = ui
                                .ctx()
                                .data_mut(|d| d.get_temp::<Vec<Option<TraceRef>>>(start_order_key))
                                .unwrap_or_else(|| before.clone());
                            let moved: HashSet<TraceRef> = p.trace_refs().into_iter().collect();
                            Self::group_move(traces, &start, update.to, &moved);
                        }
                    }
                }
            }

            // Ctrl/Shift-aware selection clicks collected in the rows are
            // resolved here where the full list order is available.
            if let Some((t, idx, mods)) = sel_click {
                if mods.shift {
                    let ordered: Vec<TraceRef> = traces.iter().flatten().cloned().collect();
                    selection.select_range(list_key, &t, &ordered, mods.command);
                } else if mods.command {
                    selection.toggle(list_key, t);
                } else {
                    selection.select_only(list_key, t);
                    // Preserve the existing plain-click behavior of opening the
                    // trace look editor.
                    if let Some(open_out) = open_look_editor.as_mut() {
                        **open_out = idx;
                    }
                }
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

                let flat = t.trace_refs();
                for x in traces.iter_mut() {
                    if flat.iter().any(|f| x.as_ref() == Some(f)) {
                        *x = None;
                    }
                }
                for tref in flat {
                    traces.push(Some(tref));
                }
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
            Self::apply_slot_drop(traces, idx, &dropped);
        }

        if drag_active {
            if let Some(p) = dragging_payload {
                *global_dragging = Some(p);
            }
        }

        (*traces != before, dropped_payload)
    }

    /// Look for a newly-created pair slot from the dropped trace's own look.
    fn look_of(traces_collection: &TracesCollection, t: &TraceRef) -> TraceLook {
        traces_collection
            .get_trace(t)
            .map(|tr| tr.look.clone())
            .unwrap_or_default()
    }

    /// Apply a payload dropped onto a pair-list cell or the list end.
    /// `idx` is the pair row index (or `pairs.len()` for the end slot) and
    /// `cell` is the target cell for `DragItem::Trace` items (`XyPair` items
    /// always fill a whole row).
    fn apply_pair_drop(
        pairs: &mut Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>,
        idx: usize,
        cell: PairCell,
        payload: &DragPayload,
        traces_collection: &TracesCollection,
    ) {
        let moved: Vec<TraceRef> = payload.trace_refs();
        if moved.is_empty() {
            return;
        }

        // Move semantics: clear the moved members from all other rows.
        for (j, (x, y, _)) in pairs.iter_mut().enumerate() {
            if j == idx {
                continue;
            }
            if x.as_ref().is_some_and(|t| moved.contains(t)) {
                *x = None;
            }
            if y.as_ref().is_some_and(|t| moved.contains(t)) {
                *y = None;
            }
        }

        let place = |pairs: &mut Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>,
                     row: usize,
                     cell: PairCell,
                     item: &DragItem| match item {
            DragItem::XyPair { x, y, look } => {
                if row < pairs.len() {
                    pairs[row] = (x.clone(), y.clone(), look.clone());
                } else {
                    pairs.push((x.clone(), y.clone(), look.clone()));
                }
            }
            DragItem::Trace(t) => {
                if row < pairs.len() {
                    let p = &mut pairs[row];
                    let was_empty = p.0.is_none() && p.1.is_none();
                    match cell {
                        PairCell::X => p.0 = Some(t.clone()),
                        PairCell::Y => p.1 = Some(t.clone()),
                    }
                    if was_empty {
                        p.2 = Self::look_of(traces_collection, t);
                    }
                } else {
                    // New rows: a lone trace lands on the dragged side so the
                    // pair can be completed by a later drop on the other cell.
                    let look = Self::look_of(traces_collection, t);
                    match cell {
                        PairCell::X => pairs.push((Some(t.clone()), None, look)),
                        PairCell::Y => pairs.push((None, Some(t.clone()), look)),
                    }
                }
            }
        };

        let mut row = idx;
        for item in payload.items.iter() {
            place(pairs, row, cell, item);
            row += 1;
        }
    }

    /// Render one merged row-per-pair list for XY scopes.  Reordering moves
    /// whole pairs (X, Y and the pair look travel together); Ctrl/Shift
    /// selection and multi-drag operate on pairs.
    #[allow(clippy::too_many_arguments)]
    fn render_pair_list<S: Hash + std::fmt::Debug>(
        ui: &mut Ui,
        id_salt: S,
        global_dragging: &mut Option<DragPayload>,
        origin_scope_id: usize,
        traces_collection: &mut TracesCollection,
        pairs: &mut Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>,
        selection: &mut TraceSelection,
        list_key: TraceListKey,
        open_pair_editor: &mut usize,
    ) -> (bool, Option<(usize, DragPayload)>) {
        let before: Vec<(Option<TraceRef>, Option<TraceRef>)> = pairs
            .iter()
            .map(|(x, y, _)| (x.clone(), y.clone()))
            .collect();
        let mut pending_drop: Option<(usize, PairCell, DragPayload)> = None;
        let mut dropped_payload: Option<(usize, DragPayload)> = None;
        let mut sel_click: Option<(usize, TraceRef, egui::Modifiers)> = None;
        let mut own_drag: Option<DragPayload> = None;

        let mut drag_active = global_dragging.is_some();
        let mut dragging_payload: Option<DragPayload> = global_dragging.clone();
        let start_order_key = Id::new(("dnd_start_order_pairs", &id_salt));

        let pairs_snapshot = pairs.clone();

        ui.vertical(|ui| {
            // Header
            let chrome_w = 20.0 + 24.0 + 18.0 + 8.0; // handle + color + palette + spacing
            let cell_w = ((ui.available_width() - chrome_w) / 2.0).max(60.0);
            ui.horizontal(|ui| {
                ui.add_space(chrome_w);
                ui.allocate_ui(egui::vec2(cell_w, 14.0), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.strong("X");
                    });
                });
                ui.allocate_ui(egui::vec2(cell_w, 14.0), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.strong("Y");
                    });
                });
            });
            ui.separator();

            let dnd_id = Id::new(("pair_list_dnd", &id_salt));
            let dnd_resp = dnd(ui, dnd_id).show_custom(|ui, iter| {
                for (idx, (x, y, look)) in pairs.iter_mut().enumerate() {
                    let is_draggable = x.is_some() || y.is_some();
                    let row_id = Id::new((
                        "pair_row",
                        &id_salt,
                        x.as_ref().map(|t| t.0.clone()),
                        y.as_ref().map(|t| t.0.clone()),
                        idx,
                    ));

                    iter.next(ui, row_id, idx, is_draggable, |ui, item_handle| {
                        item_handle.ui(ui, |ui, handle, state| {
                            if state.dragged && is_draggable {
                                let this_selected =
                                    x.as_ref().is_some_and(|t| selection.traces.contains(t))
                                        || y.as_ref().is_some_and(|t| selection.traces.contains(t));
                                let mut items: Vec<DragItem> = if this_selected {
                                    pairs_snapshot
                                        .iter()
                                        .filter(|(px, py, _)| {
                                            px.as_ref()
                                                .is_some_and(|t| selection.traces.contains(t))
                                                || py
                                                    .as_ref()
                                                    .is_some_and(|t| selection.traces.contains(t))
                                        })
                                        .map(|(px, py, l)| DragItem::XyPair {
                                            x: px.clone(),
                                            y: py.clone(),
                                            look: l.clone(),
                                        })
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                                if items.is_empty() {
                                    items.push(DragItem::XyPair {
                                        x: x.clone(),
                                        y: y.clone(),
                                        look: look.clone(),
                                    });
                                }
                                let p = DragPayload {
                                    items,
                                    origin_scope_id: Some(origin_scope_id),
                                };
                                ui.ctx().data_mut(|d| {
                                    if d.get_temp::<Vec<(
                                        Option<TraceRef>,
                                        Option<TraceRef>,
                                        TraceLook,
                                    )>>(start_order_key)
                                    .is_none()
                                    {
                                        d.insert_temp(start_order_key, pairs_snapshot.clone());
                                    }
                                });
                                own_drag = Some(p.clone());
                                dragging_payload = Some(p.clone());
                                *global_dragging = Some(p);
                                drag_active = true;
                            }

                            let pair_selected =
                                x.as_ref().is_some_and(|t| selection.traces.contains(t))
                                    || y.as_ref().is_some_and(|t| selection.traces.contains(t));
                            let frame = egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(2, 0))
                                .corner_radius(egui::CornerRadius::same(3));
                            let frame = if pair_selected {
                                frame.fill(ui.visuals().selection.bg_fill.linear_multiply(0.35))
                            } else {
                                frame
                            };
                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    handle.ui(ui, |ui| {
                                        ui.label(DOTS_SIX_VERTICAL);
                                    });

                                    ui.color_edit_button_srgba(&mut look.color)
                                        .on_hover_text("Change pair color");
                                    if ui
                                        .small_button(PALETTE.as_str())
                                        .on_hover_text("Edit pair style")
                                        .clicked()
                                    {
                                        *open_pair_editor = idx;
                                    }

                                    // X cell
                                    ui.allocate_ui(egui::vec2(cell_w, 18.0), |ui| {
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| match x {
                                                Some(t) => {
                                                    let resp = ui
                                                        .add(
                                                            egui::Label::new(t.0.clone())
                                                                .truncate()
                                                                .show_tooltip_when_elided(true)
                                                                .sense(egui::Sense::click()),
                                                        )
                                                        .on_hover_text(trace_tooltip(
                                                            traces_collection,
                                                            t,
                                                        ));
                                                    if resp.hovered() {
                                                        traces_collection.hover_trace =
                                                            Some(vec![t.clone()]);
                                                    }
                                                    if resp.clicked() {
                                                        sel_click = Some((
                                                            idx,
                                                            t.clone(),
                                                            ui.input(|i| i.modifiers),
                                                        ));
                                                    }
                                                    if ui
                                                        .small_button(TRASH)
                                                        .on_hover_text("Remove X")
                                                        .clicked()
                                                    {
                                                        *x = None;
                                                    }
                                                }
                                                None => {
                                                    if let Some(dropped) = Self::render_drop_slot(
                                                        ui,
                                                        ("pair_slot_x", &id_salt, idx),
                                                        dragging_payload.as_ref(),
                                                        drag_active,
                                                        "X",
                                                    ) {
                                                        pending_drop = Some((
                                                            idx,
                                                            PairCell::X,
                                                            dropped.clone(),
                                                        ));
                                                        dropped_payload =
                                                            Some((idx, dropped.clone()));
                                                        *global_dragging = None;
                                                    }
                                                }
                                            },
                                        );
                                    });

                                    // Y cell
                                    ui.allocate_ui(egui::vec2(cell_w, 18.0), |ui| {
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| match y {
                                                Some(t) => {
                                                    let resp = ui
                                                        .add(
                                                            egui::Label::new(t.0.clone())
                                                                .truncate()
                                                                .show_tooltip_when_elided(true)
                                                                .sense(egui::Sense::click()),
                                                        )
                                                        .on_hover_text(trace_tooltip(
                                                            traces_collection,
                                                            t,
                                                        ));
                                                    if resp.hovered() {
                                                        traces_collection.hover_trace =
                                                            Some(vec![t.clone()]);
                                                    }
                                                    if resp.clicked() {
                                                        sel_click = Some((
                                                            idx,
                                                            t.clone(),
                                                            ui.input(|i| i.modifiers),
                                                        ));
                                                    }
                                                    if ui
                                                        .small_button(TRASH)
                                                        .on_hover_text("Remove Y")
                                                        .clicked()
                                                    {
                                                        *y = None;
                                                    }
                                                }
                                                None => {
                                                    if let Some(dropped) = Self::render_drop_slot(
                                                        ui,
                                                        ("pair_slot_y", &id_salt, idx),
                                                        dragging_payload.as_ref(),
                                                        drag_active,
                                                        "Y",
                                                    ) {
                                                        pending_drop = Some((
                                                            idx,
                                                            PairCell::Y,
                                                            dropped.clone(),
                                                        ));
                                                        dropped_payload =
                                                            Some((idx, dropped.clone()));
                                                        *global_dragging = None;
                                                    }
                                                }
                                            },
                                        );
                                    });
                                });
                            });
                        })
                    });
                }
            });
            // Keep sibling columns in sync while reordering and on drop.
            dnd_resp.update_vec(pairs);

            if dnd_resp.is_dragging() {
                ui.ctx().request_repaint();
                drag_active = true;
            } else {
                ui.ctx().data_mut(|d| {
                    d.remove::<Vec<(Option<TraceRef>, Option<TraceRef>, TraceLook)>>(
                        start_order_key,
                    )
                });
            }

            if pending_drop.is_none() {
                if let Some(update) = dnd_resp.final_update() {
                    if let Some(p) = &own_drag {
                        if p.items.len() > 1 {
                            let start = ui
                                .ctx()
                                .data_mut(|d| {
                                    d.get_temp::<Vec<(
                                        Option<TraceRef>,
                                        Option<TraceRef>,
                                        TraceLook,
                                    )>>(start_order_key)
                                })
                                .unwrap_or_else(|| pairs_snapshot.clone());
                            let moved: HashSet<TraceRef> = p.trace_refs().into_iter().collect();
                            Self::group_move_pairs(pairs, &start, update.to, &moved);
                        }
                    }
                }
            }

            // Pair-level selection: clicking a member selects the whole pair.
            // Re-find the pair by member — it may have shifted during the drag.
            if let Some((_idx, member, mods)) = sel_click {
                let idx = pairs
                    .iter()
                    .position(|(px, py, _)| {
                        px.as_ref() == Some(&member) || py.as_ref() == Some(&member)
                    })
                    .unwrap_or(0);
                let pair_key = pairs
                    .get(idx)
                    .and_then(|(px, py, _)| px.clone().or_else(|| py.clone()));
                if let Some(key_ref) = pair_key {
                    if mods.shift {
                        // Range over pair rows using each row's first member.
                        let ordered: Vec<TraceRef> = pairs
                            .iter()
                            .filter_map(|(px, py, _)| px.clone().or_else(|| py.clone()))
                            .collect();
                        // Determine the index range first, then select all
                        // members of the covered pairs.
                        let anchor_t = selection
                            .anchor
                            .as_ref()
                            .and_then(|(k, t)| (*k == list_key).then(|| t.clone()));
                        let a = anchor_t
                            .and_then(|t| ordered.iter().position(|o| o == &t))
                            .unwrap_or(idx);
                        let (lo, hi) = (a.min(idx), a.max(idx));
                        if !mods.command {
                            selection.traces.clear();
                        }
                        for (px, py, _) in pairs.iter().take(hi + 1).skip(lo) {
                            if let Some(t) = px {
                                selection.traces.insert(t.clone());
                            }
                            if let Some(t) = py {
                                selection.traces.insert(t.clone());
                            }
                        }
                        selection.anchor = Some((list_key, key_ref));
                    } else if mods.command {
                        let (px, py, _) = &pairs[idx];
                        let members: Vec<TraceRef> =
                            [px.clone(), py.clone()].into_iter().flatten().collect();
                        let selected = members.iter().any(|t| selection.traces.contains(t));
                        if selected {
                            for t in &members {
                                selection.traces.remove(t);
                            }
                        } else {
                            for t in members {
                                selection.traces.insert(t);
                            }
                        }
                        selection.anchor = Some((list_key, key_ref));
                    } else {
                        selection.traces.clear();
                        if let (Some(px), Some(py)) = (&pairs[idx].0, &pairs[idx].1) {
                            selection.traces.insert(px.clone());
                            selection.traces.insert(py.clone());
                        } else {
                            selection.traces.insert(key_ref.clone());
                        }
                        selection.anchor = Some((list_key, key_ref));
                    }
                }
            }

            let dragging = dragging_payload.as_ref();
            if let Some(dropped) = Self::render_drop_slot(
                ui,
                ("pair_drop_end", &id_salt),
                dragging,
                drag_active,
                "Drop trace / pair here",
            ) {
                let idx = pairs.len();
                dropped_payload = Some((idx, dropped.clone()));
                *global_dragging = None;
                pending_drop = Some((idx, PairCell::X, dropped));
            }
        });

        // Drop empty rows and apply pending drops.
        pairs.retain(|(x, y, _)| x.is_some() || y.is_some());
        if let Some((idx, cell, payload)) = pending_drop {
            Self::apply_pair_drop(
                pairs,
                idx.min(pairs.len()),
                cell,
                &payload,
                traces_collection,
            );
            pairs.retain(|(x, y, _)| x.is_some() || y.is_some());
        }

        if drag_active {
            if let Some(p) = dragging_payload {
                *global_dragging = Some(p);
            }
        }

        let after: Vec<(Option<TraceRef>, Option<TraceRef>)> = pairs
            .iter()
            .map(|(x, y, _)| (x.clone(), y.clone()))
            .collect();
        (after != before, dropped_payload)
    }

    /// Render a complete scope assignment block: header/settings + either trace list (time) or
    /// X/Y pairing UI (XY).
    pub fn render_scope_assignment(
        &mut self,
        ui: &mut Ui,
        args: ScopeAssignmentArgs<'_>,
    ) -> ScopeSettingsResponse {
        let ScopeAssignmentArgs {
            scope,
            can_remove_scope,
            pending,
            global_dragging,
            traces_collection,
            look_editor_out,
            xy_pair_look_editor_out,
            selection,
        } = args;
        let mut resp = self.render_scope_settings(ui, scope, can_remove_scope, pending);
        let mut scope_changed = false;

        if resp.recolor_requested {
            traces_collection.recolor_by_order(&scope.trace_order);
        }

        if resp.sort_requested {
            if resp.sort_ascending {
                scope.trace_order.sort_by(|a, b| a.0.cmp(&b.0));
            } else {
                scope.trace_order.sort_by(|a, b| b.0.cmp(&a.0));
            }
            if scope.scope_type == ScopeType::XYScope {
                scope.rebuild_xy_pairs_from_trace_order(traces_collection);
            }
            scope_changed = true;
        }

        if resp.type_changed {
            match scope.scope_type {
                ScopeType::XYScope => {
                    scope.rebuild_xy_pairs_from_trace_order(traces_collection);
                }
                ScopeType::TimeScope => {
                    scope.rebuild_trace_order_from_xy_pairs();
                }
            }
            Self::reorder_trace_order_pairs_first(scope);
        }

        match scope.scope_type {
            ScopeType::TimeScope => {
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
                    TraceListArgs {
                        id_salt: ("time_scope", scope.id),
                        title: "Trace",
                        global_dragging,
                        origin_scope_id: Some(scope.id),
                        traces_collection,
                        traces: &mut tmp,
                        empty_label: "Drop trace here",
                        preserve_slots: false,
                        color_chooser: Some(color_refs),
                        open_look_editor: Some(&mut open_editor_idx),
                        selection,
                        list_key: TraceListKey::ScopeTraces(scope.id),
                    },
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
                            resp.moved_from_scope = Some((src, dropped.1.trace_refs()));
                        }
                    }
                }
            }
            ScopeType::XYScope => {
                // Merged pair list: one row per XY pair; reorder and multi-drag
                // move whole pairs (members + pair look travel together).
                let mut open_editor_idx = usize::MAX;
                let (changed, dropped) = Self::render_pair_list(
                    ui,
                    ("xy_scope", scope.id),
                    global_dragging,
                    scope.id,
                    traces_collection,
                    &mut scope.xy_pairs,
                    selection,
                    TraceListKey::ScopePairs(scope.id),
                    &mut open_editor_idx,
                );
                scope_changed |= changed;

                if open_editor_idx != usize::MAX {
                    *look_editor_out = None;
                    *xy_pair_look_editor_out = Some((scope.id, open_editor_idx));
                }

                if let Some(dropped) = dropped {
                    if let Some(src) = dropped.1.origin_scope_id {
                        if src != scope.id {
                            resp.moved_from_scope = Some((src, dropped.1.trace_refs()));
                        }
                    }
                }
            }
        }

        resp.scope_changed = scope_changed;
        resp
    }
}
