//! TraceRef and TracesCollection: trace identity and data management.

use crate::data::trace_look::TraceLook;
use crate::sink::PlotCommand;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap, VecDeque};

/// Identifier for a trace by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TraceRef(pub String);

impl Default for TraceRef {
    fn default() -> Self {
        TraceRef("".to_string())
    }
}

impl TraceRef {
    pub fn new<S: Into<String>>(name: S) -> Self {
        TraceRef(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for TraceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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

impl PartialEq<String> for TraceRef {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<TraceRef> for String {
    fn eq(&self, other: &TraceRef) -> bool {
        self == &other.0
    }
}

impl PartialEq<&str> for TraceRef {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
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

impl From<TraceRef> for String {
    fn from(value: TraceRef) -> Self {
        value.0
    }
}

/// Collection of all traces with their data.
pub struct TracesCollection {
    traces: HashMap<TraceRef, TraceData>,
    pub max_points: usize,
    rx: Option<std::sync::mpsc::Receiver<PlotCommand>>,
    /// Mapping from numeric trace ID to trace name (for PlotCommand API)
    id_to_name: HashMap<u32, String>,
}

impl Default for TracesCollection {
    fn default() -> Self {
        Self {
            traces: HashMap::new(),
            max_points: 10_000,
            rx: None,
            id_to_name: HashMap::new(),
        }
    }
}

impl TracesCollection {
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        let mut instance = Self::default();
        instance.set_rx(rx);
        instance
    }

    pub fn set_rx(&mut self, rx: std::sync::mpsc::Receiver<PlotCommand>) {
        self.rx = Some(rx);
    }

    fn update_rx(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    PlotCommand::RegisterTrace { id, name, info } => {
                        self.id_to_name.insert(id, name.clone());
                        let tref = TraceRef(name.clone());
                        let new_index = self.traces.len();
                        let entry = match self.traces.entry(tref) {
                            Entry::Occupied(entry) => entry.into_mut(),
                            Entry::Vacant(entry) => entry.insert(TraceData {
                                look: TraceLook::new(new_index),
                                offset: 0.0,
                                live: VecDeque::new(),
                                snap: None,
                                info: String::new(),
                                #[cfg(feature = "fft")]
                                last_fft: None,
                                is_math: false,
                            }),
                        };
                        if let Some(inf) = info {
                            entry.info = inf;
                        }
                    }
                    PlotCommand::Point { trace_id, point } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            let new_index = self.traces.len();
                            let entry = match self.traces.entry(tref) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => entry.insert(TraceData {
                                    look: TraceLook::new(new_index),
                                    offset: 0.0,
                                    live: VecDeque::new(),
                                    snap: None,
                                    info: String::new(),
                                    #[cfg(feature = "fft")]
                                    last_fft: None,
                                    is_math: false,
                                }),
                            };
                            entry.live.push_back([point.x, point.y]);
                            if entry.live.len() > self.max_points {
                                entry.live.pop_front();
                            }
                        } else {
                            // Auto-register trace
                            let name = format!("trace-{}", trace_id);
                            self.id_to_name.insert(trace_id, name.clone());
                            let tref = TraceRef(name);
                            let new_index = self.traces.len();
                            let entry = self.traces.entry(tref).or_insert_with(|| TraceData {
                                look: TraceLook::new(new_index),
                                offset: 0.0,
                                live: VecDeque::new(),
                                snap: None,
                                info: String::new(),
                                #[cfg(feature = "fft")]
                                last_fft: None,
                                is_math: false,
                            });
                            entry.live.push_back([point.x, point.y]);
                        }
                    }
                    PlotCommand::Points { trace_id, points } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            let new_index = self.traces.len();
                            let entry = match self.traces.entry(tref) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => entry.insert(TraceData {
                                    look: TraceLook::new(new_index),
                                    offset: 0.0,
                                    live: VecDeque::new(),
                                    snap: None,
                                    info: String::new(),
                                    #[cfg(feature = "fft")]
                                    last_fft: None,
                                    is_math: false,
                                }),
                            };
                            for p in points {
                                entry.live.push_back([p.x, p.y]);
                            }
                            while entry.live.len() > self.max_points {
                                entry.live.pop_front();
                            }
                        }
                    }
                    PlotCommand::SetData { trace_id, points } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            let new_index = self.traces.len();
                            let entry = match self.traces.entry(tref) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => entry.insert(TraceData {
                                    look: TraceLook::new(new_index),
                                    offset: 0.0,
                                    live: VecDeque::new(),
                                    snap: None,
                                    info: String::new(),
                                    #[cfg(feature = "fft")]
                                    last_fft: None,
                                    is_math: false,
                                }),
                            };
                            entry.live.clear();
                            for p in points {
                                entry.live.push_back([p.x, p.y]);
                            }
                        }
                    }
                    PlotCommand::ClearData { trace_id } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                tr.live.clear();
                            }
                        }
                    }
                    PlotCommand::SetPointsY { trace_id, xs, y } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                for pt in tr.live.iter_mut() {
                                    if xs.iter().any(|&x| (x - pt[0]).abs() < 1e-12) {
                                        pt[1] = y;
                                    }
                                }
                            }
                        }
                    }
                    PlotCommand::DeletePointsX { trace_id, xs } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                tr.live
                                    .retain(|pt| !xs.iter().any(|&x| (x - pt[0]).abs() < 1e-12));
                            }
                        }
                    }
                    PlotCommand::DeleteXRange {
                        trace_id,
                        x_min,
                        x_max,
                    } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                tr.live.retain(|pt| pt[0] < x_min || pt[0] > x_max);
                            }
                        }
                    }
                    PlotCommand::ApplyYFnAtX { trace_id, xs, f } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                for pt in tr.live.iter_mut() {
                                    if xs.iter().any(|&x| (x - pt[0]).abs() < 1e-12) {
                                        pt[1] = f(pt[1]);
                                    }
                                }
                            }
                        }
                    }
                    PlotCommand::ApplyYFnInXRange {
                        trace_id,
                        x_min,
                        x_max,
                        f,
                    } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name);
                            if let Some(tr) = self.traces.get_mut(&tref) {
                                for pt in tr.live.iter_mut() {
                                    if pt[0] >= x_min && pt[0] <= x_max {
                                        pt[1] = f(pt[1]);
                                    }
                                }
                            }
                        }
                    }
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

    pub fn clear_trace(&mut self, name: &TraceRef) {
        if let Some(trace) = self.traces.get_mut(name) {
            trace.clear_all();
        }
    }

    pub fn clear_all(&mut self) {
        for trace in self.traces.values_mut() {
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
                    #[cfg(feature = "fft")]
                    last_fft: None,
                    is_math: false,
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

    pub fn traces_iter_mut(&mut self) -> impl Iterator<Item = (&TraceRef, &mut TraceData)> {
        self.traces.iter_mut()
    }

    pub fn get_trace(&self, name: &TraceRef) -> Option<&TraceData> {
        self.traces.get(name)
    }

    pub fn get_trace_mut(&mut self, name: &TraceRef) -> Option<&mut TraceData> {
        self.traces.get_mut(name)
    }

    pub fn contains_key(&self, name: &TraceRef) -> bool {
        self.traces.contains_key(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &TraceRef> {
        self.traces.keys()
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }
}

/// Per-trace data: live buffer, optional snapshot, and styling.
#[derive(Default)]
pub struct TraceData {
    pub look: TraceLook,
    pub offset: f64,
    pub live: VecDeque<[f64; 2]>,
    pub snap: Option<VecDeque<[f64; 2]>>,
    pub info: String,
    /// Cached last computed FFT (frequency, magnitude)
    #[cfg(feature = "fft")]
    pub last_fft: Option<Vec<[f64; 2]>>,
    /// Whether this trace is a derived math trace
    pub is_math: bool,
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
        pts.iter()
            .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
            .cloned()
            .collect()
    }
}
