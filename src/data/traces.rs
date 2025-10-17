use std::collections::{HashMap, VecDeque};

use egui::Color32;

use crate::sink::MultiSample;
use crate::data::trace_look::TraceLook;

#[derive(Default)]
pub struct TraceState {
    pub name: String,
    pub look: TraceLook,
    pub offset: f64,
    pub live: VecDeque<[f64;2]>,
    pub snap: Option<VecDeque<[f64;2]>>,
    pub info: String,
}

#[derive(Default)]
pub struct TracesData {
    pub y_unit: Option<String>,
    pub y_log: bool,
    pub max_points: usize,
    pub time_window: f64,
    pub show_info_in_legend: bool,
    pub rx: Option<std::sync::mpsc::Receiver<MultiSample>>,
    pub traces: HashMap<String, TraceState>,
    pub trace_order: Vec<String>,
    pub selection_trace: Option<String>,
}

impl TracesData {
    pub fn set_rx(&mut self, rx: std::sync::mpsc::Receiver<MultiSample>) { self.rx = Some(rx); }

    pub fn drain_and_update(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(s) = rx.try_recv() {
                let is_new = !self.traces.contains_key(&s.trace);
                let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                    self.trace_order.push(s.trace.clone());
                    TraceState { name: s.trace.clone(), look: Self::alloc_color(self.trace_order.len()-1), offset: 0.0, live: VecDeque::new(), snap: None, info: String::new() }
                });
                if is_new && self.selection_trace.is_none() { self.selection_trace = Some(s.trace.clone()); }
                let t = s.timestamp_micros as f64 * 1e-6;
                entry.live.push_back([t, s.value]);
                if entry.live.len() > self.max_points { entry.live.pop_front(); }
                if let Some(inf) = s.info { entry.info = inf; }
            }
        }
    }

    pub fn prune_by_time_window(&mut self) {
        for (_k, tr) in self.traces.iter_mut() {
            if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                let cutoff = t_latest - self.time_window * 1.15;
                while let Some(&[t, _]) = tr.live.front() { if t < cutoff { tr.live.pop_front(); } else { break; } }
            }
        }
    }

    fn alloc_color(idx: usize) -> TraceLook {
        // Simple palette
        let palette = [
            Color32::from_rgb(0x3b,0x82,0xf6), // blue-500
            Color32::from_rgb(0x10,0xb9,0x81), // emerald-500
            Color32::from_rgb(0xf5,0x93,0x00), // amber-500
            Color32::from_rgb(0xef,0x44,0x44), // red-500
            Color32::from_rgb(0x8b,0x5c,0xff), // violet-500
        ];
        let color = palette[idx % palette.len()];
        TraceLook { color, ..Default::default() }
    }
}
