//! Thresholds: detect when a single trace exceeds a condition and record events.
//!
//! A threshold monitors one source trace and creates events when the signal
//! exceeds a condition continuously for at least `min_duration_s` (default 2 ms).
//! For `GreaterThan`, the event area integrates (value - threshold).
//! For `LessThan`, the event area integrates (threshold - value).
//! For `InRange`, the event area integrates (value - low), while the event is
//! active only while `low <= value <= high`.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
// use std::sync::mpsc::Sender;
// use std::sync::{Arc, Mutex};

use crate::data::trace_look::TraceLook;
use crate::data::scope::AxisSettings;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRef(pub String);

/// Threshold condition kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdKind {
    /// Event when value > `value`
    GreaterThan { value: f64 },
    /// Event when value < `value`
    LessThan { value: f64 },
    /// Event when `low <= value <= high`
    InRange { low: f64, high: f64 },
}

impl ThresholdKind {
    /// Compute the "excess" at value v relative to the threshold definition.
    /// Excess is >= 0 when the threshold condition holds, 0 when not.
    #[inline]
    pub fn excess(&self, v: f64) -> f64 {
        match self {
            ThresholdKind::GreaterThan { value } => (v - *value).max(0.0),
            ThresholdKind::LessThan { value } => (*value - v).max(0.0),
            ThresholdKind::InRange { low, high } => {
                if v >= *low && v <= *high {
                    (v - *low).max(0.0)
                } else {
                    0.0
                }
            }
        }
    }

    /// Whether the condition holds at value v
    #[inline]
    pub fn is_active(&self, v: f64) -> bool {
        self.excess(v) > 0.0
    }
}

/// Definition of a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdDef {
    /// Unique name for this threshold.
    pub name: String,
    /// Optional display name for UI/legend; falls back to `name` when None/empty.
    /// Source trace to monitor.
    pub target: TraceRef,
    /// Condition to test.
    pub kind: ThresholdKind,
    /// Optional color hint for rendering this threshold (RGB).
    #[serde(skip)]
    pub look: TraceLook,
    #[serde(skip)]
    pub start_look: TraceLook,
    #[serde(skip)]
    pub stop_look: TraceLook,
    /// Minimum duration (seconds) for an event to be recorded. Default 0.002 s.
    pub min_duration_s: f64,
    /// Maximum number of events to keep (oldest dropped). Default 100.
    pub max_events: usize,

    #[serde(skip)]
    runtime_state: ThresholdRuntimeState,
}

impl Default for ThresholdDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            target: TraceRef(String::new()),
            kind: ThresholdKind::GreaterThan { value: 0.0 },
            look: TraceLook::default(),
            start_look: TraceLook::default(),
            stop_look: TraceLook::default(),
            min_duration_s: 0.002,
            max_events: 100,
            runtime_state: ThresholdRuntimeState::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEvent {
    pub threshold: String,
    pub trace: String,
    pub start_t: f64,
    pub end_t: f64,
    /// Duration in seconds.
    pub duration: f64,
    /// Integrated area (see module docs for definition per kind).
    pub area: f64,
}

/// Runtime state for evaluating a threshold incrementally.
#[derive(Debug, Clone)]
pub struct ThresholdRuntimeState {
    active: bool,
    pub start_t: f64,
    pub last_t: Option<f64>,
    pub last_excess: f64,
    pub accum_area: f64,
    pub prev_in_t: Option<f64>,
    /// Ring buffer of recent events (cap enforced per def.max_events)
    pub events: VecDeque<ThresholdEvent>,
}

impl Default for ThresholdRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            start_t: 0.0,
            last_t: None,
            last_excess: 0.0,
            accum_area: 0.0,
            prev_in_t: None,
            events: VecDeque::new(),
        }
    }
}

impl ThresholdRuntimeState {
    /// Push, enforcing a cap.
    pub fn push_event_capped(&mut self, evt: ThresholdEvent, cap: usize) {
        self.events.push_back(evt);
        while self.events.len() > cap {
            self.events.pop_front();
        }
    }
}

