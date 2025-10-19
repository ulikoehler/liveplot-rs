use std::collections::{HashMap, VecDeque};

use egui::Color32;

use crate::sink::MultiSample;
use crate::data::trace_look::TraceLook;

#[derive(Default)]
pub struct TraceData {
    pub name: String,
    pub look: TraceLook,
    pub offset: f64,
    pub live: VecDeque<[f64;2]>,
    pub snap: Option<VecDeque<[f64;2]>>,
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
}
