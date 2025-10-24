// src/point_selection.rs
// Point selection logic for Sine Scope
// Extracted from main.rs for clarity and maintainability.

#[derive(Debug, Clone, Default)]
pub struct PointSelection {
    pub selected_p1: Option<[f64; 2]>,
    pub selected_p2: Option<[f64; 2]>,
}

impl PointSelection {
    /// Handle point selection logic on click (with absolute XY coordinate).
    pub fn handle_click_point(&mut self, point: [f64; 2]) {
        match (self.selected_p1, self.selected_p2) {
            (None, _) => {
                self.selected_p1 = Some(point);
            }
            (Some(p1), None) => {
                if (p1[0] - point[0]).abs() > f64::EPSILON
                    || (p1[1] - point[1]).abs() > f64::EPSILON
                {
                    self.selected_p2 = Some(point);
                } else {
                    self.selected_p1 = None;
                }
            }
            (Some(_), Some(_)) => {
                self.selected_p1 = Some(point);
                self.selected_p2 = None;
            }
        }
    }

    /// Clear both selections
    pub fn clear(&mut self) {
        self.selected_p1 = None;
        self.selected_p2 = None;
    }
}
