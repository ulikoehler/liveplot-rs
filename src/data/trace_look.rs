//! TraceLook: visual styling for plot traces.

use eframe::egui;
use egui_plot::{LineStyle, MarkerShape};

/// The visual presentation of a trace (color, visibility, line style, markers).
#[derive(Debug, Clone)]
pub struct TraceLook {
    pub color: egui::Color32,
    pub visible: bool,
    pub width: f32,
    pub show_points: bool,
    pub style: LineStyle,
    pub point_size: f32,
    pub marker: MarkerShape,
}

impl Default for TraceLook {
    fn default() -> Self {
        Self {
            color: egui::Color32::GRAY,
            visible: true,
            width: 1.5,
            show_points: false,
            style: LineStyle::Solid,
            point_size: 4.0,
            marker: MarkerShape::Circle,
        }
    }
}

impl TraceLook {
    /// Create a new TraceLook with a color allocated based on the trace index.
    pub fn new(index: usize) -> Self {
        Self {
            color: Self::alloc_color(index),
            ..Default::default()
        }
    }

    /// Allocate a distinct color for the given trace index.
    pub fn alloc_color(index: usize) -> egui::Color32 {
        const PALETTE: [egui::Color32; 10] = [
            egui::Color32::from_rgb(31, 119, 180),
            egui::Color32::from_rgb(255, 127, 14),
            egui::Color32::from_rgb(44, 160, 44),
            egui::Color32::from_rgb(214, 39, 40),
            egui::Color32::from_rgb(148, 103, 189),
            egui::Color32::from_rgb(140, 86, 75),
            egui::Color32::from_rgb(227, 119, 194),
            egui::Color32::from_rgb(127, 127, 127),
            egui::Color32::from_rgb(188, 189, 34),
            egui::Color32::from_rgb(23, 190, 207),
        ];
        PALETTE[index % PALETTE.len()]
    }
}

// --- UI helpers (moved from root trace_look module) ---
impl TraceLook {
    /// Render an inline editor for a TraceLook.
    ///
    /// Arguments:
    /// - ui: egui Ui to render into
    /// - allow_points: whether to show point-marker related controls
    /// - label_prefix: optional prefix label (e.g., "Line" or "Events") to name sections
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
