//! Custom scrollable legend overlay for scopes.
//!
//! egui_plot's built-in legend cannot scroll and its internal item list is not
//! public, so liveplot renders its own legend on top of the plot frame and
//! communicates with the plot purely through `PlotMemory`
//! (`hidden_items` / `hovered_legend_item`), which egui_plot applies during
//! the next `Plot::show`.

use egui::epaint::CircleShape;
use egui::{
    pos2, vec2, Align, Color32, Direction, Frame, Layout, PointerButton, Rect, Response, Sense,
    Shadow, Shape, TextStyle, Ui, UiBuilder,
};

use crate::data::scope::LegendPosition;
use crate::data::traces::TraceRef;

/// One row in the scope legend overlay.
#[derive(Clone, Debug)]
pub struct LegendEntryData {
    /// Canonical plot item id — must match the `.id()` of the drawn item(s) so
    /// `PlotMemory::hidden_items` and `hovered_legend_item` apply to them.
    pub id: egui_plot::ItemId,
    /// Label displayed in the legend (may differ from the canonical item id,
    /// e.g. display aliases or appended info text).
    pub label: String,
    /// Color of the swatch. `Color32::TRANSPARENT` renders with the default
    /// text color, mirroring egui_plot's color-conflict fallback.
    pub color: Color32,
}

// ---------------------------------------------------------------------------
// Canonical plot item ids.
//
// Item ids must not depend on display labels so that `hidden_items` survives
// legend-label changes (info text toggle, display aliases).
// ---------------------------------------------------------------------------

/// Item id for a single (time-scope) trace. Matches `PlotItemBase::new`'s
/// `ItemId::new(&name)` so explicitly setting it is a no-op for `Line`, but is
/// required for additional items like `Points` (whose default id would be the
/// legend label).
pub fn trace_item_id(name: &TraceRef) -> egui_plot::ItemId {
    egui_plot::ItemId::new(&name.0)
}

/// Item id shared by the line/points of an XY pair.
pub fn xy_pair_item_id(x: &TraceRef, y: &TraceRef) -> egui_plot::ItemId {
    egui_plot::ItemId::new(("liveplot_xy_pair", &x.0, &y.0))
}

/// Item id of the "(last)" newest-point marker of an XY pair.
pub fn xy_last_point_item_id(pair_id: egui_plot::ItemId) -> egui_plot::ItemId {
    egui_plot::ItemId::new((pair_id, "last"))
}

/// Item id shared by all items of a threshold definition.
pub fn threshold_item_id(name: &str) -> egui_plot::ItemId {
    egui_plot::ItemId::new(("liveplot_threshold", name))
}

/// Item id shared by all items of a trigger.
pub fn trigger_item_id(name: &str) -> egui_plot::ItemId {
    egui_plot::ItemId::new(("liveplot_trigger", name))
}

/// Item id shared by all items of a measurement.
pub fn measurement_item_id(name: &str) -> egui_plot::ItemId {
    egui_plot::ItemId::new(("liveplot_measurement", name))
}

/// Item id shared by all items of a marker.
pub fn marker_item_id(name: &str) -> egui_plot::ItemId {
    egui_plot::ItemId::new(("liveplot_marker", name))
}

/// Outcome of a legend render pass; the caller applies it to `PlotMemory`.
#[derive(Debug, Default)]
pub struct ScopeLegendResult {
    /// Ids of unchecked entries → `PlotMemory::hidden_items`.
    pub hidden_items: Vec<egui_plot::ItemId>,
    /// Entry under the pointer → `PlotMemory::hovered_legend_item`.
    pub hovered_item: Option<egui_plot::ItemId>,
    /// Rect actually covered by the legend — lets the plot ignore pointer/scroll
    /// input that lands on the legend instead of the axes.
    pub rect: Option<Rect>,
}

