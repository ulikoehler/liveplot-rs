use eframe::egui;
use egui_plot::{LineStyle, MarkerShape};

#[derive(Debug, Clone)]
pub(crate) struct TraceLook {
    pub visible: bool,

    // Line style
    pub color: egui::Color32,
    pub width: f32,
    pub style: egui_plot::LineStyle,

    // Point style
    pub show_points: bool,
    pub point_size: f32,
    pub marker: egui_plot::MarkerShape,
}

impl Default for TraceLook {
    fn default() -> Self {
        Self {
            visible: true,
            color: egui::Color32::WHITE,
            width: 1.5, // Updated from Janosch branch (was 1.0)
            style: egui_plot::LineStyle::Solid,
            show_points: false,
            point_size: 2.0,
            marker: egui_plot::MarkerShape::Circle,
        }
    }
}

/// Render an inline editor for a TraceLook.
///
/// Arguments:
/// - ui: egui Ui to render into
/// - allow_points: whether to show point-marker related controls
/// - label_prefix: optional prefix label (e.g., "Line" or "Events") to name sections
impl TraceLook {
    pub(crate) fn render_editor(
        &mut self,
        ui: &mut egui::Ui,
        allow_points: bool,
        label_prefix: Option<&str>,
        hide_color: bool,
        lock_color: Option<egui::Color32>,
    ) {
        if let Some(p) = label_prefix {
            ui.strong(p);
        }
        ui.horizontal(|ui| {
            if !hide_color {
                ui.label("Color");
                let mut c = self.color;
                if ui.color_edit_button_srgba(&mut c).changed() {
                    self.color = c;
                }
            }
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut self.width)
                    .range(0.1..=10.0)
                    .speed(0.1),
            );
        });
        egui::ComboBox::from_label("Line style")
            .selected_text(match self.style {
                LineStyle::Solid => "Solid",
                LineStyle::Dashed { .. } => "Dashed",
                LineStyle::Dotted { .. } => "Dotted",
            })
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(matches!(self.style, LineStyle::Solid), "Solid")
                    .clicked()
                {
                    self.style = LineStyle::Solid;
                }
                if ui
                    .selectable_label(matches!(self.style, LineStyle::Dashed { .. }), "Dashed")
                    .clicked()
                {
                    self.style = LineStyle::Dashed { length: 6.0 };
                }
                if ui
                    .selectable_label(matches!(self.style, LineStyle::Dotted { .. }), "Dotted")
                    .clicked()
                {
                    self.style = LineStyle::Dotted { spacing: 4.0 };
                }
            });

        // Additional controls for dashed/dotted parameters
        match &mut self.style {
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
            ui.checkbox(&mut self.show_points, "Points");
            ui.horizontal(|ui| {
                ui.label("Size");
                ui.add_enabled(
                    self.show_points,
                    egui::DragValue::new(&mut self.point_size)
                        .range(0.5..=10.0)
                        .speed(0.1),
                );
            });
            ui.add_enabled_ui(self.show_points, |ui| {
                egui::ComboBox::from_label("Marker shape")
                    .selected_text(match self.marker {
                        MarkerShape::Circle => "Circle",
                        MarkerShape::Square => "Square",
                        MarkerShape::Diamond => "Diamond",
                        MarkerShape::Cross => "Cross",
                        MarkerShape::Plus => "Plus",
                        _ => "Other",
                    })
                    .show_ui(ui, |ui| {
                        let current = self.marker;
                        if ui
                            .selectable_label(matches!(current, MarkerShape::Circle), "Circle")
                            .clicked()
                        {
                            self.marker = MarkerShape::Circle;
                        }
                        if ui
                            .selectable_label(matches!(current, MarkerShape::Square), "Square")
                            .clicked()
                        {
                            self.marker = MarkerShape::Square;
                        }
                        if ui
                            .selectable_label(matches!(current, MarkerShape::Diamond), "Diamond")
                            .clicked()
                        {
                            self.marker = MarkerShape::Diamond;
                        }
                        if ui
                            .selectable_label(matches!(current, MarkerShape::Cross), "Cross")
                            .clicked()
                        {
                            self.marker = MarkerShape::Cross;
                        }
                        if ui
                            .selectable_label(matches!(current, MarkerShape::Plus), "Plus")
                            .clicked()
                        {
                            self.marker = MarkerShape::Plus;
                        }
                    });
            });
        }

        if let Some(c) = lock_color {
            self.color = c;
        }
    }
}

// (legacy free function removed)

// (popup window editor removed; style editing is inline in panels now)
