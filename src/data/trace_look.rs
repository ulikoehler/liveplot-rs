use egui::Color32;
use egui_plot::{LineStyle, MarkerShape};

#[derive(Clone, Debug)]
pub struct TraceLook {
    pub color: Color32,
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
            color: Color32::LIGHT_BLUE,
            visible: true,
            width: 1.5,
            show_points: false,
            style: LineStyle::Solid,
            point_size: 2.0,
            marker: MarkerShape::Circle,
        }
    }
}
