// src/point_selection.rs
// Point selection logic for Sine Scope
// Extracted from main.rs for clarity and maintainability.

#[derive(Debug, Clone, Default)]
pub struct PointSelection {
    pub selected_idx1: Option<usize>,
    pub selected_idx2: Option<usize>,
}

impl PointSelection {
    /// Adjust selection indices after N elements were removed from the front of the buffer.
    /// Rules:
    ///  * If a selected index < removed count -> that point left the buffer.
    ///  * If P1 removed and P2 still present -> promote P2 to P1, clear P2.
    ///  * If only P2 removed -> just clear P2.
    ///  * Remaining indices shift down by removed count.
    pub fn adjust_for_front_removal(&mut self, removed: usize) {
        if removed == 0 { return; }
        let mut p1 = self.selected_idx1;
        let mut p2 = self.selected_idx2;
        // Helper to shift or invalidate an index
        let shift = |opt: &mut Option<usize>| {
            if let Some(i) = *opt {
                if i < removed { *opt = None; } else { *opt = Some(i - removed); }
            }
        };
        shift(&mut p1);
        shift(&mut p2);
        // Promotion logic
        if p1.is_none() && p2.is_some() {
            p1 = p2;
            p2 = None;
        }
        self.selected_idx1 = p1;
        self.selected_idx2 = p2;
    }

    /// Invalidate selections if indices out of range after pruning/live update
    pub fn invalidate_out_of_range(&mut self, len: usize) {
        if let Some(i) = self.selected_idx1 { if i >= len { self.selected_idx1 = None; } }
        if let Some(i) = self.selected_idx2 { if i >= len { self.selected_idx2 = None; } }
    }

    /// Handle point selection logic on click
    pub fn handle_click(&mut self, best_i: usize) {
        match (self.selected_idx1, self.selected_idx2) {
            (None, _) => { self.selected_idx1 = Some(best_i); },
            (Some(i1), None) => {
                if best_i != i1 { self.selected_idx2 = Some(best_i); } else { self.selected_idx1 = None; }
            },
            (Some(_), Some(_)) => {
                self.selected_idx1 = Some(best_i);
                self.selected_idx2 = None;
            }
        }
    }

    /// Clear both selections
    pub fn clear(&mut self) {
        self.selected_idx1 = None;
        self.selected_idx2 = None;
    }
}
