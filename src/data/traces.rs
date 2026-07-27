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
                                    })
                                }
                            };
                            entry.live.push_back([point.x, point.y]);
                            entry.envelope_add_point([point.x, point.y]);
                            if entry.live.len() > self.max_points {
                                if let Some(removed) = entry.live.pop_front() {
                                    entry.envelope_remove_point(removed);
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
                                }
                            });
                            entry.live.push_back([point.x, point.y]);
                            entry.envelope_add_point([point.x, point.y]);
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
                                    })
                                }
                            };
                            for p in points {
                                entry.live.push_back([p.x, p.y]);
                                entry.envelope_add_point([p.x, p.y]);
                            }
                            while entry.live.len() > self.max_points {
                                if let Some(removed) = entry.live.pop_front() {
                                    entry.envelope_remove_point(removed);
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
                                    })
                                }
                            };
                            entry.live.clear();
                            entry.envelope_cache = None;
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
    pub fn get_drawn_points_decimated(
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
            // No decimation needed — just filter by bounds
            return Some(
                source
                    .iter()
                    .filter(|p| p[0] >= bounds.0 && p[0] <= bounds.1)
                    .copied()
                    .collect(),
            );
        }
        // Stride decimation: pick every Nth point within bounds
        let stride = (len + max_pts - 1) / max_pts;
        let mut out = Vec::with_capacity(max_pts.min(len));
        let mut i = 0usize;
        while i < len {
            let p = source[i];
            if p[0] >= bounds.0 && p[0] <= bounds.1 {
                out.push(p);
            }
            i += stride;
        }
        // Always include the last point so the line doesn't appear truncated
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

        let visible_width = bounds.1 - bounds.0;
        if visible_width <= 0.0 || screen_width == 0 {
            return Some(Vec::new());
        }

        // Ensure cache is valid for current params
        if trace.envelope_needs_recompute(screen_width, visible_width) {
            trace.recompute_envelope(screen_width, visible_width);
        }

        let cache = trace.envelope_cache.as_ref()?;
        let mut out = Vec::new();

        for b in &cache.buckets {
            if b.count == 0 {
                continue;
            }
            // Skip buckets entirely outside the visible bounds
            if b.x_max < bounds.0 || b.x_min > bounds.1 {
                continue;
            }
            // Emit min and max points for this bucket
            out.push([b.x_min, b.y_min]);
            out.push([b.x_max, b.y_max]);
        }

        Some(out)
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
}

impl TraceData {
    pub fn prune_by_points(&mut self, max_points: usize) {
        while self.live.len() > max_points {
            if let Some(removed) = self.live.pop_front() {
                self.envelope_remove_point(removed);
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
                self.envelope_remove_point(front);
            } else {
                break;
            }
        }
    }

    pub fn clear_all(&mut self) {
        self.live.clear();
        self.snap = None;
        self.envelope_cache = None;
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
        if self.live.is_empty() || screen_width == 0 || visible_width <= 0.0 {
            self.envelope_cache = None;
            return;
        }
        if self.live.len() <= screen_width {
            // No envelope needed — few enough points to draw directly
            self.envelope_cache = None;
            return;
        }

        let data_min_x = self.live.front().map(|p| p[0]).unwrap_or(0.0);
        let data_max_x = self.live.back().map(|p| p[0]).unwrap_or(0.0);
        let bucket_width = visible_width / screen_width as f64;
        if bucket_width <= 0.0 {
            self.envelope_cache = None;
            return;
        }
        let data_range = data_max_x - data_min_x;
        let num_buckets = ((data_range / bucket_width).ceil() as usize).max(1);

        let mut buckets: VecDeque<EnvelopeBucket> = VecDeque::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push_back(EnvelopeBucket::default());
        }

        // Initialize bucket x-ranges
        for (i, b) in buckets.iter_mut().enumerate() {
            b.x_min = data_min_x + i as f64 * bucket_width;
            b.x_max = b.x_min + bucket_width;
        }

        // Assign each point to its bucket
        for &p in &self.live {
            let idx = ((p[0] - data_min_x) / bucket_width + 1e-9) as usize;
            let idx = idx.min(num_buckets - 1);
            let b = &mut buckets[idx];
            if b.count == 0 {
                b.y_min = p[1];
                b.y_max = p[1];
            } else {
                b.y_min = b.y_min.min(p[1]);
                b.y_max = b.y_max.max(p[1]);
            }
            b.count += 1;
        }

        self.envelope_cache = Some(EnvelopeCache {
            buckets,
            bucket_width,
            origin_x: data_min_x,
            screen_width,
            visible_width,
        });
    }

    /// Check if the envelope cache needs a full recompute for the given params.
    pub fn envelope_needs_recompute(&self, screen_width: usize, visible_width: f64) -> bool {
        match &self.envelope_cache {
            None => true,
            Some(c) => {
                c.screen_width != screen_width
                    || (c.visible_width - visible_width).abs() > visible_width * 0.001
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
            } else {
                b.y_min = b.y_min.min(point[1]);
                b.y_max = b.y_max.max(point[1]);
            }
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
                        count: 1,
                    });
                } else {
                    cache.buckets.push_back(EnvelopeBucket {
                        x_min,
                        x_max,
                        y_min: 0.0,
                        y_max: 0.0,
                        count: 0,
                    });
                }
            }
            // Rebalance check: if bucket count grew too large, mark for recompute
            if cache.buckets.len() > 2 * cache.screen_width {
                // Mark stale by clearing — will be recomputed on next render
                self.envelope_cache = None;
            }
        }
    }

    /// Incremental update on point remove — O(1) typical, O(bucket_size) worst case.
    /// Only recomputes the bucket if the removed point was its min or max.
    pub fn envelope_remove_point(&mut self, point: [f64; 2]) {
        let cache = match &mut self.envelope_cache {
            Some(c) => c,
            None => return,
        };

        let idx = match cache.bucket_index(point[0]) {
            Some(i) => i,
            None => return,
        };

        let b = &mut cache.buckets[idx];

        // If the removed point is neither min nor max, bucket is unchanged
        let was_min = b.count > 0 && (point[1] - b.y_min).abs() < f64::EPSILON;
        let was_max = b.count > 0 && (point[1] - b.y_max).abs() < f64::EPSILON;

        if b.count > 0 {
            b.count -= 1;
        }

        if b.count == 0 {
            // Bucket is now empty — reset it
            b.y_min = 0.0;
            b.y_max = 0.0;
            // Don't remove the bucket from the middle — just leave it empty.
            // Edge buckets will be handled by rebalance check.
        } else if was_min || was_max {
            // Need to rescan this bucket to find new min/max
            let bucket_x_min = b.x_min;
            let bucket_x_max = b.x_max;
            // Search the live VecDeque for points in this bucket's x-range
            let mut new_min = f64::INFINITY;
            let mut new_max = f64::NEG_INFINITY;
            let mut found = false;
            for &p in &self.live {
                if p[0] >= bucket_x_min && p[0] < bucket_x_max {
                    new_min = new_min.min(p[1]);
                    new_max = new_max.max(p[1]);
                    found = true;
                }
            }
            if found {
                b.y_min = new_min;
                b.y_max = new_max;
            }
        }

        // Rebalance check: if bucket count shrank too much, mark for recompute
        let non_empty_count = cache.buckets.iter().filter(|b| b.count > 0).count();
        if non_empty_count < cache.screen_width / 2 {
            self.envelope_cache = None;
        }
    }
}
