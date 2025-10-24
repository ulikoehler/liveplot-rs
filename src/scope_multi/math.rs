//! Math trace management for `LivePlotApp`.
//!
//! Responsibilities:
//! - add/remove/update math trace definitions
//! - recompute math outputs each tick using current sources (including math-of-math)
//! - reset runtime storage for stateful operators (filters, integrators, min/max)
//! - build human-readable formula strings for legend/info

use std::collections::{HashMap, VecDeque};

use crate::math::{compute_math_trace, MathRuntimeState, MathTraceDef};

use super::traceslook_ui::TraceLook;
use super::types::TraceState;
use super::LivePlotApp;

impl LivePlotApp {
    pub(crate) fn add_math_trace_internal(&mut self, def: MathTraceDef) {
        if self.traces.contains_key(&def.name) { return; }
        let idx = self.trace_order.len();
        self.trace_order.push(def.name.clone());
        let color = Self::alloc_color(idx);
        self.traces.insert(
            def.name.clone(),
            TraceState {
                name: def.name.clone(),
                look: { let mut l = TraceLook::default(); l.color = color; l },
                offset: 0.0,
                live: VecDeque::new(),
                snap: None,
                last_fft: None,
                is_math: true,
                info: String::new(),
            },
        );
        self.math_states.entry(def.name.clone()).or_insert_with(MathRuntimeState::new);
        self.math_defs.push(def);
    }

    pub(crate) fn remove_math_trace_internal(&mut self, name: &str) {
        self.math_defs.retain(|d| d.name != name);
        self.math_states.remove(name);
        self.traces.remove(name);
        self.trace_order.retain(|n| n != name);
    }

    /// Public API: add a math trace definition (creates a new virtual trace that auto-updates).
    pub fn add_math_trace(&mut self, def: MathTraceDef) { self.add_math_trace_internal(def); }

    /// Public API: remove a previously added math trace by name.
    pub fn remove_math_trace(&mut self, name: &str) { self.remove_math_trace_internal(name); }

    /// Public API: list current math trace definitions.
    pub fn math_traces(&self) -> &[MathTraceDef] { &self.math_defs }

    pub(super) fn recompute_math_traces(&mut self) {
        if self.math_defs.is_empty() { return; }
        let mut sources: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (name, tr) in &self.traces {
            let iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused { if let Some(s) = &tr.snap { Box::new(s.iter()) } else { Box::new(tr.live.iter()) } } else { Box::new(tr.live.iter()) };
            sources.insert(name.clone(), iter.cloned().collect());
        }
        for def in &self.math_defs.clone() {
            let st = self.math_states.entry(def.name.clone()).or_insert_with(MathRuntimeState::new);
            let prev_out = sources.get(&def.name).map(|v| v.as_slice());
            let prune_cut = {
                let latest = self
                    .trace_order
                    .iter()
                    .filter_map(|n| sources.get(n).and_then(|v| v.last().map(|p| p[0])))
                    .fold(f64::NEG_INFINITY, f64::max);
                if latest.is_finite() { Some(latest - self.time_window * 1.2) } else { None }
            };
            let pts = compute_math_trace(def, &sources, prev_out, prune_cut, st);
            sources.insert(def.name.clone(), pts.clone());
            if let Some(tr) = self.traces.get_mut(&def.name) {
                tr.live = pts.iter().copied().collect();
                if self.paused { tr.snap = Some(tr.live.clone()); } else { tr.snap = None; }
                tr.info = Self::math_formula_string(def);
            } else {
                let idx = self.trace_order.len();
                self.trace_order.push(def.name.clone());
                let mut dq: VecDeque<[f64; 2]> = VecDeque::new();
                dq.extend(pts.iter().copied());
                self.traces.insert(
                    def.name.clone(),
                    TraceState {
                        name: def.name.clone(),
                        look: { let mut l = TraceLook::default(); l.color = Self::alloc_color(idx); l },
                        offset: 0.0,
                        live: dq.clone(),
                        snap: if self.paused { Some(dq.clone()) } else { None },
                        last_fft: None,
                        is_math: true,
                        info: Self::math_formula_string(def),
                    },
                );
            }
        }
    }

    /// Reset runtime storage for all math traces that maintain state (filters, integrators, min/max).
    pub(crate) fn reset_all_math_storage(&mut self) {
        for def in self.math_defs.clone().into_iter() {
            let is_stateful = matches!(
                def.kind,
                crate::math::MathKind::Integrate { .. }
                    | crate::math::MathKind::Filter { .. }
                    | crate::math::MathKind::MinMax { .. }
            );
            if is_stateful { self.reset_math_storage(&def.name); }
        }
    }

