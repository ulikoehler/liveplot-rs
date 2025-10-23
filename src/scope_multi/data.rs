//! Data ingestion and trace bookkeeping for the multi-trace oscilloscope.
//!
//! This module hosts the non-UI pieces of `ScopeAppMulti`:
//! - receiving samples and maintaining per-trace buffers
//! - creating traces on first sighting and assigning distinct colors
//! - pruning by time window to keep memory bounded
//! - publishing snapshots to external controllers
//! - convenience setters for fixed datasets

use egui::Color32;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::controllers::{TraceInfo, TracesController, TracesInfo};
use crate::sink::MultiSample;

use super::traceslook_ui::TraceLook;
use super::types::TraceState;
use super::ScopeAppMulti;

impl ScopeAppMulti {
    /// Directly set/replace the sample buffer for a named trace.
    ///
    /// The provided points are absolute `[t_seconds, value]` pairs. The trace is created if
    /// absent, a snapshot is taken, the app is paused, and auto-fit for both axes is requested.
    pub fn set_trace_data<S: Into<String>>(&mut self, name: S, points: Vec<[f64; 2]>) {
        let name = name.into();
        // Create trace if missing
        let is_new = !self.traces.contains_key(&name);
        let idx = self.trace_order.len();
        let entry = self.traces.entry(name.clone()).or_insert_with(|| {
            self.trace_order.push(name.clone());
            let mut look = TraceLook::default();
            look.color = Self::alloc_color(idx);
            TraceState {
                name: name.clone(),
                look,
                offset: 0.0,
                live: VecDeque::new(),
                snap: None,
                last_fft: None,
                is_math: false,
                info: String::new(),
            }
        });
        // Replace buffers
        entry.live = points.iter().copied().collect();
        entry.snap = Some(entry.live.clone());
        // Select first trace if none selected yet
        if is_new && self.selection_trace.is_none() {
            self.selection_trace = Some(name);
        }
        // Show fixed data without scrolling/pruning
        self.paused = true;
        // Request auto-fit to the provided data
        self.pending_auto_x = true;
        self.pending_auto_y = true;
    }

    /// Convenience: set/replace multiple traces at once.
    pub fn set_traces_data(&mut self, data: Vec<(String, Vec<[f64; 2]>)>) {
        for (name, pts) in data {
            self.set_trace_data(name, pts);
        }
        self.paused = true;
        self.pending_auto_x = true;
        self.pending_auto_y = true;
    }

    /// Drain incoming samples and append to per-trace buffers. Create traces on first sighting.
    pub(super) fn drain_rx_and_update_traces(&mut self) {
        while let Ok(s) = self.rx.try_recv() {
            let is_new = !self.traces.contains_key(&s.trace);
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                let mut look = TraceLook::default();
                look.color = Self::alloc_color(idx);
                TraceState {
                    name: s.trace.clone(),
                    look,
                    offset: 0.0,
                    live: VecDeque::new(),
                    snap: None,
                    last_fft: None,
                    is_math: false,
                    info: String::new(),
                }
            });
            if is_new && self.selection_trace.is_none() {
                self.selection_trace = Some(s.trace.clone());
            }
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            // Set/refresh info if provided by producer
            if let Some(info) = s.info.as_ref() {
                entry.info = info.clone();
            }
            if entry.live.len() > self.max_points {
                entry.live.pop_front();
            }
        }
    }

    /// Prune each live buffer by a margin beyond the visible window to cap memory.
    pub(super) fn prune_by_time_window(&mut self) {
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            for (_k, tr) in self.traces.iter_mut() {
                if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                    let cutoff = t_latest - self.time_window * 1.15;
                    while let Some(&[t, _]) = tr.live.front() {
                        if t < cutoff {
                            tr.live.pop_front();
                        } else {
                            break;
                        }
                    }
                }
            }
            self.last_prune = std::time::Instant::now();
        }
    }

    /// Compute latest overall time across traces respecting paused state.
    pub(super) fn latest_time_overall(&self) -> Option<f64> {
        let mut t_latest_overall = f64::NEG_INFINITY;
        for name in self.trace_order.iter() {
            if let Some(tr) = self.traces.get(name) {
                let last_t = if self.paused {
                    tr.snap.as_ref().and_then(|s| s.back()).map(|p| p[0])
                } else {
                    tr.live.back().map(|p| p[0])
                };
                if let Some(t) = last_t {
                    if t > t_latest_overall {
                        t_latest_overall = t;
                    }
                }
            }
        }
        if t_latest_overall.is_finite() { Some(t_latest_overall) } else { None }
    }

    /// Apply trace controller requests and publish snapshot to listeners.
    pub(super) fn apply_traces_controller_requests_and_publish(&mut self) {
        if let Some(ctrl) = &self.traces_controller {
            // Apply incoming requests first
            {
                let mut inner = ctrl.inner.lock().unwrap();
                for (name, rgb) in inner.color_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.look.color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    }
                }
                for (name, vis) in inner.visible_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.look.visible = vis;
                    }
                }
                for (name, off) in inner.offset_requests.drain(..) {
                    if let Some(tr) = self.traces.get_mut(&name) {
                        tr.offset = off;
                    }
                }
                if let Some(sel) = inner.selection_request.take() {
                    self.selection_trace = sel;
                }
                if let Some(unit_opt) = inner.y_unit_request.take() {
                    self.y_unit = unit_opt;
                }
                if let Some(ylog) = inner.y_log_request.take() {
                    self.y_log = ylog;
                }
            }
            // Publish snapshot
            let traces: Vec<TraceInfo> = self
                .trace_order
                .iter()
                .filter_map(|n| {
                    self.traces.get(n).map(|tr| TraceInfo {
                        name: tr.name.clone(),
                        color_rgb: [tr.look.color.r(), tr.look.color.g(), tr.look.color.b()],
                        visible: tr.look.visible,
                        is_math: tr.is_math,
                        offset: tr.offset,
                    })
                })
                .collect();
            let info = TracesInfo {
                traces,
                marker_selection: self.selection_trace.clone(),
                y_unit: self.y_unit.clone(),
                y_log: self.y_log,
            };
            let mut inner = ctrl.inner.lock().unwrap();
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }
    }

    /// Allocate a visually distinct color for a given trace index.
    pub(super) fn alloc_color(index: usize) -> Color32 {
        // Simple distinct color palette
        const PALETTE: [Color32; 10] = [
            Color32::LIGHT_BLUE,
            Color32::LIGHT_RED,
            Color32::LIGHT_GREEN,
            Color32::GOLD,
            Color32::from_rgb(0xAA, 0x55, 0xFF), // purple
            Color32::from_rgb(0xFF, 0xAA, 0x00), // orange
            Color32::from_rgb(0x00, 0xDD, 0xDD), // cyan
            Color32::from_rgb(0xDD, 0x00, 0xDD), // magenta
            Color32::from_rgb(0x66, 0xCC, 0x66), // green2
            Color32::from_rgb(0xCC, 0x66, 0x66), // red2
        ];
        PALETTE[index % PALETTE.len()]
    }
}