impl ThresholdDef {
    pub fn get_info(&self, axis_setting: &AxisSettings) -> String {
        let dec_pl = 4usize;
        match &self.kind {
            ThresholdKind::GreaterThan { value } => {
                let v_fmt = axis_setting.format_value(*value, dec_pl, value.abs());
                format!("{} > {}", self.target.0, v_fmt)
            }
            ThresholdKind::LessThan { value } => {
                let v_fmt = axis_setting.format_value(*value, dec_pl, value.abs());
                format!("{} < {}", self.target.0, v_fmt)
            }
            ThresholdKind::InRange { low, high } => {
                let diff = (*high - *low).abs();
                let lo = axis_setting.format_value(*low, dec_pl, diff);
                let hi = axis_setting.format_value(*high, dec_pl, diff);
                format!("{} in [{}, {}]", self.target.0, lo, hi)
            }
        }
    }

    pub fn clear_threshold_events(&mut self) {
        self.runtime_state.events.clear();
    }

    pub fn count_threshold_events(&self) -> usize {
        self.runtime_state.events.len()
    }

    pub fn get_last_threshold_event(&self) -> Option<ThresholdEvent> {
        self.runtime_state.events.back().cloned()
    }

    pub fn get_threshold_events(&self) -> Vec<ThresholdEvent> {
        self.runtime_state.events.iter().cloned().collect()
    }

    pub fn get_runtime_state(&self) -> &ThresholdRuntimeState {
        &self.runtime_state
    }

    /// Process new data points for this threshold, updating its runtime state and recording events.

    pub fn process_threshold(&mut self, sources: HashMap<String, VecDeque<[f64; 2]>>) {
        let data = sources.get(&self.target.0).unwrap();

        let mut start_idx = 0usize;
        if let Some(t0) = self.runtime_state.prev_in_t {
            start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                Ok(mut i) => {
                    while i < data.len() && data[i][0] <= t0 {
                        i += 1;
                    }
                    i
                }
                Err(i) => i,
            };
        }
        for p in data.iter().skip(start_idx) {
            let t = p[0];
            let v = p[1];
            let e = self.kind.excess(v);
            if let Some(t0) = self.runtime_state.last_t {
                let dt = (t - t0).max(0.0);
                if self.runtime_state.active || e > 0.0 {
                    self.runtime_state.accum_area +=
                        0.5 * (self.runtime_state.last_excess + e) * dt;
                }
            }
            if !self.runtime_state.active && e > 0.0 {
                self.runtime_state.active = true;
                self.runtime_state.start_t = t;
            } else if self.runtime_state.active && e == 0.0 {
                let end_t = t;
                let dur = end_t - self.runtime_state.start_t;
                if dur >= self.min_duration_s {
                    let evt = ThresholdEvent {
                        threshold: self.name.clone(),
                        trace: self.target.0.clone(),
                        start_t: self.runtime_state.start_t,
                        end_t,
                        duration: dur,
                        area: self.runtime_state.accum_area,
                    };
                    self.runtime_state
                        .push_event_capped(evt.clone(), self.max_events);
                    // self.threshold_event_log.push_back(evt.clone());
                    // while self.threshold_event_log.len() > self.threshold_event_log_cap {
                    //     self.threshold_event_log.pop_front();
                    // }
                    // if let Some(ctrl) = &self.threshold_controller {
                    //     let mut inner = ctrl.inner.lock().unwrap();
                    //     inner.listeners.retain(|s| s.send(evt.clone()).is_ok());
                    // }
                }
                self.runtime_state.active = false;
                self.runtime_state.accum_area = 0.0;
            }
            self.runtime_state.last_t = Some(t);
            self.runtime_state.last_excess = e;
            self.runtime_state.prev_in_t = Some(t);
        }
    }
}

