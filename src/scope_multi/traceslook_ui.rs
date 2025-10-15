use eframe::egui;
use egui_plot::{LineStyle, MarkerShape};

use super::types::TraceLook;

/// Render an inline editor for a TraceLook.
///
/// Arguments:
/// - ui: egui Ui to render into
/// - look: the TraceLook to edit (mutable)
/// - allow_points: whether to show point-marker related controls
/// - label_prefix: optional prefix label (e.g., "Line" or "Events") to name sections
pub(crate) fn trace_look_editor_inline(
    ui: &mut egui::Ui,
    look: &mut TraceLook,
    allow_points: bool,
    label_prefix: Option<&str>,
    hide_color: bool,
    lock_color: Option<egui::Color32>,
) {
    if let Some(p) = label_prefix { ui.strong(p); }
    ui.horizontal(|ui| {
        if !hide_color {
            ui.label("Color");
            let mut c = look.color;
            if ui.color_edit_button_srgba(&mut c).changed() { look.color = c; }
        }
        ui.label("Width");
        ui.add(egui::DragValue::new(&mut look.width).range(0.1..=10.0).speed(0.1));
    });
    egui::ComboBox::from_label("Line style")
        .selected_text(match look.style {
            LineStyle::Solid => "Solid",
            LineStyle::Dashed { .. } => "Dashed",
            LineStyle::Dotted { .. } => "Dotted",
        })
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(matches!(look.style, LineStyle::Solid), "Solid")
                .clicked()
            {
                look.style = LineStyle::Solid;
            }
            if ui
                .selectable_label(matches!(look.style, LineStyle::Dashed { .. }), "Dashed")
                .clicked()
            {
                look.style = LineStyle::Dashed { length: 6.0 };
            }
            if ui
                .selectable_label(matches!(look.style, LineStyle::Dotted { .. }), "Dotted")
                .clicked()
            {
                look.style = LineStyle::Dotted { spacing: 4.0 };
            }
        });

    // Additional controls for dashed/dotted parameters
    match &mut look.style {
        LineStyle::Dashed { length } => {
            ui.horizontal(|ui| {
                ui.label("Dash length");
                ui.add(egui::DragValue::new(length).range(0.5..=200.0).speed(0.5))
                    .on_hover_text("Length of dash segments");
            });
        }
        LineStyle::Dotted { spacing } => {
            ui.horizontal(|ui| {
                ui.label("Dot spacing");
                ui.add(egui::DragValue::new(spacing).range(0.5..=200.0).speed(0.5))
                    .on_hover_text("Space between dots");
            });
        }
        LineStyle::Solid => {}
    }

    if allow_points {
        ui.separator();
        ui.checkbox(&mut look.show_points, "Points");
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add_enabled(
                look.show_points,
                egui::DragValue::new(&mut look.point_size)
                    .range(0.5..=10.0)
                    .speed(0.1),
            );
        });
        ui.add_enabled_ui(look.show_points, |ui| {
            egui::ComboBox::from_label("Marker shape")
                .selected_text(match look.marker {
                    MarkerShape::Circle => "Circle",
                    MarkerShape::Square => "Square",
                    MarkerShape::Diamond => "Diamond",
                    MarkerShape::Cross => "Cross",
                    MarkerShape::Plus => "Plus",
                    _ => "Other",
                })
                .show_ui(ui, |ui| {
                    let current = look.marker;
                    if ui
                        .selectable_label(matches!(current, MarkerShape::Circle), "Circle")
                        .clicked()
                    {
                        look.marker = MarkerShape::Circle;
                    }
                    if ui
                        .selectable_label(matches!(current, MarkerShape::Square), "Square")
                        .clicked()
                    {
                        look.marker = MarkerShape::Square;
                    }
                    if ui
                        .selectable_label(matches!(current, MarkerShape::Diamond), "Diamond")
                        .clicked()
                    {
                        look.marker = MarkerShape::Diamond;
                    }
                    if ui
                        .selectable_label(matches!(current, MarkerShape::Cross), "Cross")
                        .clicked()
                    {
                        look.marker = MarkerShape::Cross;
                    }
                    if ui
                        .selectable_label(matches!(current, MarkerShape::Plus), "Plus")
                        .clicked()
                    {
                        look.marker = MarkerShape::Plus;
                    }
                });
        });
    }

    if let Some(c) = lock_color { look.color = c; }
}

// (popup window editor removed; style editing is inline in panels now)
