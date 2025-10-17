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
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

use crate::math::TraceRef;

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
                if v >= *low && v <= *high { (v - *low).max(0.0) } else { 0.0 }
            }
        }
    }

    /// Whether the condition holds at value v
    #[inline]
    pub fn is_active(&self, v: f64) -> bool { self.excess(v) > 0.0 }
}

/// Definition of a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdDef {
    /// Unique name for this threshold.
    pub name: String,
    /// Optional display name for UI/legend; falls back to `name` when None/empty.
    pub display_name: Option<String>,
    /// Source trace to monitor.
    pub target: TraceRef,
    /// Condition to test.
    pub kind: ThresholdKind,
    /// Optional color hint for rendering this threshold (RGB).
    pub color_hint: Option<[u8; 3]>,
    /// Minimum duration (seconds) for an event to be recorded. Default 0.002 s.
    pub min_duration_s: f64,
    /// Maximum number of events to keep (oldest dropped). Default 100.
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

/// One threshold event instance.
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
    pub active: bool,
    pub start_t: f64,
    pub last_t: Option<f64>,
    pub last_excess: f64,
    pub accum_area: f64,
    pub prev_in_t: Option<f64>,
    /// Ring buffer of recent events (cap enforced per def.max_events)
    pub events: VecDeque<ThresholdEvent>,
}

impl ThresholdRuntimeState {
    pub fn new() -> Self {
        Self { active: false, start_t: 0.0, last_t: None, last_excess: 0.0, accum_area: 0.0, prev_in_t: None, events: VecDeque::new() }
    }

    /// Push, enforcing a cap.
    pub fn push_event_capped(&mut self, evt: ThresholdEvent, cap: usize) {
        self.events.push_back(evt);
        while self.events.len() > cap { self.events.pop_front(); }
    }
}

/// Controller to add/remove thresholds and subscribe to resulting events from outside the UI.
#[derive(Clone)]
pub struct ThresholdController {
    pub(crate) inner: Arc<Mutex<ThresholdCtrlInner>>, // crate-visible for UI
}

pub(crate) struct ThresholdCtrlInner {
    pub(crate) add_requests: Vec<ThresholdDef>,
    pub(crate) remove_requests: Vec<String>,
    pub(crate) listeners: Vec<Sender<ThresholdEvent>>,
}

impl ThresholdController {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(ThresholdCtrlInner { add_requests: Vec::new(), remove_requests: Vec::new(), listeners: Vec::new() })) }
    }

    /// Request adding a new threshold (applied by the UI thread on next frame).
    pub fn add_threshold(&self, def: ThresholdDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.add_requests.push(def);
    }

    /// Request removing a threshold by name.
    pub fn remove_threshold<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.remove_requests.push(name.into());
    }

    /// Subscribe to threshold events as they are recorded.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<ThresholdEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        rx
    }
}
