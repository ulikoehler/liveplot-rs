pub struct Measurement {
    pub name: String,
    pub p1: Option<[f64; 2]>,
    pub p2: Option<[f64; 2]>,
}

impl Default for Measurement {
    fn default() -> Self {
        Self {
            name: String::new(),
            p1: None,
            p2: None,
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

    /// Handle point selection logic on click (with absolute XY coordinate).
    pub fn set_point(&mut self, point: [f64; 2]) {
        match (self.p1, self.p2) {
            (None, _) => {
                self.p1 = Some(point);
            }
            (Some(p1), None) => {
                if (p1[0] - point[0]).abs() > f64::EPSILON
                    || (p1[1] - point[1]).abs() > f64::EPSILON
                {
                    self.p2 = Some(point);
                } else {
                    self.p1 = None;
                }
            }
            (Some(_), Some(_)) => {
                self.p1 = Some(point);
                self.p2 = None;
            }
        }
    }

    pub fn set_point1(&mut self, point: [f64; 2]) {
        self.p1 = Some(point);
    }

    pub fn set_point2(&mut self, point: [f64; 2]) {
        self.p2 = Some(point);
    }

    pub fn get_points(&self) -> (Option<[f64; 2]>, Option<[f64; 2]>) {
        (self.p1, self.p2)
    }

    /// Clear both selections
    pub fn clear(&mut self) {
        self.p1 = None;
        self.p2 = None;
    }

    pub fn has_both_points(&self) -> bool {
        self.p1.is_some() && self.p2.is_some()
    }
}