// impl LivePlotApp {
//     /// Apply threshold controller add/remove requests.
//     pub(super) fn apply_threshold_controller_requests(&mut self) {
//         if let Some(ctrl) = &self.threshold_controller {
//             let (adds, removes) = {
//                 let mut inner = ctrl.inner.lock().unwrap();
//                 let adds: Vec<ThresholdDef> = inner.add_requests.drain(..).collect();
//                 let removes: Vec<String> = inner.remove_requests.drain(..).collect();
//                 (adds, removes)
//             };
//             for def in adds {
//                 self.add_threshold_internal(def);
//             }
//             for name in removes {
//                 self.remove_threshold_internal(&name);
//             }
//         }
//     }

//     pub(super) fn process_thresholds(&mut self) {
//         if self.threshold_defs.is_empty() {
//             return;
//         }
//         let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
//         for (name, tr) in &self.traces {
//             let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
//                 if let Some(s) = &tr.snap {
//                     Box::new(s.iter())
//                 } else {
//                     Box::new(tr.live.iter())
//                 }
//             } else {
//                 Box::new(tr.live.iter())
//             };
//             sources.insert(name.clone(), iter.cloned().collect());
//         }
//         for def in self.threshold_defs.clone().iter() {
//             let state = self
//                 .threshold_states
//                 .entry(def.name.clone())
//                 .or_insert_with(ThresholdRuntimeState::new);
//             let data = match sources.get(&def.target.0) {
//                 Some(v) => v,
//                 None => continue,
//             };
//             let mut start_idx = 0usize;
//             if let Some(t0) = state.prev_in_t {
//                 start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
//                     Ok(mut i) => {
//                         while i < data.len() && data[i][0] <= t0 {
//                             i += 1;
//                         }
//                         i
//                     }
//                     Err(i) => i,
//                 };
//             }
//             for p in data.iter().skip(start_idx) {
//                 let t = p[0];
//                 let v = p[1];
//                 let e = def.kind.excess(v);
//                 if let Some(t0) = state.last_t {
//                     let dt = (t - t0).max(0.0);
//                     if state.active || e > 0.0 {
//                         state.accum_area += 0.5 * (state.last_excess + e) * dt;
//                     }
//                 }
//                 if !state.active && e > 0.0 {
//                     state.active = true;
//                     state.start_t = t;
//                 } else if state.active && e == 0.0 {
//                     let end_t = t;
//                     let dur = end_t - state.start_t;
//                     if dur >= def.min_duration_s {
//                         let evt = ThresholdEvent {
//                             threshold: def.name.clone(),
//                             trace: def.target.0.clone(),
//                             start_t: state.start_t,
//                             end_t,
//                             duration: dur,
//                             area: state.accum_area,
//                         };
//                         state.push_event_capped(evt.clone(), def.max_events);
//                         self.threshold_event_log.push_back(evt.clone());
//                         while self.threshold_event_log.len() > self.threshold_event_log_cap {
//                             self.threshold_event_log.pop_front();
//                         }
//                         if let Some(ctrl) = &self.threshold_controller {
//                             let mut inner = ctrl.inner.lock().unwrap();
//                             inner.listeners.retain(|s| s.send(evt.clone()).is_ok());
//                         }
//                     }
//                     state.active = false;
//                     state.accum_area = 0.0;
//                 }
//                 state.last_t = Some(t);
//                 state.last_excess = e;
//                 state.prev_in_t = Some(t);
//             }
//         }
//     }

//     pub(crate) fn add_threshold_internal(&mut self, def: ThresholdDef) {
//         if self.threshold_defs.iter().any(|d| d.name == def.name) {
//             return;
//         }
//         self.threshold_states
//             .entry(def.name.clone())
//             .or_insert_with(ThresholdRuntimeState::new);
//         self.threshold_defs.push(def);
//     }

//     /// Clear all threshold events from the global log and from each threshold's runtime state.
//     pub(crate) fn clear_all_threshold_events(&mut self) {
//         self.threshold_event_log.clear();
//         for (_name, st) in self.threshold_states.iter_mut() {
//             st.events.clear();
//         }
//     }

