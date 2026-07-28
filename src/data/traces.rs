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
    /// Maximum age in seconds for retained points.  0.0 disables time-based pruning.
    pub max_age_secs: f64,
    /// Slider bounds for `max_age_secs`.
    pub max_age_bounds: (f64, f64),
    pub hover_trace: Option<Vec<TraceRef>>,
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
            points_bounds: (100, 200000),
            max_age_secs: 0.0,
            max_age_bounds: (0.0, 3600.0),
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
                        let new_index = self.next_color_index();
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
                                    envelope_cache: None,
                                    decimation_cache: None,
                                    density_cache: None,
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
                            let new_index = self.next_color_index();
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
                                        envelope_cache: None,
                                        decimation_cache: None,
                                        density_cache: None,
                                    })
                                }
                            };
                            entry.live.push_back([point.x, point.y]);
                            if entry.snap.is_none() {
                                entry.envelope_add_point([point.x, point.y]);
                                entry.decimation_add_point([point.x, point.y]);
                                entry.density_add_point([point.x, point.y]);
                            }
                            if entry.live.len() > self.max_points {
                                if let Some(removed) = entry.live.pop_front() {
                                    if entry.snap.is_none() {
                                        entry.envelope_remove_point(removed);
                                        entry.decimation_remove_point(removed);
                                        entry.density_remove_point(removed);
                                    }
                                }
                            }
                        } else {
                            // Auto-register trace
                            let name = format!("trace-{}", trace_id);
                            self.id_to_name.insert(trace_id, name.clone());
                            let tref = TraceRef(name.clone());
                            let new_index = self.next_color_index();
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
                                    envelope_cache: None,
                                    decimation_cache: None,
                                    density_cache: None,
                                }
                            });
                            entry.live.push_back([point.x, point.y]);
                            if entry.snap.is_none() {
                                entry.envelope_add_point([point.x, point.y]);
                                entry.decimation_add_point([point.x, point.y]);
                                entry.density_add_point([point.x, point.y]);
                            }
                        }
                    }
                    PlotCommand::Points { trace_id, points } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name.clone());
                            let new_index = self.next_color_index();
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
                                        envelope_cache: None,
                                        decimation_cache: None,
                                        density_cache: None,
                                    })
                                }
                            };
                            for p in points {
                                entry.live.push_back([p.x, p.y]);
                                if entry.snap.is_none() {
                                    entry.envelope_add_point([p.x, p.y]);
                                    entry.decimation_add_point([p.x, p.y]);
                                    entry.density_add_point([p.x, p.y]);
                                }
                            }
                            while entry.live.len() > self.max_points {
                                if let Some(removed) = entry.live.pop_front() {
                                    if entry.snap.is_none() {
                                        entry.envelope_remove_point(removed);
                                        entry.decimation_remove_point(removed);
                                        entry.density_remove_point(removed);
                                    }
                                }
                            }
                        }
                    }
                    PlotCommand::SetData { trace_id, points } => {
                        if let Some(name) = self.id_to_name.get(&trace_id).cloned() {
                            let tref = TraceRef(name.clone());
                            let new_index = self.next_color_index();
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
                                        envelope_cache: None,
                                        decimation_cache: None,
                                        density_cache: None,
                                    })
                                }
                            };
                            entry.live.clear();
                            entry.envelope_cache = None;
                            entry.decimation_cache = None;
                            entry.density_cache = None;
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
                                tr.envelope_cache = None;
                                tr.decimation_cache = None;
                                tr.density_cache = None;
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
            trace.prune_by_age(self.max_age_secs);
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
            let new_index = self.next_color_index();
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
                    envelope_cache: None,
                    decimation_cache: None,
                    density_cache: None,
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

    /// Return a reference to the point buffer without cloning.
    /// When `snapshot` is true, returns the snapshot buffer if available,
    /// otherwise falls back to the live buffer.
    pub fn get_points_ref(&self, name: &TraceRef, snapshot: bool) -> Option<&VecDeque<[f64; 2]>> {
        let trace = self.traces.get(name)?;
        if snapshot {
            Some(trace.snap.as_ref().unwrap_or(&trace.live))
        } else {
            Some(&trace.live)
        }
    }

    /// Return decimated points for a trace, filtering by x-bounds and
    /// reducing to at most `max_pts` points.  This avoids cloning the
    /// full VecDeque — it iterates in-place and collects only the kept
    /// points into a Vec.
    pub fn get_drawn_points_uncached(
        &self,
        name: &TraceRef,
        snapshot: bool,
        bounds: (f64, f64),
        max_pts: usize,
    ) -> Option<Vec<[f64; 2]>> {
        let trace = self.traces.get(name)?;
        let source: &VecDeque<[f64; 2]> = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live)
        } else {
            &trace.live
        };
        let len = source.len();
        if len == 0 {
            return Some(Vec::new());
        }
        if len <= max_pts {
            return Some(
                source
                    .iter()
                    .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                    .copied()
                    .collect(),
            );
        }
        let stride = (len + max_pts - 1) / max_pts;
        let mut out = Vec::with_capacity(max_pts.min(len));
        for (i, &p) in source.iter().enumerate() {
            if i % stride == 0 && p[0] >= bounds.0 && p[0] <= bounds.1 {
                out.push(p);
            }
        }
        if let Some(&last) = source.back() {
            if last[0] >= bounds.0 && last[0] <= bounds.1 {
                if out.last() != Some(&last) {
                    out.push(last);
                }
            }
        }
        Some(out)
    }

    /// Return decimated points for a trace, filtering by x-bounds and
    /// reducing to at most `max_pts` points.  This avoids cloning the
    /// full VecDeque — it iterates in-place and collects only the kept
    /// points into a Vec.
    pub fn get_drawn_points_decimated(
        &mut self,
        name: &TraceRef,
        snapshot: bool,
        bounds: (f64, f64),
        max_pts: usize,
    ) -> Option<Vec<[f64; 2]>> {
        let trace = self.traces.get_mut(name)?;
        let len = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live).len()
        } else {
            trace.live.len()
        };
        if len == 0 {
            return Some(Vec::new());
        }
        if len <= max_pts {
            // No decimation needed — just filter by bounds
            let source: &VecDeque<[f64; 2]> = if snapshot {
                trace.snap.as_ref().unwrap_or(&trace.live)
            } else {
                &trace.live
            };
            return Some(
                source
                    .iter()
                    .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                    .copied()
                    .collect(),
            );
        }
        // Ensure decimation cache is valid
        if trace.decimation_needs_recompute(len, max_pts) {
            trace.recompute_decimation_from(snapshot, max_pts);
        }
        let cache = trace.decimation_cache.as_ref()?;
        // Filter cached selected points by bounds — no recompute on scroll
        let mut out: Vec<[f64; 2]> = cache
            .selected
            .iter()
            .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
            .copied()
            .collect();
        // Always include the last visible point so the line doesn't appear truncated
        let source: &VecDeque<[f64; 2]> = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live)
        } else {
            &trace.live
        };
        if let Some(&last) = source.back() {
            if last[0] >= bounds.0 && last[0] <= bounds.1 {
                if out.last() != Some(&last) {
                    out.push(last);
                }
            }
        }
        Some(out)
    }

    /// Return min/max envelope points for a trace, filtered by x-bounds.
    /// Uses an incremental cache that is maintained on point add/remove.
    /// Full recompute only on cache miss, screen_width change, or zoom.
    pub fn get_drawn_points_envelope(
        &mut self,
        name: &TraceRef,
        snapshot: bool,
        bounds: (f64, f64),
        visible_width: f64,
        screen_width: usize,
    ) -> Option<Vec<[f64; 2]>> {
        let trace = self.traces.get_mut(name)?;
        let source: &VecDeque<[f64; 2]> = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live)
        } else {
            &trace.live
        };
        let len = source.len();
        if len == 0 {
            return Some(Vec::new());
        }
        if len <= screen_width {
            // No envelope needed — return all points within bounds
            return Some(
                source
                    .iter()
                    .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                    .copied()
                    .collect(),
            );
        }

        if visible_width <= 0.0 || screen_width == 0 {
            return Some(Vec::new());
        }

        // Ensure cache is valid for current params
        if trace.envelope_needs_recompute(screen_width, visible_width, bounds) {
            // Build cache from the correct data source (snapshot when paused)
            let source_for_recompute: VecDeque<[f64; 2]> = if snapshot {
                trace.snap.as_ref().unwrap_or(&trace.live).clone()
            } else {
                trace.live.clone()
            };
            trace.recompute_envelope_from(
                &source_for_recompute,
                screen_width,
                visible_width,
                Some(bounds),
            );
        }

        let cache = trace.envelope_cache.as_ref()?;
        let mut out = Vec::new();

        // Find the index of the last non-empty bucket within bounds
        let last_visible_idx = cache
            .buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.count > 0 && !(b.x_max < bounds.0 || b.x_min > bounds.1))
            .map(|(i, _)| i)
            .last();

        for (idx, b) in cache.buckets.iter().enumerate() {
            if b.count == 0 {
                continue;
            }
            // Skip buckets entirely outside the visible bounds
            if b.x_max < bounds.0 || b.x_min > bounds.1 {
                continue;
            }
            // Emit first point always; emit last point only if not the final bucket
            out.push([b.x_first, b.y_first]);
            if b.count > 1 && Some(idx) != last_visible_idx {
                out.push([b.x_last, b.y_last]);
            }
        }

        Some(out)
    }

    /// Return min/max envelope points for a trace, filtered by x-bounds.
    /// Emits [x_min, y_min] and [x_max, y_max] per bucket to show the
    /// full vertical extent of the signal in each pixel-width bucket.
    pub fn get_drawn_points_minmax_envelope(
        &mut self,
        name: &TraceRef,
        snapshot: bool,
        bounds: (f64, f64),
        visible_width: f64,
        screen_width: usize,
    ) -> Option<Vec<[f64; 2]>> {
        let trace = self.traces.get_mut(name)?;
        let source: &VecDeque<[f64; 2]> = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live)
        } else {
            &trace.live
        };
        let len = source.len();
        if len == 0 {
            return Some(Vec::new());
        }
        if len <= screen_width {
            return Some(
                source
                    .iter()
                    .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                    .copied()
                    .collect(),
            );
        }

        if visible_width <= 0.0 || screen_width == 0 {
            return Some(Vec::new());
        }

        if trace.envelope_needs_recompute(screen_width, visible_width, bounds) {
            let source_for_recompute: VecDeque<[f64; 2]> = if snapshot {
                trace.snap.as_ref().unwrap_or(&trace.live).clone()
            } else {
                trace.live.clone()
            };
            trace.recompute_envelope_from(
                &source_for_recompute,
                screen_width,
                visible_width,
                Some(bounds),
            );
        }

        let cache = trace.envelope_cache.as_ref()?;
        let mut out = Vec::new();

        for b in &cache.buckets {
            if b.count == 0 {
                continue;
            }
            if b.x_max < bounds.0 || b.x_min > bounds.1 {
                continue;
            }
            out.push([b.x_at_ymin, b.y_min]);
            if b.count > 1 {
                out.push([b.x_at_ymax, b.y_max]);
            }
        }

        Some(out)
    }

    /// Ensure the density cache for a trace is valid and return a reference to it.
    /// Returns None if the trace doesn't exist or has no data.
    pub fn get_density_cache(
        &mut self,
        name: &TraceRef,
        snapshot: bool,
        _bounds: (f64, f64),
        visible_width: f64,
        screen_width: usize,
    ) -> Option<&DensityCache> {
        let trace = self.traces.get_mut(name)?;
        let source: &VecDeque<[f64; 2]> = if snapshot {
            trace.snap.as_ref().unwrap_or(&trace.live)
        } else {
            &trace.live
        };
        if source.is_empty() || screen_width == 0 {
            return None;
        }

        if visible_width <= 0.0 {
            return None;
        }

        if trace.density_needs_recompute(screen_width, visible_width) {
            trace.recompute_density(screen_width, visible_width);
        }

        trace.density_cache.as_ref()
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

    /// Find the first palette slot not currently used by any existing trace.
    ///
    /// Returns the index to use for `TraceLook::new(index)` so that a newly
    /// created trace gets a colour that doesn't collide with any existing
    /// trace.  If all palette slots are in use, returns a sequential index
    /// (based on the current trace count) so that consecutive new traces get
    /// distinct colours that wrap around the palette, rather than all
    /// collapsing to the same colour.
    pub fn next_color_index(&self) -> usize {
        let palette = crate::color_scheme::global_palette();
        if palette.is_empty() {
            return 0;
        }
        let pal_len = palette.len();
        let used: std::collections::HashSet<usize> = self
            .traces
            .values()
            .map(|tr| tr.creation_index % pal_len)
            .collect();
        for slot in 0..pal_len {
            if !used.contains(&slot) {
                return slot;
            }
        }
        self.traces.len()
    }

    /// Recolour traces to match their position in `order`.
    ///
    /// The Nth trace in `order` gets `palette[N % palette.len()]`.  Traces
    /// not present in `order` are left unchanged.
    pub fn recolor_by_order(&mut self, order: &[TraceRef]) {
        let palette = crate::color_scheme::global_palette();
        if palette.is_empty() {
            return;
        }
        for (i, name) in order.iter().enumerate() {
            if let Some(tr) = self.traces.get_mut(name) {
                tr.look.color = palette[i % palette.len()];
                tr.creation_index = i;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }
}

/// A single bucket in the min/max envelope cache.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvelopeBucket {
    /// X-range covered by this bucket.
    pub x_min: f64,
    pub x_max: f64,
    /// Min/max y-values of all samples in this bucket.
    pub y_min: f64,
    pub y_max: f64,
    /// Actual x position of the point with y_min.
    pub x_at_ymin: f64,
    /// Actual x position of the point with y_max.
    pub x_at_ymax: f64,
    /// First data point in this bucket (at its real x position).
    pub x_first: f64,
    pub y_first: f64,
    /// Last data point in this bucket (at its real x position).
    pub x_last: f64,
    pub y_last: f64,
    /// Number of samples in this bucket.
    pub count: usize,
}

/// Incremental min/max envelope cache for a trace.
///
/// Buckets cover the ENTIRE data range, not just the visible range.
/// `bucket_width` is derived from `visible_width / screen_width` so each
/// bucket ≈ 1 pixel on screen. Stays constant during scrolling (visible
/// range shifts but width stays = time_window). Changes on zoom or resize.
pub struct EnvelopeCache {
    /// Bucket i covers [origin_x + i*bucket_width, origin_x + (i+1)*bucket_width).
    pub buckets: VecDeque<EnvelopeBucket>,
    /// Fixed width of each bucket in data-x units.
    pub bucket_width: f64,
    /// X-value of the left edge of the first bucket.
    pub origin_x: f64,
    /// Screen width the cache was built for.
    pub screen_width: usize,
    /// Visible range width the cache was built for.
    pub visible_width: f64,
}

impl EnvelopeCache {
    /// Compute bucket index for a given x-value. Returns None if out of range.
    pub fn bucket_index(&self, x: f64) -> Option<usize> {
        if x < self.origin_x || self.bucket_width <= 0.0 {
            return None;
        }
        let idx = ((x - self.origin_x) / self.bucket_width + 1e-9) as usize;
        if idx < self.buckets.len() {
            Some(idx)
        } else {
            None
        }
    }
}

/// A column of y-bucket counts in the density grid.
pub struct DensityColumn {
    /// Count of points in each y-bucket for this x-bucket.
    pub counts: Vec<u16>,
}

/// 2D density grid cache for a trace.
///
/// Uses the same x-bucketing as `EnvelopeCache` (bucket_width = visible_width / screen_width).
/// Y-buckets cover the data's y-range with a fixed resolution (default 200 buckets).
/// Maintained incrementally on point add/remove. Full recompute only on cache miss,
/// screen_width change, visible_width change, or y-range expansion.
pub struct DensityCache {
    /// One column per x-bucket, each containing `num_y_buckets` counts.
    pub cells: Vec<DensityColumn>,
    /// X-bucket width in data units (= visible_width / screen_width).
    pub bucket_width: f64,
    /// Y-bucket height in data units.
    pub bucket_height: f64,
    /// X-value of the left edge of the first bucket.
    pub origin_x: f64,
    /// Y-value of the bottom edge of the first bucket.
    pub origin_y: f64,
    /// Number of x-buckets (grows as data extends).
    pub num_x_buckets: usize,
    /// Number of y-buckets (fixed at recompute time).
    pub num_y_buckets: usize,
    /// Screen width the cache was built for.
    pub screen_width: usize,
    /// Visible range width the cache was built for.
    pub visible_width: f64,
    /// Maximum count in any single cell (for normalization during rendering).
    pub max_count: u16,
}

impl DensityCache {
    /// Compute x-bucket index for a given x-value. Returns None if out of range.
    pub fn x_bucket_index(&self, x: f64) -> Option<usize> {
        if x < self.origin_x || self.bucket_width <= 0.0 {
            return None;
        }
        let idx = ((x - self.origin_x) / self.bucket_width + 1e-9) as usize;
        if idx < self.num_x_buckets {
            Some(idx)
        } else {
            None
        }
    }

    /// Compute y-bucket index for a given y-value. Returns None if out of range.
    pub fn y_bucket_index(&self, y: f64) -> Option<usize> {
        if y < self.origin_y || self.bucket_height <= 0.0 {
            return None;
        }
        let idx = ((y - self.origin_y) / self.bucket_height + 1e-9) as usize;
        if idx < self.num_y_buckets {
            Some(idx)
        } else {
            None
        }
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
    /// Incremental min/max envelope cache. Built lazily on first render
    /// request, then maintained incrementally on point add/remove.
    pub envelope_cache: Option<EnvelopeCache>,
    /// Stable decimation cache for line mode rendering.
    pub decimation_cache: Option<DecimationCache>,
    /// 2D density grid cache for density splatting render mode.
    pub density_cache: Option<DensityCache>,
}

/// Stable decimation cache for line mode rendering.
///
/// Stores every stride-th point so that the same physical points are selected
/// regardless of scroll position. `add_counter` is monotonic — never decremented
/// on removal — ensuring phase stability. Invalidated only when stride changes
/// (data length crosses a multiple of max_pts), snapshot taken/cleared, or data cleared.
pub struct DecimationCache {
    /// Every stride-th point from the source data.
    pub selected: VecDeque<[f64; 2]>,
    /// Stride used to build the cache.
    pub stride: usize,
    /// Max points target used to compute stride.
    pub max_pts: usize,
    /// Monotonic counter — incremented on every add, never decremented on removal.
    pub add_counter: usize,
}

impl TraceData {
    pub fn prune_by_points(&mut self, max_points: usize) {
        while self.live.len() > max_points {
            if let Some(removed) = self.live.pop_front() {
                if self.snap.is_none() {
                    self.envelope_remove_point(removed);
                    self.decimation_remove_point(removed);
                    self.density_remove_point(removed);
                }
            }
        }
    }

    /// Remove points whose X value is older than `max_age_secs` behind the
    /// newest point.  A value of `0.0` or negative disables this pruning.
    pub fn prune_by_age(&mut self, max_age_secs: f64) {
        if max_age_secs <= 0.0 {
            return;
        }
        let Some(newest_x) = self.live.back().map(|p| p[0]) else {
            return;
        };
        let cutoff = newest_x - max_age_secs;
        while let Some(&front) = self.live.front() {
            if front[0] < cutoff {
                self.live.pop_front();
                if self.snap.is_none() {
                    self.envelope_remove_point(front);
                    self.decimation_remove_point(front);
                    self.density_remove_point(front);
                }
            } else {
                break;
            }
        }
    }

    pub fn clear_all(&mut self) {
        self.live.clear();
        self.snap = None;
        self.envelope_cache = None;
        self.decimation_cache = None;
        self.density_cache = None;
    }

    pub fn take_snapshot(&mut self) {
        self.snap = Some(self.live.clone());
        // Clear caches so they rebuild from snapshot data on next render
        self.envelope_cache = None;
        self.decimation_cache = None;
        self.density_cache = None;
    }

    pub fn clear_snapshot(&mut self) {
        self.snap = None;
        // Clear caches so they rebuild from live data on next render
        self.envelope_cache = None;
        self.decimation_cache = None;
        self.density_cache = None;
    }

    // ── Decimation cache methods ────────────────────────────────────────

    /// Check if the decimation cache needs to be rebuilt.
    /// Returns true if cache is missing or stride has changed.
    pub fn decimation_needs_recompute(&self, source_len: usize, max_pts: usize) -> bool {
        match &self.decimation_cache {
            None => true,
            Some(c) => {
                if c.max_pts != max_pts {
                    return true;
                }
                let expected_stride = (source_len + max_pts - 1) / max_pts;
                c.stride != expected_stride
            }
        }
    }

    /// Rebuild the decimation cache from scratch.
    pub fn recompute_decimation_from(&mut self, snapshot: bool, max_pts: usize) {
        let cache_data = {
            let source: &VecDeque<[f64; 2]> = if snapshot {
                self.snap.as_ref().unwrap_or(&self.live)
            } else {
                &self.live
            };
            let len = source.len();
            if len == 0 || len <= max_pts {
                None
            } else {
                let stride = (len + max_pts - 1) / max_pts;
                let mut selected = VecDeque::with_capacity(len / stride + 1);
                for (i, &p) in source.iter().enumerate() {
                    if i % stride == 0 {
                        selected.push_back(p);
                    }
                }
                Some((selected, stride, len))
            }
        };
        self.decimation_cache = cache_data.map(|(selected, stride, add_counter)| DecimationCache {
            selected,
            stride,
            max_pts,
            add_counter,
        });
    }

    /// Incremental update on point add — O(1).
    pub fn decimation_add_point(&mut self, point: [f64; 2]) {
        let cache = match &mut self.decimation_cache {
            Some(c) => c,
            None => return,
        };
        cache.add_counter += 1;
        if cache.add_counter % cache.stride == 0 {
            cache.selected.push_back(point);
        }
    }

    /// Incremental update on point remove — O(1) typical.
    /// Pops from selected if the removed point matches the front.
    /// Also removes any stale selected points that no longer exist in live.
    pub fn decimation_remove_point(&mut self, point: [f64; 2]) {
        // Phase 1: Pop the front if it matches the removed point
        if let Some(cache) = &mut self.decimation_cache {
            if let Some(&front) = cache.selected.front() {
                if (front[0] - point[0]).abs() < f64::EPSILON
                    && (front[1] - point[1]).abs() < f64::EPSILON
                {
                    cache.selected.pop_front();
                }
            }
        }
        // Phase 2: Pop any stale selected points that are no longer in live
        // (can happen when a non-selected point was removed and the
        // selected front is now behind the live front)
        let live_front_x = self.live.front().map(|p| p[0]);
        if let Some(cache) = &mut self.decimation_cache {
            while let Some(&sel) = cache.selected.front() {
                if let Some(live_x) = live_front_x {
                    if sel[0] < live_x {
                        cache.selected.pop_front();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
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

    /// Filter by x-bounds and decimate to at most `max_pts` points.
    /// Returns a Vec suitable for passing directly to egui_plot.
    /// When the input has fewer points than `max_pts`, all points within
    /// bounds are returned.  When more, every Nth point is kept (stride
    /// = ceil(len / max_pts)) so the overall shape is preserved.
    pub fn cap_and_decimate(pts: &[[f64; 2]], bounds: (f64, f64), max_pts: usize) -> Vec<[f64; 2]> {
        let len = pts.len();
        if len <= max_pts {
            return pts
                .iter()
                .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                .copied()
                .collect();
        }
        let stride = (len + max_pts - 1) / max_pts;
        let mut out = Vec::with_capacity(max_pts.min(len));
        let mut i = 0;
        while i < len {
            let p = pts[i];
            if p[0] >= bounds.0 && p[0] <= bounds.1 {
                out.push(p);
            }
            i += stride;
        }
        // Always include the last point so the line doesn't appear truncated
        if let Some(last) = pts.last() {
            if last[0] >= bounds.0 && last[0] <= bounds.1 {
                if out.last() != Some(last) {
                    out.push(*last);
                }
            }
        }
        out
    }

    // ── Envelope cache methods ──────────────────────────────────────────

    /// Full O(n) recompute of the envelope cache. Called on cache miss,
    /// screen_width change, or visible_width (zoom) change.
    pub fn recompute_envelope(&mut self, screen_width: usize, visible_width: f64) {
        let live = self.live.clone();
        self.recompute_envelope_from(&live, screen_width, visible_width, None);
    }

    /// Full O(n) recompute of the envelope cache from a specific data source.
    /// Used to build the cache from snapshot data when paused.
    /// `bounds` is the visible x-range; when data exceeds the cache capacity,
    /// the cache is centered on the visible range instead of the most recent data.
    pub fn recompute_envelope_from(
        &mut self,
        source: &VecDeque<[f64; 2]>,
        screen_width: usize,
        visible_width: f64,
        bounds: Option<(f64, f64)>,
    ) {
        if source.is_empty() || screen_width == 0 || visible_width <= 0.0 {
            self.envelope_cache = None;
            return;
        }
        if source.len() <= screen_width {
            // No envelope needed — few enough points to draw directly
            self.envelope_cache = None;
            return;
        }

        let data_min_x = source.front().map(|p| p[0]).unwrap_or(0.0);
        let data_max_x = source.back().map(|p| p[0]).unwrap_or(0.0);
        let bucket_width = visible_width / screen_width as f64;
        if bucket_width <= 0.0 {
            self.envelope_cache = None;
            return;
        }
        let data_range = data_max_x - data_min_x;
        let max_buckets = 2 * screen_width;
        let (origin_x, num_buckets) = if data_range > max_buckets as f64 * bucket_width {
            // Data range exceeds cache capacity — center on visible range
            let origin = if let Some((b0, _)) = bounds {
                // Start one visible_width before the visible area for scroll margin
                (b0 - visible_width).max(data_min_x)
            } else {
                // No bounds info — fall back to most recent data
                data_max_x - max_buckets as f64 * bucket_width
            };
            (origin, max_buckets)
        } else {
            (
                data_min_x,
                ((data_range / bucket_width).ceil() as usize).max(1),
            )
        };

        let mut buckets: VecDeque<EnvelopeBucket> = VecDeque::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push_back(EnvelopeBucket::default());
        }

        // Initialize bucket x-ranges
        for (i, b) in buckets.iter_mut().enumerate() {
            b.x_min = origin_x + i as f64 * bucket_width;
            b.x_max = b.x_min + bucket_width;
        }

        // Assign each point to its bucket (skip points before origin_x)
        for &p in source {
            if p[0] < origin_x {
                continue;
            }
            let idx = ((p[0] - origin_x) / bucket_width + 1e-9) as usize;
            let idx = idx.min(num_buckets - 1);
            let b = &mut buckets[idx];
            if b.count == 0 {
                b.y_min = p[1];
                b.y_max = p[1];
                b.x_at_ymin = p[0];
                b.x_at_ymax = p[0];
                b.x_first = p[0];
                b.y_first = p[1];
            } else {
                if p[1] < b.y_min {
                    b.y_min = p[1];
                    b.x_at_ymin = p[0];
                }
                if p[1] > b.y_max {
                    b.y_max = p[1];
                    b.x_at_ymax = p[0];
                }
            }
            b.x_last = p[0];
            b.y_last = p[1];
            b.count += 1;
        }

        self.envelope_cache = Some(EnvelopeCache {
            buckets,
            bucket_width,
            origin_x,
            screen_width,
            visible_width,
        });
    }

    /// Check if the envelope cache needs a full recompute for the given params.
    /// Also checks if the visible bounds fall outside the cached range.
    pub fn envelope_needs_recompute(
        &self,
        screen_width: usize,
        visible_width: f64,
        bounds: (f64, f64),
    ) -> bool {
        match &self.envelope_cache {
            None => true,
            Some(c) => {
                if c.screen_width != screen_width
                    || (c.visible_width - visible_width).abs() > visible_width * 0.001
                {
                    return true;
                }
                // If cache covers all data (not capped), bounds check is unnecessary
                if c.buckets.len() < 2 * c.screen_width {
                    return false;
                }
                // Cache was capped — check if visible range is outside the cached range
                let cache_min = c.origin_x;
                let cache_max = c.origin_x + c.buckets.len() as f64 * c.bucket_width;
                bounds.0 < cache_min || bounds.1 > cache_max
            }
        }
    }

    /// Incremental update on point add — O(1), updates only the target bucket.
    pub fn envelope_add_point(&mut self, point: [f64; 2]) {
        let cache = match &mut self.envelope_cache {
            Some(c) => c,
            None => return,
        };

        let idx = if cache.bucket_width <= 0.0 {
            return;
        } else {
            ((point[0] - cache.origin_x) / cache.bucket_width + 1e-9) as usize
        };

        if idx < cache.buckets.len() {
            // Point falls in existing bucket
            let b = &mut cache.buckets[idx];
            if b.count == 0 {
                b.y_min = point[1];
                b.y_max = point[1];
                b.x_at_ymin = point[0];
                b.x_at_ymax = point[0];
                b.x_first = point[0];
                b.y_first = point[1];
            } else {
                if point[1] < b.y_min {
                    b.y_min = point[1];
                    b.x_at_ymin = point[0];
                }
                if point[1] > b.y_max {
                    b.y_max = point[1];
                    b.x_at_ymax = point[0];
                }
            }
            b.x_last = point[0];
            b.y_last = point[1];
            b.count += 1;
        } else {
            // Point is beyond current range — append gap buckets + target bucket
            let start = cache.buckets.len();
            for i in start..=idx {
                let x_min = cache.origin_x + i as f64 * cache.bucket_width;
                let x_max = x_min + cache.bucket_width;
                if i == idx {
                    cache.buckets.push_back(EnvelopeBucket {
                        x_min,
                        x_max,
                        y_min: point[1],
                        y_max: point[1],
                        x_at_ymin: point[0],
                        x_at_ymax: point[0],
                        x_first: point[0],
                        y_first: point[1],
                        x_last: point[0],
                        y_last: point[1],
                        count: 1,
                    });
                } else {
                    cache.buckets.push_back(EnvelopeBucket {
                        x_min,
                        x_max,
                        y_min: 0.0,
                        y_max: 0.0,
                        x_at_ymin: 0.0,
                        x_at_ymax: 0.0,
                        x_first: 0.0,
                        y_first: 0.0,
                        x_last: 0.0,
                        y_last: 0.0,
                        count: 0,
                    });
                }
            }
            // Rebalance: if bucket count grew too large, evict oldest buckets
            // instead of clearing the entire cache
            while cache.buckets.len() > 2 * cache.screen_width {
                cache.buckets.pop_front();
                cache.origin_x += cache.bucket_width;
            }
        }
    }

    /// Incremental update on point remove — O(1) typical, O(bucket_size) worst case.
    /// Only recomputes the bucket if the removed point was its min or max.
    pub fn envelope_remove_point(&mut self, point: [f64; 2]) {
        let origin_x = match &self.envelope_cache {
            Some(c) => c.origin_x,
            None => return,
        };

        let idx = match self.envelope_cache.as_ref().unwrap().bucket_index(point[0]) {
            Some(i) => i,
            None => {
                // Point is outside the cached range.
                // If it's before origin_x, the cache no longer matches the data
                // (points were evicted but still counted) — invalidate cache.
                if point[0] < origin_x {
                    self.envelope_cache = None;
                }
                return;
            }
        };

        let cache = self.envelope_cache.as_mut().unwrap();
        let b = &mut cache.buckets[idx];

        if b.count > 0 {
            b.count -= 1;
        }

        if b.count == 0 {
            // Bucket is now empty — reset it
            b.y_min = 0.0;
            b.y_max = 0.0;
            b.x_at_ymin = 0.0;
            b.x_at_ymax = 0.0;
            b.x_first = 0.0;
            b.y_first = 0.0;
            b.x_last = 0.0;
            b.y_last = 0.0;
            // Don't remove the bucket from the middle — just leave it empty.
            // Edge buckets will be handled by rebalance check.
        } else {
            // Need to rescan this bucket to find new min/max and first/last
            let bucket_x_min = b.x_min;
            let bucket_x_max = b.x_max;
            // Search the live VecDeque for points in this bucket's x-range
            let mut new_min = f64::INFINITY;
            let mut new_max = f64::NEG_INFINITY;
            let mut x_at_min = 0.0;
            let mut x_at_max = 0.0;
            let mut first_x = 0.0;
            let mut first_y = 0.0;
            let mut last_x = 0.0;
            let mut last_y = 0.0;
            let mut found = false;
            // Determine the correct data source to rescan
            let source: &VecDeque<[f64; 2]> = self.snap.as_ref().unwrap_or(&self.live);
            for &p in source {
                if p[0] >= bucket_x_min && p[0] < bucket_x_max {
                    if p[1] < new_min {
                        new_min = p[1];
                        x_at_min = p[0];
                    }
                    if p[1] > new_max {
                        new_max = p[1];
                        x_at_max = p[0];
                    }
                    if !found {
                        first_x = p[0];
                        first_y = p[1];
                    }
                    last_x = p[0];
                    last_y = p[1];
                    found = true;
                }
            }
            if found {
                b.y_min = new_min;
                b.y_max = new_max;
                b.x_at_ymin = x_at_min;
                b.x_at_ymax = x_at_max;
                b.x_first = first_x;
                b.y_first = first_y;
                b.x_last = last_x;
                b.y_last = last_y;
            }
        }

        // Pop empty buckets from the front of the cache
        while let Some(b) = cache.buckets.front() {
            if b.count == 0 {
                cache.buckets.pop_front();
                cache.origin_x += cache.bucket_width;
            } else {
                break;
            }
        }

        // Rebalance check: if more than half the buckets are empty, mark for recompute
        let total_buckets = cache.buckets.len();
        let non_empty_count = cache.buckets.iter().filter(|b| b.count > 0).count();
        if total_buckets > 0 && non_empty_count < total_buckets / 2 {
            self.envelope_cache = None;
        }
    }

    // ── Density cache methods ───────────────────────────────────────────

    /// Number of y-buckets for the density grid.
    const DENSITY_Y_BUCKETS: usize = 200;

    /// Full O(n) recompute of the density cache.
    pub fn recompute_density(&mut self, screen_width: usize, visible_width: f64) {
        if self.live.is_empty() || screen_width == 0 || visible_width <= 0.0 {
            self.density_cache = None;
            return;
        }

        let data_min_x = self.live.front().map(|p| p[0]).unwrap_or(0.0);
        let data_max_x = self.live.back().map(|p| p[0]).unwrap_or(0.0);
        let bucket_width = visible_width / screen_width as f64;
        if bucket_width <= 0.0 {
            self.density_cache = None;
            return;
        }

        // Compute y-range from data
        let mut data_min_y = f64::INFINITY;
        let mut data_max_y = f64::NEG_INFINITY;
        for &p in &self.live {
            data_min_y = data_min_y.min(p[1]);
            data_max_y = data_max_y.max(p[1]);
        }
        if !data_min_y.is_finite() || !data_max_y.is_finite() {
            self.density_cache = None;
            return;
        }
        // Add 1% padding to y-range to avoid edge points falling outside
        let y_range = (data_max_y - data_min_y).max(1e-9);
        let y_pad = y_range * 0.01;
        data_min_y -= y_pad;
        data_max_y += y_pad;
        let y_range_padded = data_max_y - data_min_y;
        let bucket_height = y_range_padded / Self::DENSITY_Y_BUCKETS as f64;

        let data_range = data_max_x - data_min_x;
        let num_x_buckets = ((data_range / bucket_width).ceil() as usize).max(1);
        let num_y_buckets = Self::DENSITY_Y_BUCKETS;

        let mut cells: Vec<DensityColumn> = Vec::with_capacity(num_x_buckets);
        for _ in 0..num_x_buckets {
            cells.push(DensityColumn {
                counts: vec![0u16; num_y_buckets],
            });
        }

        let mut max_count: u16 = 0;
        for &p in &self.live {
            let xi = ((p[0] - data_min_x) / bucket_width + 1e-9) as usize;
            let xi = xi.min(num_x_buckets - 1);
            let yi = ((p[1] - data_min_y) / bucket_height + 1e-9) as usize;
            let yi = yi.min(num_y_buckets - 1);
            let cell = &mut cells[xi];
            cell.counts[yi] = cell.counts[yi].saturating_add(1);
            if cell.counts[yi] > max_count {
                max_count = cell.counts[yi];
            }
        }

        self.density_cache = Some(DensityCache {
            cells,
            bucket_width,
            bucket_height,
            origin_x: data_min_x,
            origin_y: data_min_y,
            num_x_buckets,
            num_y_buckets,
            screen_width,
            visible_width,
            max_count,
        });
    }

    /// Check if the density cache needs a full recompute for the given params.
    pub fn density_needs_recompute(&self, screen_width: usize, visible_width: f64) -> bool {
        match &self.density_cache {
            None => true,
            Some(c) => {
                c.screen_width != screen_width
                    || (c.visible_width - visible_width).abs() > visible_width * 0.001
            }
        }
    }

    /// Incremental update on point add — O(1).
    pub fn density_add_point(&mut self, point: [f64; 2]) {
        let cache = match &mut self.density_cache {
            Some(c) => c,
            None => return,
        };

        let xi = if cache.bucket_width <= 0.0 {
            return;
        } else {
            ((point[0] - cache.origin_x) / cache.bucket_width + 1e-9) as usize
        };

        // Check if point is within existing y-range
        let yi = if point[1] < cache.origin_y
            || point[1] >= cache.origin_y + cache.num_y_buckets as f64 * cache.bucket_height
        {
            // Point outside y-range — need full recompute
            self.density_cache = None;
            return;
        } else {
            ((point[1] - cache.origin_y) / cache.bucket_height + 1e-9) as usize
        };

        if xi < cache.num_x_buckets {
            if yi < cache.num_y_buckets {
                let cell = &mut cache.cells[xi];
                cell.counts[yi] = cell.counts[yi].saturating_add(1);
                if cell.counts[yi] > cache.max_count {
                    cache.max_count = cell.counts[yi];
                }
            }
        } else {
            // Point beyond current x-range — extend with empty columns
            let start = cache.num_x_buckets;
            for _ in start..=xi {
                cache.cells.push(DensityColumn {
                    counts: vec![0u16; cache.num_y_buckets],
                });
            }
            cache.num_x_buckets = xi + 1;
            if yi < cache.num_y_buckets {
                let cell = &mut cache.cells[xi];
                cell.counts[yi] = 1;
                if cell.counts[yi] > cache.max_count {
                    cache.max_count = cell.counts[yi];
                }
            }
            // Rebalance check
            if cache.num_x_buckets > 2 * cache.screen_width {
                self.density_cache = None;
            }
        }
    }

    /// Incremental update on point remove — O(1).
    /// Note: we don't rescan for max_count on remove; it's updated on next recompute.
    pub fn density_remove_point(&mut self, point: [f64; 2]) {
        let cache = match &mut self.density_cache {
            Some(c) => c,
            None => return,
        };

        let xi = match cache.x_bucket_index(point[0]) {
            Some(i) => i,
            None => return,
        };
        let yi = match cache.y_bucket_index(point[1]) {
            Some(i) => i,
            None => return,
        };

        let cell = &mut cache.cells[xi];
        if cell.counts[yi] > 0 {
            cell.counts[yi] -= 1;
        }
    }
}