/// Render the scope legend as an overlay on `frame` (the plot's transform
/// frame). `is_hidden` reports whether an item id is currently in
/// `PlotMemory::hidden_items`. `salt` (e.g. the scope id) keeps per-scope
/// scroll state distinct.
///
/// Returns `None` when there is nothing to show.
pub fn render_scope_legend(
    ui: &mut Ui,
    frame: Rect,
    position: LegendPosition,
    entries: &[LegendEntryData],
    salt: usize,
    is_hidden: impl Fn(&egui_plot::ItemId) -> bool,
) -> Option<ScopeLegendResult> {
    if entries.is_empty() {
        return None;
    }

    let main_dir = match position {
        LegendPosition::LeftTop | LegendPosition::RightTop => Direction::TopDown,
        LegendPosition::LeftBottom | LegendPosition::RightBottom => Direction::BottomUp,
    };
    let cross_align = match position {
        LegendPosition::LeftTop | LegendPosition::LeftBottom => Align::LEFT,
        LegendPosition::RightTop | LegendPosition::RightBottom => Align::RIGHT,
    };
    let layout = Layout::from_main_dir_and_cross_align(main_dir, cross_align);

    let legend_rect = frame.shrink(4.0);
    let mut legend_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("scope_legend", salt))
            .max_rect(legend_rect)
            .layout(layout),
    );

    let mut checked: Vec<bool> = entries.iter().map(|e| !is_hidden(&e.id)).collect();
    let mut result = ScopeLegendResult::default();

    // Same look as egui_plot's LegendWidget (alpha background, window stroke).
    let background = Frame {
        inner_margin: vec2(8.0, 4.0).into(),
        corner_radius: ui.style().visuals.window_corner_radius,
        shadow: Shadow::NONE,
        fill: ui.style().visuals.extreme_bg_color,
        stroke: ui.style().visuals.window_stroke(),
        ..Default::default()
    }
    .multiply_with_opacity(0.75);

    let frame_resp = background.show(&mut legend_ui, |ui| {
        // The point of this custom legend: scroll when the entries exceed the
        // plot height instead of overflowing the scope.
        let max_height = (legend_rect.height() - 16.0).max(24.0);
        let max_width = (legend_rect.width() * 0.75).max(40.0);
        egui::ScrollArea::vertical()
            .id_salt("liveplot_scope_legend")
            .auto_shrink([true, true])
            .max_height(max_height)
            .max_width(max_width)
            .show(ui, |ui| {
                ui.set_max_width(max_width);
                let font_id = TextStyle::Body.resolve(ui.style());
                let mut focus: Option<usize> = None;
                for (i, entry) in entries.iter().enumerate() {
                    let response =
                        legend_entry_ui(ui, &entry.label, entry.color, checked[i], &font_id);
                    checked[i] ^= response.clicked_by(PointerButton::Primary);
                    if response.hovered() {
                        result.hovered_item = Some(entry.id);
                    }
                    // Alt-click isolates one entry (same as egui_plot).
                    if response.clicked() && ui.input(|r| r.modifiers.alt) {
                        focus = Some(i);
                    }
                }
                if let Some(fi) = focus {
                    // If only the focus entry is visible, re-show everything.
                    let only_visible = entries
                        .iter()
                        .enumerate()
                        .all(|(i, _)| !checked[i] || i == fi);
                    for (i, c) in checked.iter_mut().enumerate() {
                        *c = only_visible || i == fi;
                    }
                }
            });
    });
    result.rect = Some(frame_resp.response.rect);

    result.hidden_items = entries
        .iter()
        .zip(checked.iter())
        .filter_map(|(e, c)| (!*c).then_some(e.id))
        .collect();
    Some(result)
}

/// Single legend row: color dot + label, styled like egui_plot's `LegendEntry`.
fn legend_entry_ui(
    ui: &mut Ui,
    label: &str,
    color: Color32,
    checked: bool,
    font_id: &egui::FontId,
) -> Response {
    let galley =
        ui.fonts_mut(|f| f.layout_delayed_color(label.to_owned(), font_id.clone(), f32::INFINITY));

    let icon_size = galley.size().y;
    let icon_spacing = icon_size / 5.0;
    let total_extra = vec2(icon_size + icon_spacing, 0.0);
    let desired_size = total_extra + galley.size();

    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    let visuals = ui.style().interact(&response);
    let label_on_the_left = ui.layout().horizontal_placement() == Align::RIGHT;

    let icon_position_x = if label_on_the_left {
        rect.right() - icon_size / 2.0
    } else {
        rect.left() + icon_size / 2.0
    };
    let icon_position = pos2(icon_position_x, rect.center().y);
    let icon_rect = Rect::from_center_size(icon_position, vec2(icon_size, icon_size));

    let painter = ui.painter();

    painter.add(CircleShape {
        center: icon_rect.center(),
        radius: icon_size * 0.35,
        fill: visuals.bg_fill,
        stroke: visuals.bg_stroke,
    });

    if checked {
        let fill = if color == Color32::TRANSPARENT {
            ui.visuals().noninteractive().fg_stroke.color
        } else {
            color
        };
        painter.add(Shape::circle_filled(
            icon_rect.center(),
            icon_size * 0.25,
            fill,
        ));
    }

    let text_position_x = if label_on_the_left {
        rect.right() - icon_size - icon_spacing - galley.size().x
    } else {
        rect.left() + icon_size + icon_spacing
    };
    let text_position = pos2(text_position_x, rect.center().y - 0.5 * galley.size().y);
    painter.galley(text_position, galley, visuals.text_color());

    response
}
