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
            color: Color32::WHITE,
            visible: true,
            width: 1.5,
            show_points: false,
            style: LineStyle::Solid,
            point_size: 2.0,
            marker: MarkerShape::Circle,
        }
    }
}

impl TraceLook {
    pub fn alloc_color(index: usize) -> Color32 {
        // Simple distinct color palette
        const PALETTE: [Color32; 10] = [
            Color32::LIGHT_BLUE,
            Color32::LIGHT_RED,
            Color32::LIGHT_GREEN,
            Color32::GOLD,
            Color32::from_rgb(0xAA, 0x55, 0xFF), // purple
            Color32::from_rgb(0xFF, 0xAA, 0x00), // orange
            Color32::from_rgb(0x00, 0xDD, 0xDD), // cyan
            Color32::from_rgb(0xDD, 0x00, 0xDD), // magenta
            Color32::from_rgb(0x66, 0xCC, 0x66), // green2
            Color32::from_rgb(0xCC, 0x66, 0x66), // red2
        ];
        PALETTE[index % PALETTE.len()]
    }

    pub fn new(index: usize) -> Self {
        Self {
            color: Self::alloc_color(index),
            ..Default::default()
        }
    }
}
