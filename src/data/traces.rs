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
    pub points_bounds: (usize, usize),
    pub hover_trace: Option<TraceRef>,
    rx: Option<std::sync::mpsc::Receiver<PlotCommand>>,
    /// Mapping from numeric trace ID to trace name (for PlotCommand API)
    id_to_name: HashMap<u32, String>,
    /// Pending styles for traces that haven't been created yet.
    /// When a trace is loaded from a saved state, the style is stored here
    /// until the trace is created from incoming data.
    pending_styles: HashMap<String, (TraceLook, f64)>,
}

impl Default for TracesCollection {
    fn default() -> Self {
        Self {
            traces: HashMap::new(),
            max_points: 10_000,
            points_bounds: (500, 200000),
            hover_trace: None,
            rx: None,
            id_to_name: HashMap::new(),
            pending_styles: HashMap::new(),
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

    /// Store a pending style for a trace that may not exist yet.
    /// When the trace is created from incoming data, this style will be applied
    /// instead of the default palette color.
    pub fn set_pending_style(&mut self, name: &str, look: TraceLook, offset: f64) {
        // If the trace already exists, apply immediately
        let tref = TraceRef(name.to_string());
        if let Some(tr) = self.traces.get_mut(&tref) {
            tr.look = look;
            tr.offset = offset;
        } else {
            self.pending_styles.insert(name.to_string(), (look, offset));
        }
    }

    fn update_rx(&mut self) -> Vec<TraceRef> {
        let mut new_traces: Vec<TraceRef> = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    PlotCommand::RegisterTrace { id, name, info } => {
                        self.id_to_name.insert(id, name.clone());
                        let tref = TraceRef(name.clone());
                        let new_index = self.traces.len();
                        let pending = self.pending_styles.remove(name.as_str());
                        let entry = match self.traces.entry(tref.clone()) {
                            Entry::Occupied(entry) => entry.into_mut(),
                            Entry::Vacant(entry) => {
                                new_traces.push(tref.clone());
                                let (look, offset) =
                                    pending.unwrap_or((TraceLook::new(new_index), 0.0));
                                entry.insert(TraceData {
                                    look,
                                    offset,
                                    live: VecDeque::new(),
                                    snap: None,
                                    info: String::new(),
                                    creation_index: new_index,
                                    #[cfg(feature = "fft")]
                                    last_fft: None,
                                })
                            }
                        };
                        if let Some(inf) = info {
                            entry.info = inf;
                        }
                    }
                    PlotCommand::SetTraceInfo { trace_id, info } => {
                        if let Some(name) = self.id_to_name.get(&trace_id) {
                            let tref = TraceRef(name.clone());
                            if let Some(entry) = self.traces.get_mut(&tref) {
                                entry.info = info;
                            }
                        }
                    }
                    PlotCommand::Point { trace_id, point } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name.clone());
                            let new_index = self.traces.len();
                            let pending = self.pending_styles.remove(name.as_str());
                            let entry = match self.traces.entry(tref.clone()) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => {
                                    new_traces.push(tref.clone());
                                    let (look, offset) =
                                        pending.unwrap_or((TraceLook::new(new_index), 0.0));
                                    entry.insert(TraceData {
                                        look,
                                        offset,
                                        live: VecDeque::new(),
                                        snap: None,
                                        info: String::new(),
                                        creation_index: new_index,
                                        #[cfg(feature = "fft")]
                                        last_fft: None,
                                    })
                                }
                            };
                            entry.live.push_back([point.x, point.y]);
                            if entry.live.len() > self.max_points {
                                entry.live.pop_front();
                            }
                        } else {
                            // Auto-register trace
                            let name = format!("trace-{}", trace_id);
                            self.id_to_name.insert(trace_id, name.clone());
                            let tref = TraceRef(name.clone());
                            let new_index = self.traces.len();
                            let pending = self.pending_styles.remove(name.as_str());
                            let entry = self.traces.entry(tref.clone()).or_insert_with(|| {
                                new_traces.push(tref.clone());
                                let (look, offset) =
                                    pending.unwrap_or((TraceLook::new(new_index), 0.0));
                                TraceData {
                                    look,
                                    offset,
                                    live: VecDeque::new(),
                                    snap: None,
                                    info: String::new(),
                                    creation_index: new_index,
                                    #[cfg(feature = "fft")]
                                    last_fft: None,
                                }
                            });
                            entry.live.push_back([point.x, point.y]);
                        }
                    }
                    PlotCommand::Points { trace_id, points } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name.clone());
                            let new_index = self.traces.len();
                            let pending = self.pending_styles.remove(name.as_str());
                            let entry = match self.traces.entry(tref.clone()) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => {
                                    new_traces.push(tref.clone());
                                    let (look, offset) =
                                        pending.unwrap_or((TraceLook::new(new_index), 0.0));
                                    entry.insert(TraceData {
                                        look,
                                        offset,
                                        live: VecDeque::new(),
                                        snap: None,
                                        info: String::new(),
                                        creation_index: new_index,
                                        #[cfg(feature = "fft")]
                                        last_fft: None,
                                    })
                                }
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
                            let tref = TraceRef(name.clone());
                            let new_index = self.traces.len();
                            let pending = self.pending_styles.remove(name.as_str());
                            let entry = match self.traces.entry(tref.clone()) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => {
                                    new_traces.push(tref.clone());
                                    let (look, offset) =
                                        pending.unwrap_or((TraceLook::new(new_index), 0.0));
                                    entry.insert(TraceData {
                                        look,
                                        offset,
                                        live: VecDeque::new(),
                                        snap: None,
                                        info: String::new(),
                                        creation_index: new_index,
                                        #[cfg(feature = "fft")]
                                        last_fft: None,
                                    })
                                }
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
        new_traces
    }

    fn drain(&mut self) {
        for (_name, trace) in self.traces.iter_mut() {
            trace.prune_by_points(self.max_points);
        }
    }

    pub fn update(&mut self) -> Vec<TraceRef> {
        let new_traces = self.update_rx();
        self.drain();
        new_traces
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
            let new_index = self.traces.len();
            let pending = self.pending_styles.remove(name.as_ref());
            let (look, offset) = pending.unwrap_or((TraceLook::new(new_index), 0.0));
            // note: later when the TraceData is created the `creation_index` is set
            // appropriately (see above insertion sites)
            self.traces.insert(
                name.clone(),
                TraceData {
                    look,
                    offset,
                    live: VecDeque::new(),
                    snap: None,
                    info: String::new(),
                    creation_index: new_index,
                    #[cfg(feature = "fft")]
                    last_fft: None,
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

    pub fn all_trace_names(&self) -> Vec<TraceRef> {
        self.traces.keys().cloned().collect()
    }

    /// Update every trace's colour to match the current global palette.
    ///
    /// This is called when the colour scheme changes so that existing traces
    /// (created before the scheme was applied) are recoloured appropriately.
    pub fn recolor_using_palette(&mut self) {
        let palette = crate::color_scheme::global_palette();
        if palette.is_empty() {
            return;
        }
        for (_name, tr) in self.traces.iter_mut() {
            let idx = tr.creation_index;
            tr.look.color = palette[idx % palette.len()];
        }
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
    /// Index assigned when the trace was created.  Used for deterministic
    /// colour allocation so that recolouring after a scheme change keeps the
    /// same order.
    pub creation_index: usize,
    /// Cached spectrum for the trace when the `fft` feature is enabled.
    ///
    /// The various constructors in this module previously filled this field
    /// during `cfg(feature = "fft")` builds, which led to compilation
    /// failures when the field was missing.  The value is not used anywhere
    /// outside of FFT-related code, so it is only included behind the same
    /// feature flag.
    #[cfg(feature = "fft")]
    pub last_fft: Option<VecDeque<[f64; 2]>>,
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

// --- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_scheme;
    use crate::sink::PlotCommand;
    use egui::Color32;

    #[test]
    fn recolor_changes_existing_traces() {
        // create collection with two traces
        let (tx, rx) = std::sync::mpsc::channel();
        let mut col = TracesCollection::new(rx);
        // register two traces via commands
        let _ = tx.send(PlotCommand::RegisterTrace {
            id: 1,
            name: "a".to_string(),
            info: None,
        });
        let _ = tx.send(PlotCommand::RegisterTrace {
            id: 2,
            name: "b".to_string(),
            info: None,
        });
        let new = col.update();
        assert_eq!(new.len(), 2);
        // initial palette must be default dark
        let first_color = col.traces.get(&TraceRef("a".into())).unwrap().look.color;
        assert_ne!(first_color, Color32::GRAY); // sanity
                                                // set a simple custom palette
        color_scheme::set_global_palette(vec![
            Color32::from_rgb(9, 9, 9),
            Color32::from_rgb(8, 8, 8),
        ]);
        col.recolor_using_palette();
        assert_eq!(
            col.traces.get(&TraceRef("a".into())).unwrap().look.color,
            Color32::from_rgb(9, 9, 9)
        );
        assert_eq!(
            col.traces.get(&TraceRef("b".into())).unwrap().look.color,
            Color32::from_rgb(8, 8, 8)
        );
    }
}
