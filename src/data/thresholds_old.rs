use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

use serde::{Deserialize, Serialize};

use super::traces::TracesData;
use crate::data::math::TraceRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdKind {
    GreaterThan { value: f64 },
    LessThan { value: f64 },
    InRange { low: f64, high: f64 },
}

impl ThresholdKind {
    #[inline]
    pub fn is_active(&self, v: f64) -> bool {
        match self {
            ThresholdKind::GreaterThan { value } => v > *value,
            ThresholdKind::LessThan { value } => v < *value,
            ThresholdKind::InRange { low, high } => v >= *low && v <= *high,
        }
    }
    /// Excess used for area accumulation (>= 0 when active)
    #[inline]
    pub fn excess(&self, v: f64) -> f64 {
        match self {
            ThresholdKind::GreaterThan { value } => (v - *value).max(0.0),
            ThresholdKind::LessThan { value } => (*value - v).max(0.0),
            ThresholdKind::InRange { low, high: _ } => {
                if v >= *low { (v - *low).max(0.0) } else { 0.0 }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdDef {
    pub name: String,
    pub display_name: Option<String>,
    pub target: TraceRef,
    pub kind: ThresholdKind,
    /// Optional color hint (RGB) for UI overlays
    pub color_hint: Option<[u8; 3]>,
    pub min_duration_s: f64,
    pub max_events: usize,
}

impl Default for ThresholdDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            display_name: None,
            target: TraceRef(String::new()),
            kind: ThresholdKind::GreaterThan { value: 0.0 },
            color_hint: None,
            min_duration_s: 0.002,
            max_events: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEvent {
    pub threshold: String,
    pub trace: String,
    pub start_t: f64,
    pub end_t: f64,
    pub duration: f64,
    pub area: f64,
}

#[derive(Debug, Default, Clone)]
pub struct ThresholdRuntimeState {
    pub active: bool,
    pub start_t: f64,
    pub last_t: Option<f64>,
    pub last_excess: f64,
    pub accum_area: f64,
    pub events: VecDeque<ThresholdEvent>,
}

impl ThresholdRuntimeState {
    pub fn new() -> Self { Self { active: false, start_t: 0.0, last_t: None, last_excess: 0.0, accum_area: 0.0, events: VecDeque::new() } }
    pub fn push_event_capped(&mut self, evt: ThresholdEvent, cap: usize) {
        self.events.push_back(evt);
        while self.events.len() > cap { self.events.pop_front(); }
    }
}

#[derive(Clone)]
pub struct ThresholdController { pub(crate) inner: Arc<Mutex<ThresholdCtrlInner>> }
pub(crate) struct ThresholdCtrlInner {
    pub(crate) add_requests: Vec<ThresholdDef>,
    pub(crate) remove_requests: Vec<String>,
    pub(crate) listeners: Vec<Sender<ThresholdEvent>>,
}
impl ThresholdController {
    pub fn new() -> Self { Self { inner: Arc::new(Mutex::new(ThresholdCtrlInner { add_requests: Vec::new(), remove_requests: Vec::new(), listeners: Vec::new() })) } }
    pub fn add_threshold(&self, def: ThresholdDef) { self.inner.lock().unwrap().add_requests.push(def); }
    pub fn remove_threshold<S: Into<String>>(&self, name: S) { self.inner.lock().unwrap().remove_requests.push(name.into()); }
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<ThresholdEvent> { let (tx, rx) = std::sync::mpsc::channel(); self.inner.lock().unwrap().listeners.push(tx); rx }
}

#[derive(Default)]
pub struct ThresholdsData {
    pub defs: Vec<ThresholdDef>,
    pub state: HashMap<String, ThresholdRuntimeState>,
    pub event_log: Vec<ThresholdEvent>,
    pub controller: Option<ThresholdController>,
}

impl ThresholdsData {
    pub fn attach_controller(&mut self, ctrl: ThresholdController) { self.controller = Some(ctrl); }
    pub fn add_def(&mut self, def: ThresholdDef) {
        if !self.defs.iter().any(|d| d.name == def.name) {
            self.defs.push(def);
        }
    }
    pub fn remove_def(&mut self, name: &str) {
        self.defs.retain(|d| d.name != name);
        self.state.remove(name);
        // Keep events in log; UI may clear explicitly
    }
    pub fn clear_events_for(&mut self, name: &str) {
        if let Some(s) = self.state.get_mut(name) { s.events.clear(); }
        self.event_log.retain(|e| e.threshold != name);
    }
    pub fn clear_all_events(&mut self) {
        for s in self.state.values_mut() { s.events.clear(); }
        self.event_log.clear();
    }

    pub fn remove_event(&mut self, ev: &ThresholdEvent) {
        // Remove from global log (first equal match) and from the threshold's local deque
        if let Some(pos) = self.event_log.iter().position(|e| e.threshold == ev.threshold && e.start_t == ev.start_t && e.end_t == ev.end_t && e.trace == ev.trace) {
            self.event_log.remove(pos);
        }
        if let Some(st) = self.state.get_mut(&ev.threshold) {
            if let Some(pos) = st.events.iter().position(|e| e.start_t == ev.start_t && e.end_t == ev.end_t && e.trace == ev.trace) {
                st.events.remove(pos);
            }
        }
    }

    pub fn calculate(&mut self, traces: &TracesData) {
        // Apply controller requests
        if let Some(ctrl) = &self.controller {
            let (adds, removes) = {
                let mut inner = ctrl.inner.lock().unwrap();
                (inner.add_requests.drain(..).collect::<Vec<_>>(), inner.remove_requests.drain(..).collect::<Vec<_>>())
            };
            for def in adds { self.add_def(def); }
            for name in removes { self.remove_def(&name); }
        }

        // Evaluate each threshold against its target trace
        // Build source snapshots for quick access
        for def in self.defs.clone() {
            let name = def.name.clone();
            let target_name = def.target.0.clone();
            let data = if let Some(tr) = traces.traces.get(&target_name) {
                // Use live data (VecDeque) as slice
                tr.live.iter().copied().collect::<Vec<[f64;2]>>()
            } else { continue };

            let st = self.state.entry(name.clone()).or_insert_with(ThresholdRuntimeState::new);
            let mut last_t = st.last_t;
            let mut last_excess = st.last_excess;
            let mut active = st.active;
            let mut start_t = st.start_t;
            let mut accum_area = st.accum_area;

            // Find start index: first point strictly after last_t to avoid double count
            let mut start_idx = 0usize;
            if let Some(t0) = last_t { start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) { Ok(mut i)=>{ while i < data.len() && data[i][0] <= t0 { i += 1; } i }, Err(i)=>i } }

            for p in data.iter().skip(start_idx) {
                let t = p[0];
                let v = p[1];
                let exc = def.kind.excess(v);
                if active {
                    // integrate trapezoidal on excess
                    if let Some(t0) = last_t { let dt = t - t0; if dt > 0.0 { accum_area += 0.5 * (exc + last_excess) * dt; } }
                    if exc <= 0.0 {
                        // end event
                        let end_t = t;
                        let duration = end_t - start_t;
                        if duration >= def.min_duration_s {
                            let evt = ThresholdEvent { threshold: name.clone(), trace: target_name.clone(), start_t, end_t, duration, area: accum_area };
                            st.push_event_capped(evt.clone(), def.max_events);
                            self.event_log.push(evt.clone());
                            // Notify listeners
                            if let Some(ctrl) = &self.controller { let mut inner = ctrl.inner.lock().unwrap(); inner.listeners.retain(|tx| tx.send(evt.clone()).is_ok()); }
                        }
                        active = false;
                        accum_area = 0.0;
                    }
                } else {
                    if exc > 0.0 {
                        // start event
                        active = true;
                        start_t = t;
                        accum_area = 0.0;
                    }
                }
                last_t = Some(t);
                last_excess = exc;
            }

            // Persist state
            st.last_t = last_t;
            st.last_excess = last_excess;
            st.active = active;
            st.start_t = start_t;
            st.accum_area = accum_area;
        }
    }
}
