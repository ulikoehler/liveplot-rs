use std::collections::VecDeque;

use crate::data::trace_look::TraceLook;

#[derive(Default)]
pub struct TraceData {
    pub name: String,
    pub look: TraceLook,
    pub offset: f64,
    pub live: VecDeque<[f64; 2]>,
    pub snap: Option<VecDeque<[f64; 2]>>,
    pub info: String,
}

impl TraceData {
    pub fn prune_by_points(&mut self, max_points: usize) {
        while self.live.len() > max_points {
            self.live.pop_front();
        }
    }

    pub fn clear_all(&mut self) {
        self.live.clear();
        self.snap = None;
    }

    pub fn take_snapshot(&mut self) {
        self.snap = Some(self.live.clone());
    }

    pub fn clear_snapshot(&mut self) {
        self.snap = None;
    }

    pub fn get_last_live_timestamp(&self) -> Option<f64> {
        self.live.back().map(|p| p[0])
    }

    pub fn get_last_snapshot_timestamp(&self) -> Option<f64> {
        self.snap.as_ref().and_then(|s| s.back().map(|p| p[0]))
    }

    pub fn cap_by_x_bounds(pts: &VecDeque<[f64; 2]>, bounds: (f64, f64)) -> VecDeque<[f64; 2]> {
        let capped: VecDeque<[f64; 2]> = pts
            .iter()
            .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
            .cloned()
            .collect();

        capped
    }
}