//     /// Clear all events for a specific threshold: removes from its buffer and from the global log.
//     pub(crate) fn clear_threshold_events(&mut self, name: &str) {
//         if let Some(st) = self.threshold_states.get_mut(name) {
//             st.events.clear();
//         }
//         self.threshold_event_log.retain(|e| e.threshold != name);
//     }

//     /// Remove a specific threshold event from the global log and the corresponding threshold's buffer.
//     pub(crate) fn remove_threshold_event(&mut self, event: &ThresholdEvent) {
//         if let Some(pos) = self.threshold_event_log.iter().position(|e| {
//             e.threshold == event.threshold
//                 && e.trace == event.trace
//                 && e.start_t == event.start_t
//                 && e.end_t == event.end_t
//                 && e.duration == event.duration
//                 && e.area == event.area
//         }) {
//             self.threshold_event_log.remove(pos);
//         }
//         if let Some(st) = self.threshold_states.get_mut(&event.threshold) {
//             if let Some(pos) = st.events.iter().position(|e| {
//                 e.trace == event.trace
//                     && e.start_t == event.start_t
//                     && e.end_t == event.end_t
//                     && e.duration == event.duration
//                     && e.area == event.area
//             }) {
//                 st.events.remove(pos);
//             }
//         }
//     }

//     pub(crate) fn remove_threshold_internal(&mut self, name: &str) {
//         self.threshold_defs.retain(|d| d.name != name);
//         self.threshold_states.remove(name);
//         self.thresholds_panel.looks.remove(name);
//         self.thresholds_panel.start_looks.remove(name);
//         self.thresholds_panel.stop_looks.remove(name);
//     }

//     /// Public API: add/remove/list thresholds; get events for a threshold (clone).
//     pub fn add_threshold(&mut self, def: ThresholdDef) {
//         self.add_threshold_internal(def);
//     }
//     pub fn remove_threshold(&mut self, name: &str) {
//         self.remove_threshold_internal(name);
//     }
//     pub fn thresholds(&self) -> &[ThresholdDef] {
//         &self.threshold_defs
//     }
//     pub fn threshold_events(&self, name: &str) -> Option<Vec<ThresholdEvent>> {
//         self.threshold_states
//             .get(name)
//             .map(|s| s.events.iter().cloned().collect())
//     }
// }

// One threshold event instance.

// /// Controller to add/remove thresholds and subscribe to resulting events from outside the UI.
// #[derive(Clone)]
// pub struct ThresholdController {
//     pub(crate) inner: Arc<Mutex<ThresholdCtrlInner>>, // crate-visible for UI
// }

// pub(crate) struct ThresholdCtrlInner {
//     pub(crate) add_requests: Vec<ThresholdDef>,
//     pub(crate) remove_requests: Vec<String>,
//     pub(crate) listeners: Vec<Sender<ThresholdEvent>>,
// }

// impl ThresholdController {
//     pub fn new() -> Self {
//         Self {
//             inner: Arc::new(Mutex::new(ThresholdCtrlInner {
//                 add_requests: Vec::new(),
//                 remove_requests: Vec::new(),
//                 listeners: Vec::new(),
//             })),
//         }
//     }

//     /// Request adding a new threshold (applied by the UI thread on next frame).
//     pub fn add_threshold(&self, def: ThresholdDef) {
//         let mut inner = self.inner.lock().unwrap();
//         inner.add_requests.push(def);
//     }

//     /// Request removing a threshold by name.
//     pub fn remove_threshold<S: Into<String>>(&self, name: S) {
//         let mut inner = self.inner.lock().unwrap();
//         inner.remove_requests.push(name.into());
//     }

//     /// Subscribe to threshold events as they are recorded.
//     pub fn subscribe(&self) -> std::sync::mpsc::Receiver<ThresholdEvent> {
//         let (tx, rx) = std::sync::mpsc::channel();
//         let mut inner = self.inner.lock().unwrap();
//         inner.listeners.push(tx);
//         rx
//     }
// }
