use crate::data::trace_look::TraceLook;
use crate::sink::MultiSample;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TraceRef(pub String);

impl Default for TraceRef {
    fn default() -> Self {
        TraceRef("".to_string())
    }
}

impl std::cmp::Ord for TraceRef {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::cmp::PartialOrd for TraceRef {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq<str> for TraceRef {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl std::cmp::PartialOrd<str> for TraceRef {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        Some(self.0.as_str().cmp(other))
    }
}

impl std::ops::Deref for TraceRef {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for TraceRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for TraceRef {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TraceRef {
    fn from(s: &str) -> Self {
        TraceRef(s.to_string())
    }
}

impl From<String> for TraceRef {
    fn from(s: String) -> Self {
        TraceRef(s)
    }
}

pub struct TracesCollection {
    traces: HashMap<TraceRef, TraceData>,
    pub max_points: usize,
    rx: Option<std::sync::mpsc::Receiver<MultiSample>>,
}

impl Default for TracesCollection {
    fn default() -> Self {
        Self {
            traces: HashMap::new(),
            max_points: 10_000,
            rx: None,
        }
    }
}

impl TracesCollection {
    pub fn set_rx(&mut self, rx: std::sync::mpsc::Receiver<MultiSample>) {
        self.rx = Some(rx);
    }

    fn update_rx(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(s) = rx.try_recv() {
                let name = TraceRef(s.trace.clone());
                let new_index = self.traces.len();
                let entry = match self.traces.entry(name) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(TraceData {
                        look: TraceLook::new(new_index),
                        offset: 0.0,
                        live: VecDeque::new(),
                        snap: None,
                        info: String::new(),
                    }),
                };
                let t = s.timestamp_micros as f64 * 1e-6;
                entry.live.push_back([t, s.value]);
                if entry.live.len() > self.max_points {
                    entry.live.pop_front();
                }
                if let Some(inf) = s.info {
                    entry.info = inf;
                }
            }
        }
    }

    fn drain(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.prune_by_points(self.max_points);
        }
    }

    pub fn update(&mut self) {
        self.update_rx();
        self.drain();
    }

    pub fn take_snapshot(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.take_snapshot();
        }
    }

    pub fn clear_snapshot(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.clear_snapshot();
        }
    }

    pub fn has_snapshot(&self) -> bool {
        self.traces.values().any(|tr| tr.snap.is_some())
    }

    pub fn clear_traces(&mut self, name: &TraceRef) {
        if let Some(trace) = self.traces.get_mut(name) {
            trace.clear_all();
        }
    }

    pub fn remove_trace(&mut self, name: &TraceRef) {
        self.traces.remove(name);
    }

    pub fn get_trace_or_new(&mut self, name: &TraceRef) -> &mut TraceData {
        if !self.traces.contains_key(name) {
            self.traces.insert(
                name.clone(),
                TraceData {
                    look: TraceLook::new(self.traces.len()),
                    offset: 0.0,
                    live: VecDeque::new(),
                    snap: None,
                    info: String::new(),
                },
            );
        }
        self.traces.get_mut(name).unwrap()
    }

    pub fn get_points(&self, name: &TraceRef, snapshot: bool) -> Option<VecDeque<[f64; 2]>> {
        if let Some(trace) = self.traces.get(name) {
            if snapshot {
                if let Some(snap) = &trace.snap {
                    Some(snap.clone())
                } else {
                    Some(trace.live.clone())
                }
            } else {
                Some(trace.live.clone())
            }
        } else {
            None
        }
    }

    pub fn get_all_points(&self, snapshot: bool) -> HashMap<TraceRef, VecDeque<[f64; 2]>> {
        let mut result = HashMap::new();
        for (name, _) in self.traces.iter() {
            if let Some(pts) = self.get_points(name, snapshot) {
                result.insert(name.clone(), pts);
            }
        }
        result
    }

    pub fn traces_iter(&self) -> impl Iterator<Item = (&TraceRef, &TraceData)> {
        self.traces.iter()
    }

    pub fn get_trace(&self, name: &TraceRef) -> Option<&TraceData> {
        self.traces.get(name)
    }

    pub fn get_trace_mut(&mut self, name: &TraceRef) -> Option<&mut TraceData> {
        self.traces.get_mut(name)
    }
}

#[derive(Default)]
pub struct TraceData {
    //pub name: String,
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