    /// Reset runtime storage for a specific math trace (clears integrator, filter states, min/max, etc.).
    pub(crate) fn reset_math_storage(&mut self, name: &str) {
        if let Some(st) = self.math_states.get_mut(name) { *st = MathRuntimeState::new(); }
        if let Some(tr) = self.traces.get_mut(name) {
            tr.live.clear();
            if let Some(s) = &mut tr.snap { s.clear(); }
        }
    }

    /// Build a human-readable formula description for a math trace.
    pub(super) fn math_formula_string(def: &MathTraceDef) -> String {
        use crate::math::{FilterKind, MathKind, MinMaxMode};
        match &def.kind {
            MathKind::Add { inputs } => {
                if inputs.is_empty() { "0".to_string() } else {
                    let mut s = String::new();
                    for (i, (r, g)) in inputs.iter().enumerate() {
                        if i > 0 { s.push_str(" + "); }
                        if (*g - 1.0).abs() < 1e-12 { s.push_str(&r.0); } else { s.push_str(&format!("{:.3}*{}", g, r.0)); }
                    }
                    s
                }
            }
            MathKind::Multiply { a, b } => format!("{} * {}", a.0, b.0),
            MathKind::Divide { a, b } => format!("{} / {}", a.0, b.0),
            MathKind::Differentiate { input } => format!("d({})/dt", input.0),
            MathKind::Integrate { input, y0 } => format!("∫ {} dt  (y0={:.3})", input.0, y0),
            MathKind::Filter { input, kind } => {
                let k = match kind {
                    FilterKind::Lowpass { cutoff_hz } => format!("LP fc={:.3} Hz", cutoff_hz),
                    FilterKind::Highpass { cutoff_hz } => format!("HP fc={:.3} Hz", cutoff_hz),
                    FilterKind::Bandpass { low_cut_hz, high_cut_hz } => format!("BP [{:.3},{:.3}] Hz", low_cut_hz, high_cut_hz),
                    FilterKind::BiquadLowpass { cutoff_hz, q } => format!("BQ-LP fc={:.3} Q={:.3}", cutoff_hz, q),
                    FilterKind::BiquadHighpass { cutoff_hz, q } => format!("BQ-HP fc={:.3} Q={:.3}", cutoff_hz, q),
                    FilterKind::BiquadBandpass { center_hz, q } => format!("BQ-BP f0={:.3} Q={:.3}", center_hz, q),
                    FilterKind::Custom { .. } => "Custom biquad".to_string(),
                };
                format!("{} -> {}", input.0, k)
            }
            MathKind::MinMax { input, decay_per_sec, mode } => {
                let mm = match mode { MinMaxMode::Min => "Min", MinMaxMode::Max => "Max" };
                match decay_per_sec { Some(d) => format!("{}({}) with decay {:.3} 1/s", mm, input.0, d), None => format!("{}({})", mm, input.0) }
            }
        }
    }

    /// Update an existing math trace definition; supports renaming if the new name is unique.
    pub fn update_math_trace(&mut self, original_name: &str, new_def: MathTraceDef) -> Result<(), &'static str> {
        if new_def.name != original_name && self.traces.contains_key(&new_def.name) { return Err("A trace with the new name already exists"); }
        if let Some(pos) = self.math_defs.iter().position(|d| d.name == original_name) { self.math_defs[pos] = new_def.clone(); } else { return Err("Original math trace not found"); }
        self.math_states.insert(new_def.name.clone(), MathRuntimeState::new());
        if new_def.name != original_name { self.math_states.remove(original_name); }
        if new_def.name != original_name {
            if let Some(mut tr) = self.traces.remove(original_name) { tr.name = new_def.name.clone(); self.traces.insert(new_def.name.clone(), tr); }
            for name in &mut self.trace_order { if name == original_name { *name = new_def.name.clone(); break; } }
            if let Some(sel) = &mut self.selection_trace { if sel == original_name { *sel = new_def.name.clone(); } }
        }
        self.recompute_math_traces();
        Ok(())
    }

    pub(crate) fn apply_add_or_edit(&mut self, def: MathTraceDef) {
        self.math_panel.error = None;
        if let Some(orig) = self.math_panel.editing.clone() {
            match self.update_math_trace(&orig, def) {
                Ok(()) => { self.math_panel.editing = None; self.math_panel.builder = super::types::MathBuilderState::default(); }
                Err(e) => { self.math_panel.error = Some(e.to_string()); }
            }
        } else {
            if self.traces.contains_key(&def.name) { self.math_panel.error = Some("A trace with this name already exists".into()); return; }
            self.add_math_trace_internal(def);
            self.math_panel.builder = super::types::MathBuilderState::default();
        }
    }
}
