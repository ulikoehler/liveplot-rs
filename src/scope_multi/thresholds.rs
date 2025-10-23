//! Threshold processing and event bookkeeping for `ScopeAppMulti`.
//!
//! Responsibilities:
//! - Apply add/remove requests from the `ThresholdController`
//! - Detect threshold events incrementally from incoming data
//! - Maintain per-threshold buffers and a capped global event log
//! - Provide public APIs to manage and query thresholds

use std::collections::HashMap;

use crate::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdRuntimeState};

use super::ScopeAppMulti;

impl ScopeAppMulti {
    /// Apply threshold controller add/remove requests.
    pub(super) fn apply_threshold_controller_requests(&mut self) {
        if let Some(ctrl) = &self.threshold_controller {
            let (adds, removes) = {
                let mut inner = ctrl.inner.lock().unwrap();
                let adds: Vec<ThresholdDef> = inner.add_requests.drain(..).collect();
                let removes: Vec<String> = inner.remove_requests.drain(..).collect();
                (adds, removes)
            };
            for def in adds { self.add_threshold_internal(def); }
            for name in removes { self.remove_threshold_internal(&name); }
        }
    }

    pub(super) fn process_thresholds(&mut self) {
        if self.threshold_defs.is_empty() { return; }
        let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused { if let Some(s) = &tr.snap { Box::new(s.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        for def in self.threshold_defs.clone().iter() {
            let state = self.threshold_states.entry(def.name.clone()).or_insert_with(ThresholdRuntimeState::new);
            let data = match sources.get(&def.target.0) { Some(v) => v, None => continue };
            let mut start_idx = 0usize;
            if let Some(t0) = state.prev_in_t {
                start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                    Ok(mut i) => { while i < data.len() && data[i][0] <= t0 { i += 1; } i }
                    Err(i) => i,
                };
            }
            for p in data.iter().skip(start_idx) {
                let t = p[0];
                let v = p[1];
                let e = def.kind.excess(v);
                if let Some(t0) = state.last_t {
                    let dt = (t - t0).max(0.0);
                    if state.active || e > 0.0 { state.accum_area += 0.5 * (state.last_excess + e) * dt; }
                }
                if !state.active && e > 0.0 {
                    state.active = true; state.start_t = t;
                } else if state.active && e == 0.0 {
                    let end_t = t; let dur = end_t - state.start_t;
                    if dur >= def.min_duration_s {
                        let evt = ThresholdEvent { threshold: def.name.clone(), trace: def.target.0.clone(), start_t: state.start_t, end_t, duration: dur, area: state.accum_area };
                        state.push_event_capped(evt.clone(), def.max_events);
                        self.threshold_event_log.push_back(evt.clone());
                        while self.threshold_event_log.len() > self.threshold_event_log_cap { self.threshold_event_log.pop_front(); }
                        if let Some(ctrl) = &self.threshold_controller { let mut inner = ctrl.inner.lock().unwrap(); inner.listeners.retain(|s| s.send(evt.clone()).is_ok()); }
                    }
                    state.active = false; state.accum_area = 0.0;
                }
                state.last_t = Some(t);
                state.last_excess = e;
                state.prev_in_t = Some(t);
            }
        }
    }

    pub(crate) fn add_threshold_internal(&mut self, def: ThresholdDef) {
        if self.threshold_defs.iter().any(|d| d.name == def.name) { return; }
        self.threshold_states.entry(def.name.clone()).or_insert_with(ThresholdRuntimeState::new);
        self.threshold_defs.push(def);
    }

    /// Clear all threshold events from the global log and from each threshold's runtime state.
    pub(crate) fn clear_all_threshold_events(&mut self) {
        self.threshold_event_log.clear();
        for (_name, st) in self.threshold_states.iter_mut() { st.events.clear(); }
    }

    /// Clear all events for a specific threshold: removes from its buffer and from the global log.
    pub(crate) fn clear_threshold_events(&mut self, name: &str) {
        if let Some(st) = self.threshold_states.get_mut(name) { st.events.clear(); }
        self.threshold_event_log.retain(|e| e.threshold != name);
    }

    /// Remove a specific threshold event from the global log and the corresponding threshold's buffer.
    pub(crate) fn remove_threshold_event(&mut self, event: &ThresholdEvent) {
        if let Some(pos) = self.threshold_event_log.iter().position(|e| {
            e.threshold == event.threshold && e.trace == event.trace && e.start_t == event.start_t && e.end_t == event.end_t && e.duration == event.duration && e.area == event.area
        }) { self.threshold_event_log.remove(pos); }
        if let Some(st) = self.threshold_states.get_mut(&event.threshold) {
            if let Some(pos) = st.events.iter().position(|e| {
                e.trace == event.trace && e.start_t == event.start_t && e.end_t == event.end_t && e.duration == event.duration && e.area == event.area
            }) { st.events.remove(pos); }
        }
    }

    pub(crate) fn remove_threshold_internal(&mut self, name: &str) {
        self.threshold_defs.retain(|d| d.name != name);
        self.threshold_states.remove(name);
        self.thresholds_panel.looks.remove(name);
        self.thresholds_panel.start_looks.remove(name);
        self.thresholds_panel.stop_looks.remove(name);
    }

    /// Public API: add/remove/list thresholds; get events for a threshold (clone).
    pub fn add_threshold(&mut self, def: ThresholdDef) { self.add_threshold_internal(def); }
    pub fn remove_threshold(&mut self, name: &str) { self.remove_threshold_internal(name); }
    pub fn thresholds(&self) -> &[ThresholdDef] { &self.threshold_defs }
    pub fn threshold_events(&self, name: &str) -> Option<Vec<ThresholdEvent>> {
        self.threshold_states.get(name).map(|s| s.events.iter().cloned().collect())
    }
}
