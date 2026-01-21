use egui::Ui;
use egui_plot::LineStyle;

use crate::data::trace_look::TraceLook;

pub fn render_trace_look_editor(look: &mut TraceLook, ui: &mut Ui, allow_points: bool) {
    ui.horizontal(|ui| {
        ui.label("Color");
        let mut c = look.color;
        if ui.color_edit_button_srgba(&mut c).changed() {
            look.color = c;
        }
        ui.label("Width");
        ui.add(
            egui::DragValue::new(&mut look.width)
                .range(0.1..=10.0)
                .speed(0.1),
        );
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
    match &mut look.style {
        LineStyle::Dashed { length } => {
            ui.horizontal(|ui| {
                ui.label("Dash length");
                ui.add(egui::DragValue::new(length).range(0.5..=200.0).speed(0.5));
            });
        }
        LineStyle::Dotted { spacing } => {
            ui.horizontal(|ui| {
                ui.label("Dot spacing");
                ui.add(egui::DragValue::new(spacing).range(0.5..=200.0).speed(0.5));
            });
        }
        LineStyle::Solid => {}
    }
    if allow_points {
        ui.separator();
        ui.checkbox(&mut look.show_points, "Points");
        ui.checkbox(&mut look.highlight_newest_point, "Highlight newest point")
            .on_hover_text("Draw the newest sample as a larger marker in XY scopes");
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
                    egui_plot::MarkerShape::Circle => "Circle",
                    egui_plot::MarkerShape::Square => "Square",
                    egui_plot::MarkerShape::Diamond => "Diamond",
                    egui_plot::MarkerShape::Cross => "Cross",
                    egui_plot::MarkerShape::Plus => "Plus",
                    _ => "Other",
                })
                .show_ui(ui, |ui| {
                    let current = look.marker;
                    if ui
                        .selectable_label(
                            matches!(current, egui_plot::MarkerShape::Circle),
                            "Circle",
                        )
                        .clicked()
                    {
                        look.marker = egui_plot::MarkerShape::Circle;
                    }
                    if ui
                        .selectable_label(
                            matches!(current, egui_plot::MarkerShape::Square),
                            "Square",
                        )
                        .clicked()
                    {
                        look.marker = egui_plot::MarkerShape::Square;
                    }
                    if ui
                        .selectable_label(
                            matches!(current, egui_plot::MarkerShape::Diamond),
                            "Diamond",
                        )
                        .clicked()
                    {
                        look.marker = egui_plot::MarkerShape::Diamond;
                    }
                    if ui
                        .selectable_label(matches!(current, egui_plot::MarkerShape::Cross), "Cross")
                        .clicked()
                    {
                        look.marker = egui_plot::MarkerShape::Cross;
                    }
                    if ui
                        .selectable_label(matches!(current, egui_plot::MarkerShape::Plus), "Plus")
                        .clicked()
                    {
                        look.marker = egui_plot::MarkerShape::Plus;
                    }
                });
        });
    }
}
