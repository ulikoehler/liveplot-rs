//! Threshold definitions and event detection for monitoring trace crossings.
//!
//! This module provides the data structures for defining thresholds on traces
//! and detecting when signals exceed specified conditions for a minimum duration.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::data::scope::AxisSettings;
use crate::data::trace_look::TraceLook;
use crate::data::traces::TraceRef;

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
    /// Compute excess value for the given sample.
    /// Returns 0.0 if the condition is not met.
    pub fn excess(&self, v: f64) -> f64 {
        match self {
            ThresholdKind::GreaterThan { value } => (v - *value).max(0.0),
            ThresholdKind::LessThan { value } => (*value - v).max(0.0),
            ThresholdKind::InRange { low, high } => {
                if v >= *low && v <= *high {
                    v - *low
                } else {
                    0.0
                }
            }
        }
    }

    /// Check if the condition is currently active for the given value.
    pub fn is_active(&self, v: f64) -> bool {
        self.excess(v) > 0.0
    }
}

/// Definition of a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdDef {
    /// Unique name for this threshold.
    pub name: String,
    /// Source trace to monitor.
    pub target: TraceRef,
    /// Condition to test.
    pub kind: ThresholdKind,
    /// Visual appearance for the threshold line.
    #[serde(skip)]
    pub look: TraceLook,
    /// Visual appearance for event start markers.
    #[serde(skip)]
    pub start_look: TraceLook,
    /// Visual appearance for event stop markers.
    #[serde(skip)]
    pub stop_look: TraceLook,
    /// Minimum duration (seconds) for an event to be recorded. Default 0.002 s.
    pub min_duration_s: f64,
    /// Maximum number of events to keep (oldest dropped). Default 100.
    pub max_events: usize,

    #[serde(skip)]
    pub runtime_state: ThresholdRuntimeState,
}

impl Default for ThresholdDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            target: TraceRef::default(),
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

/// A recorded threshold event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEvent {
    /// Name of the threshold that generated this event.
    pub threshold: String,
    /// The trace being monitored.
    pub trace: TraceRef,
    /// Start timestamp (seconds).
    pub start_t: f64,
    /// End timestamp (seconds).
    pub end_t: f64,
    /// Duration in seconds.
    pub duration: f64,
    /// Integrated area (excess value over time).
    pub area: f64,
}

/// Runtime state for evaluating a threshold incrementally.
#[derive(Debug, Clone, Default)]
pub struct ThresholdRuntimeState {
    active: bool,
    pub start_t: f64,
    pub last_t: Option<f64>,
    pub last_excess: f64,
    pub accum_area: f64,
    pub prev_in_t: Option<f64>,
    /// Ring buffer of recent events (cap enforced per def.max_events).
    pub events: VecDeque<ThresholdEvent>,
}

impl ThresholdRuntimeState {
    /// Push an event, enforcing a capacity cap.
    pub fn push_event_capped(&mut self, evt: ThresholdEvent, cap: usize) {
        self.events.push_back(evt);
        while self.events.len() > cap {
            self.events.pop_front();
        }
    }

    /// Reset the runtime state.
    pub fn reset(&mut self) {
        self.active = false;
        self.start_t = 0.0;
        self.last_t = None;
        self.last_excess = 0.0;
        self.accum_area = 0.0;
        self.prev_in_t = None;
        self.events.clear();
    }
}

impl ThresholdDef {
    /// Get a human-readable description of the threshold for UI/legend.
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

    /// Clear all events for this threshold.
    pub fn clear_threshold_events(&mut self) {
        self.runtime_state.events.clear();
    }

    /// Count of recorded events.
    pub fn count_threshold_events(&self) -> usize {
        self.runtime_state.events.len()
    }

    /// Get the most recent event, if any.
    pub fn get_last_threshold_event(&self) -> Option<ThresholdEvent> {
        self.runtime_state.events.back().cloned()
    }

    /// Get all recorded events.
    pub fn get_threshold_events(&self) -> Vec<ThresholdEvent> {
        self.runtime_state.events.iter().cloned().collect()
    }

    /// Access the runtime state.
    pub fn get_runtime_state(&self) -> &ThresholdRuntimeState {
        &self.runtime_state
    }

    /// Process new data points for this threshold, updating its runtime state and recording events.
    pub fn process_threshold(&mut self, sources: HashMap<TraceRef, VecDeque<[f64; 2]>>) {
        let data = match sources.get(&self.target) {
            Some(d) => d,
            None => return,
        };

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

            // Integrate area using trapezoidal rule
            if let Some(t0) = self.runtime_state.last_t {
                let dt = (t - t0).max(0.0);
                if self.runtime_state.active || e > 0.0 {
                    self.runtime_state.accum_area +=
                        0.5 * (self.runtime_state.last_excess + e) * dt;
                }
            }

            // State transitions
            if !self.runtime_state.active && e > 0.0 {
                // Start of new event
                self.runtime_state.active = true;
                self.runtime_state.start_t = t;
            } else if self.runtime_state.active && e == 0.0 {
                // End of event
                let end_t = t;
                let dur = end_t - self.runtime_state.start_t;
                if dur >= self.min_duration_s {
                    let evt = ThresholdEvent {
                        threshold: self.name.clone(),
                        trace: self.target.clone(),
                        start_t: self.runtime_state.start_t,
                        end_t,
                        duration: dur,
                        area: self.runtime_state.accum_area,
                    };
                    self.runtime_state.push_event_capped(evt, self.max_events);
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
