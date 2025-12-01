//! Measurement struct for point selection and delta calculation.
//!
//! MERGED from Janosch architecture - provides the Measurement type
//! used by MeasurementPanel for measuring distances on plots.

/// A measurement consisting of two points (P1 and P2) on a plot.
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Name of this measurement (e.g., "M1", "M2").
    pub name: String,
    /// First point [x, y], if set.
    pub point1: Option<[f64; 2]>,
    /// Second point [x, y], if set.
    pub point2: Option<[f64; 2]>,
    /// Which point to set on the next click (0 for P1, 1 for P2).
    next_point: usize,
}

impl Default for Measurement {
    fn default() -> Self {
        Self {
            name: "M".to_string(),
            point1: None,
            point2: None,
            next_point: 0,
        }
    }
}

impl Measurement {
    /// Create a new measurement with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Set the next point (alternates between P1 and P2).
    pub fn set_point(&mut self, p: [f64; 2]) {
        if self.next_point == 0 {
            self.point1 = Some(p);
            self.next_point = 1;
        } else {
            self.point2 = Some(p);
            self.next_point = 0;
        }
    }

    /// Explicitly set P1.
    pub fn set_point1(&mut self, p: [f64; 2]) {
        self.point1 = Some(p);
    }

    /// Explicitly set P2.
    pub fn set_point2(&mut self, p: [f64; 2]) {
        self.point2 = Some(p);
    }

    /// Get both points as (Option<P1>, Option<P2>).
    pub fn get_points(&self) -> (Option<[f64; 2]>, Option<[f64; 2]>) {
        (self.point1, self.point2)
    }

    /// Clear both points.
    pub fn clear(&mut self) {
        self.point1 = None;
        self.point2 = None;
        self.next_point = 0;
    }

    /// Check if both points are set.
    #[allow(dead_code)]
    pub fn has_both_points(&self) -> bool {
        self.point1.is_some() && self.point2.is_some()
    }
}
