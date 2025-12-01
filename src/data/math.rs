//! Math traces: virtual signals derived from existing traces (oscilloscope-like "Math").
//!
//! This module exposes the data structures used to describe math traces (serializable via
//! serde) and a computation engine that derives new time-series from existing input traces.
//!
//! Design goals and behavior summary:
//! - Math traces are virtual and appear like regular traces in the UI. They can be
//!   recomputed on-demand (for stateless operations) or updated incrementally (for
//!   stateful operations like IIR filters, integrators and min/max trackers).
//! - Input traces are represented as time/value pairs with strictly non-decreasing
//!   timestamps. When combining multiple inputs we evaluate the result on the union of
//!   timestamps, using a last-sample-hold behaviour for channels that don't have a value
//!   exactly at a given timestamp.
//! - Stateful operations keep their state in `MathRuntimeState` and are updated only for
//!   the newly appended input samples to avoid reprocessing the full buffer on every UI
//!   refresh. Stateless operations (Add/Multiply/Divide/Differentiate) recompute fully on
//!   the union grid.
//!
//! Numerical notes and conventions:
//! - Small epsilons (1e-9 .. 1e-15 depending on context) are used to avoid division by
//!   zero or degenerate comparator behavior for floating point timestamps.
//! - The biquad and first-order filters are implemented in Direct Form I and normalized
//!   by a0 when required. The RBJ cookbook formulas are used for the biquad sections.
//!
//! See individual functions and types for more detailed documentation and per-line notes.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Import filter helper functions and types from main crate's math module
use crate::math::{
    biquad_bandpass, biquad_highpass, biquad_lowpass, biquad_step, first_order_highpass,
    first_order_lowpass, BiquadParams,
};

/// Identifier of a source trace by name.
///
/// This is just a thin wrapper around `String` used to make math trace definitions
/// more explicit. The inner string must match a key present in the `sources` map
/// passed to computation routines.
use crate::data::traces::TraceRef;

// Note: BiquadParams is now imported from crate::math to avoid duplicate type definitions.

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MinMaxMode {
    Min,
    Max,
}

/// Filter kind presets and custom option.
///
/// Presets are higher-level descriptions (cutoff frequency, Q, etc.) that are
/// translated to `BiquadParams` at runtime using the current sampling interval
/// (dt). This allows the same semantic filter (e.g. lowpass at 5 Hz) to work on
/// variable-rate input streams by recomputing coefficients per-sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterKind {
    /// First-order lowpass with cutoff Hz
    Lowpass { cutoff_hz: f64 },
    /// First-order highpass with cutoff Hz
    Highpass { cutoff_hz: f64 },
    /// Simple bandpass using cascaded 1st order HP and LP
    Bandpass { low_cut_hz: f64, high_cut_hz: f64 },
    /// Biquad lowpass with cutoff and Q
    BiquadLowpass { cutoff_hz: f64, q: f64 },
    /// Biquad highpass with cutoff and Q
    BiquadHighpass { cutoff_hz: f64, q: f64 },
    /// Biquad bandpass (constant skirt gain, peak gain = Q)
    BiquadBandpass { center_hz: f64, q: f64 },
    /// Raw custom biquad coefficients (advanced)
    Custom { params: BiquadParams },
}

/// Mathematical operation that defines how a math trace is computed from inputs.
///
/// Each variant describes a different computation. Note which kinds are
/// stateless and can be fully recomputed on the union grid (Add, Multiply,
/// Divide, Differentiate) versus which require persistent runtime state and
/// incremental processing (Integrate, Filter, MinMax).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MathKind {
    /// Sum or difference of N traces: sum_i (sign_i * x_i)
    Add { inputs: Vec<(TraceRef, f64)> },
    /// Multiply two traces
    Multiply { a: TraceRef, b: TraceRef },
    /// Divide two traces (a/b)
    Divide { a: TraceRef, b: TraceRef },
    /// Numerical derivative of one trace (dy/dt)
    Differentiate { input: TraceRef },
    /// Numerical integral of one trace (∫ y dt), optional initial value
    Integrate { input: TraceRef, y0: f64 },
    /// IIR filter on one trace
    Filter { input: TraceRef, kind: FilterKind },
    /// Track min/max with optional exponential decay (per second)
    /// Track min or max with optional exponential decay (per second)
    MinMax {
        input: TraceRef,
        decay_per_sec: Option<f64>,
        mode: MinMaxMode,
    },
}

/// Fully-defined math trace configuration.
///
/// This is the serializable description exposed to UI and persisted state. It
/// contains a human-facing name, an optional color hint and the `MathKind`
/// which specifies the actual computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathTrace {
    pub name: TraceRef,
    pub kind: MathKind,
    // #[serde(skip)]
    // runtime_state: MathRuntimeState,
}

/// Runtime state for stateful math traces.
///
/// Integrators, IIR filters and min/max trackers need to persist a small amount
/// of state between successive recomputations so that they can be updated
/// incrementally when new samples arrive. This struct holds that state and is
/// stored per-math-trace in `LivePlotApp::math_states`.
// #[derive(Debug, Clone)]
// struct MathRuntimeState {
//     /// Timestamp of the last processed input sample (or None if no samples yet).
//     pub last_t: Option<f64>,
//     /// Accumulator for the integrator (running integral value).
//     //pub accum: f64,
//     // For biquad: previous two input samples x[n-1], x[n-2] and previous two
//     // output samples y[n-1], y[n-2]. These are used to implement Direct Form I.
//     pub x1: f64,
//     pub x2: f64,
//     pub y1: f64,
//     pub y2: f64,
//     // Secondary section for cascade filters (used by Bandpass implementation).
//     pub x1b: f64,
//     pub x2b: f64,
//     pub y1b: f64,
//     pub y2b: f64,
//     // For MinMax tracker: running min and max. Initialized to infinities so the
//     // first real sample sets them properly.
//     // pub min_val: f64,
//     // pub max_val: f64,
//     // /// Timestamp where decay was last applied for the min/max exponential decay.
//     // pub last_decay_t: Option<f64>,
//     // Previous input sample used for incremental algorithms like integrate.
//     pub prev_in_t: Option<f64>,
//     pub prev_in_v: f64,
// }

// impl Default for MathRuntimeState {
//     fn default() -> Self {
//         Self {
//             last_t: None,
//             //accum: 0.0,
//             x1: 0.0,
//             x2: 0.0,
//             y1: 0.0,
//             y2: 0.0,
//             x1b: 0.0,
//             x2b: 0.0,
//             y1b: 0.0,
//             y2b: 0.0,
//             // min_val: f64::INFINITY,
//             // max_val: f64::NEG_INFINITY,
//             // last_decay_t: None,
//             prev_in_t: None,
//             prev_in_v: 0.0,
//         }
//     }
// }

/// Compute a math trace given source traces. Each source trace is provided as a slice of
/// monotonically increasing [t, y]. The result is densely sampled at the union of timestamps
/// across inputs, using last-sample hold for absent channels at a time.
/// Compute a math trace from the provided `sources`.
///
/// Arguments:
/// - `def`: math trace definition describing name and operation.
/// - `sources`: mapping from trace name to a slice of `[t, y]` pairs. Timestamps must
///   be monotone non-decreasing per trace.
/// - `prev_output`: optional reference to previously computed output points for this
///   math trace; used to keep previously computed values when only appending new
///   samples for stateful operations.
/// - `prune_before`: optional timestamp cutoff; output points strictly earlier than
///   this value should be discarded. This is used to cap memory usage when the
///   display window slides forward.
/// - `state`: mutable runtime state for stateful math kinds (filters, integrators,
///   min/max). The function will update this state to reflect processed inputs.
///
/// Behavior summary:
/// - Stateless operations (Add/Multiply/Divide/Differentiate) are computed on the
///   union of timestamps from relevant inputs and recomputed fully each call.
/// - Stateful operations (Filter/Integrate/MinMax) will attempt to process only new
///   samples since `state.prev_in_t` to avoid reprocessing older data. To force a
///   complete reset, call `MathRuntimeState::new()` for the trace and clear the
///   associated output buffer.
///
impl MathTrace {
    pub fn new(name: TraceRef, kind: MathKind) -> Self {
        Self {
            name,
            kind,
            // runtime_state: MathRuntimeState::default(),
        }
    }

    pub fn compute_math_trace(
        &mut self,
        sources: HashMap<TraceRef, Vec<[f64; 2]>>,
    ) -> Vec<[f64; 2]> {
        let mut out = if let Some(points) = sources.get(&self.name) {
            points.clone()
        } else {
            Vec::new()
        };

        match &self.kind {
            MathKind::Add { inputs } => {
                // Build union grid across inputs
                // Build the union of timestamps across all referenced inputs. Sorting
                // and deduping produces a deterministic evaluation grid. We use a
                // small tolerance when deduping to account for floating-point
                // representations of equal timestamps.

                let used_sources: Vec<Vec<[f64; 2]>> = inputs
                    .iter()
                    .filter_map(|(r, _k)| sources.get(r).cloned())
                    .collect();

                let grid: Vec<f64> = MathTrace::union_times(used_sources.clone());

                let mut idx_map: std::collections::HashMap<TraceRef, usize> = HashMap::new();
                for t in grid {
                    if out.last().map_or(false, |p| p[0] >= t) {
                        continue;
                    }
                    let mut sum = 0.0;
                    let mut all = true;
                    for (r, k) in inputs {
                        if let Some(src_data) = sources.get(r) {
                            let last_idx = if let Some(idx) = idx_map.get_mut(r) {
                                idx
                            } else {
                                idx_map.insert(r.clone(), 0);
                                idx_map.get_mut(r).unwrap()
                            };
                            //println!("Add: t = {}, last_idx for '{}' = {}", t, r.0, *last_idx);
                            if let Some(v) =
                                MathTrace::interpolate_value_at(t, src_data.clone(), last_idx)
                            {
                                sum += k * v;
                            } else {
                                all = false;
                                break;
                            }
                        }
                    }
                    if all {
                        out.push([t, sum]);
                    }
                }
            }
            MathKind::Multiply { a, b } => {
                // Multiply: similar union-grid evaluation as Add, but only produce
                // a result when both operands have a defined last-sample value at
                // the time t. Note that if one trace doesn't exist in `sources` we
                // return an empty output (handled earlier by the data lookup).

                if let (Some(src_a), Some(src_b)) = (sources.get(a), sources.get(b)) {
                    let grid: Vec<f64> = MathTrace::union_times(vec![src_a.clone(), src_b.clone()]);

                    let mut idx_a = 0usize;
                    let mut idx_b = 0usize;
                    for &t in &grid {
                        if out.last().map_or(false, |p| p[0] >= t) {
                            continue;
                        }
                        if let (Some(va), Some(vb)) = (
                            MathTrace::interpolate_value_at(t, src_a.clone(), &mut idx_a),
                            MathTrace::interpolate_value_at(t, src_b.clone(), &mut idx_b),
                        ) {
                            out.push([t, va * vb]);
                        }
                    }
                } else {
                    return out;
                }
            }
            MathKind::Divide { a, b } => {
                // Divide: same union-grid approach but guard against tiny
                // denominators. We treat |b| < 1e-12 as effectively zero and skip
                // that sample to avoid large spurious results. This threshold is a
                // pragmatic choice balancing numerical stability and dynamic range.
                if let (Some(src_a), Some(src_b)) = (sources.get(a), sources.get(b)) {
                    let grid: Vec<f64> = MathTrace::union_times(vec![src_a.clone(), src_b.clone()]);

                    let mut idx_a = 0usize;
                    let mut idx_b = 0usize;
                    for &t in &grid {
                        if out.last().map_or(false, |p| p[0] >= t) {
                            continue;
                        }
                        if let (Some(va), Some(vb)) = (
                            MathTrace::interpolate_value_at(t, src_a.clone(), &mut idx_a),
                            MathTrace::interpolate_value_at(t, src_b.clone(), &mut idx_b),
                        ) {
                            if vb.abs() > 1e-12 {
                                out.push([t, va / vb]);
                            }
                        }
                    }
                } else {
                    return out;
                }
            }
            MathKind::Differentiate { input } => {
                // Numerical differentiation implemented using two-point forward
                // difference using successive samples: dy/dt ~ (v1 - v0)/(t1 - t0).
                // We skip the very first sample since no previous point exists. If
                // timestamps are equal or dt <= 0 the sample is skipped to avoid
                // division by zero.
                if let Some(src) = sources.get(input) {
                    let mut prev: Option<(f64, f64)> = None;
                    for &p in src.iter() {
                        let t = p[0];
                        let v = p[1];

                        if out.last().map_or(false, |p| p[0] >= t) {
                            prev = Some((t, v));
                            continue;
                        }
                        if let Some((t0, v0)) = prev {
                            let dt = t - t0;
                            if dt > 0.0 {
                                out.push([t, (v - v0) / dt]);
                            }
                        }
                        prev = Some((t, v));
                    }
                } else {
                    return out;
                }

                // let data = match sources.get(&input.0) {
                //     Some(v) => v,
                //     None => return out,
                // };
                // let mut prev: Option<(f64, f64)> = None;
                // for &p in data.iter() {
                //     let t = p[0];
                //     let v = p[1];

                //     if out.last().map_or(false, |p| p[0] >= t) {
                //         continue;
                //     }
                //     // If we're asked to prune old samples, we still advance the
                //     // `prev` pointer so the next kept sample will be differentiated
                //     // against the most recent pruned point.
                //     if let Some((t0, v0)) = prev {
                //         let dt = t - t0;
                //         if dt > 0.0 {
                //             out.push([t, (v - v0) / dt]);
                //         }
                //     }
                //     prev = Some((t, v));
                // }
            }
            MathKind::Integrate { input, y0 } => {
                // Numerical integration using the trapezoidal rule. The integrator is
                // stateful: `state.accum` holds the running integral and
                // `state.prev_in_t`/`state.prev_in_v` remember the last processed
                // input sample. This allows us to append only newly arrived samples
                // without touching older results.
                if let Some(src) = sources.get(input) {
                    let mut prev: Option<(f64, f64)> = None;
                    let mut accum = if out.is_empty() {
                        *y0
                    } else {
                        out.last().unwrap()[1]
                    };
                    for &p in src.iter() {
                        let t = p[0];
                        let v = p[1];

                        if out.last().map_or(false, |p| p[0] >= t) {
                            prev = Some((t, v));
                            continue;
                        }
                        if let Some((t0, v0)) = prev {
                            let dt = t - t0;
                            if dt > 0.0 {
                                accum += 0.5 * (v + v0) * dt;
                                out.push([t, accum.clone()]);
                            }
                        } else if out.is_empty() {
                            // First sample, initialize output
                            out.push([t, accum.clone()]);
                        }
                        prev = Some((t, v));
                    }
                } else {
                    return out;
                }

                // let data = match sources.get(&input.0) {
                //     Some(v) => v,
                //     None => return out,
                // };

                // // If we have never processed this integrator before, initialize the
                // // accumulator with the provided y0. Otherwise keep the stored value.
                // let mut accum = if self.runtime_state.prev_in_t.is_none() {
                //     *y0
                // } else {
                //     self.runtime_state.accum
                // };
                // // Start `prev_t`/`prev_v` from stored state so we can integrate from
                // // the last processed point.
                // let mut prev_t = self.runtime_state.prev_in_t;
                // let mut prev_v = if self.runtime_state.prev_in_t.is_none() {
                //     None
                // } else {
                //     Some(self.runtime_state.prev_in_v)
                // };

                // // Compute the index from which we need to process new samples. If
                // // self.runtime_state.prev_in_t exists, find the first sample strictly after it.
                // let mut start_idx = 0usize;
                // if let Some(t0) = self.runtime_state.prev_in_t {
                //     start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                //         Ok(mut i) => {
                //             // advance past all samples <= t0
                //             while i < data.len() && data[i][0] <= t0 {
                //                 i += 1;
                //             }
                //             i
                //         }
                //         Err(i) => i,
                //     };
                // }

                // // Process new samples using the trapezoid rule and append outputs.
                // for p in data.iter().skip(start_idx) {
                //     let t = p[0];
                //     let v = p[1];
                //     if let (Some(t0), Some(v0)) = (prev_t, prev_v) {
                //         let dt = t - t0;
                //         if dt > 0.0 {
                //             // Trapezoidal increment: 0.5*(v + v0) * dt
                //             accum += 0.5 * (v + v0) * dt;
                //         }
                //     }
                //     prev_t = Some(t);
                //     prev_v = Some(v);
                //     out.push([t, accum]);
                // }

                // // Update persistent state so subsequent calls continue from here.
                // self.runtime_state.accum = accum;
                // self.runtime_state.last_t = prev_t;
                // self.runtime_state.prev_in_t = prev_t;
                // self.runtime_state.prev_in_v = prev_v.unwrap_or(self.runtime_state.prev_in_v);
            }
            MathKind::Filter { input, kind } => {
                // IIR filter processing. We treat several FilterKind variants by
                // converting them to `BiquadParams` on a per-sample basis because
                // the sample interval `dt` may vary between successive samples.
                //
                // Stateless variant: derive initial delay-line state from the
                // previously computed output `out` and the source input. This
                // avoids storing persistent state in `MathRuntimeState` while
                // still allowing incremental append-only processing. If no prior
                // output exists, we start from zero initial conditions.
                let data: &Vec<[f64; 2]> = match sources.get(input) {
                    Some(v) => v,
                    None => return out,
                };
                // Reconstruct delay elements (x1,x2,y1,y2) and last processed time
                // from existing output `out` if available.
                let mut x1: f64 = 0.0;
                let mut x2: f64 = 0.0;
                let mut y1: f64 = 0.0;
                let mut y2: f64 = 0.0;
                let mut x1b: f64 = 0.0;
                let mut x2b: f64 = 0.0;
                let mut y1b: f64 = 0.0;
                let mut y2b: f64 = 0.0;
                let mut last_t: Option<f64> = None;
                let mut start_idx: usize = 0;

                if !out.is_empty() {
                    // Helper to find index of exact timestamp (or next greater) in `data`.
                    let find_idx = |t: f64, d: &Vec<[f64; 2]>| -> Option<usize> {
                        match d.binary_search_by(|p| p[0].partial_cmp(&t).unwrap()) {
                            Ok(i) => Some(i),
                            Err(i) => {
                                // If timestamp wasn't found exactly, try to align to previous if equal within epsilon
                                if i > 0 && (d[i - 1][0] - t).abs() < 1e-12 {
                                    Some(i - 1)
                                } else {
                                    None
                                }
                            }
                        }
                    };

                    // Use last output sample as y1 at time t1
                    let (t1, y1v) = {
                        let p = out.last().copied().unwrap();
                        (p[0], p[1])
                    };
                    if let Some(i1) = find_idx(t1, data) {
                        y1 = y1v;
                        x1 = data[i1][1];
                        last_t = Some(t1);
                        start_idx = i1.saturating_add(1);
                    }

                    // If we have at least two outputs, set y2/x2 from the previous one
                    if out.len() >= 2 {
                        let (t2, y2v) = {
                            let p = out[out.len() - 2];
                            (p[0], p[1])
                        };
                        if let Some(i2) = find_idx(t2, data) {
                            y2 = y2v;
                            x2 = data[i2][1];
                            // Ensure start index is beyond both recovered points
                            start_idx = start_idx.max(i2.saturating_add(1));
                        }
                    }
                }

                for p in data.iter().skip(start_idx) {
                    let t = p[0];
                    let x = p[1];

                    // Derive dt from last processed timestamp; if none exists use
                    // a small default dt to avoid divide-by-zero. We clamp dt to a
                    // minimum of 1e-9 to avoid super-large computed coefficients on
                    // nearly-zero intervals.
                    let dt = if let Some(t0) = last_t {
                        (t - t0).max(1e-9)
                    } else {
                        // Reasonable small dt for the first sample; exact value
                        // isn't critical because the first output is primarily
                        // driven by initial conditions.
                        1e-3
                    };

                    // Compute filter coefficients for the current dt and run one
                    // step of the direct-form I biquad.
                    let y = match kind {
                        FilterKind::Lowpass { cutoff_hz } => {
                            let p = first_order_lowpass(*cutoff_hz, dt);
                            biquad_step(p, x, x1, x2, y1, y2)
                        }
                        FilterKind::Highpass { cutoff_hz } => {
                            let p = first_order_highpass(*cutoff_hz, dt);
                            biquad_step(p, x, x1, x2, y1, y2)
                        }
                        FilterKind::Bandpass {
                            low_cut_hz,
                            high_cut_hz,
                        } => {
                            // Implement bandpass as cascade: highpass -> lowpass.
                            let p1 = first_order_highpass(*low_cut_hz, dt);
                            let z1 = biquad_step(p1, x, x1, x2, y1, y2);
                            let p2 = first_order_lowpass(*high_cut_hz, dt);
                            biquad_step(p2, z1, x1b, x2b, y1b, y2b)
                        }
                        FilterKind::BiquadLowpass { cutoff_hz, q } => {
                            let p = biquad_lowpass(*cutoff_hz, *q, dt);
                            biquad_step(p, x, x1, x2, y1, y2)
                        }
                        FilterKind::BiquadHighpass { cutoff_hz, q } => {
                            let p = biquad_highpass(*cutoff_hz, *q, dt);
                            biquad_step(p, x, x1, x2, y1, y2)
                        }
                        FilterKind::BiquadBandpass { center_hz, q } => {
                            let p = biquad_bandpass(*center_hz, *q, dt);
                            biquad_step(p, x, x1, x2, y1, y2)
                        }
                        FilterKind::Custom { params } => {
                            biquad_step(*params, x, x1, x2, y1, y2)
                        }
                    };

                    // Advance delay-line state. Bandpass uses a cascade so we must
                    // update both primary and secondary sections using the
                    // intermediate z1 value computed above.
                    match kind {
                        FilterKind::Bandpass { .. } => {
                            // Recompute the first section step to obtain the z1 used
                            // as input to the second section when updating the
                            // internal delay elements.
                            let p1 = if let FilterKind::Bandpass { low_cut_hz, .. } = kind {
                                first_order_highpass(*low_cut_hz, dt)
                            } else {
                                first_order_highpass(1.0, dt)
                            };
                            let z1 = biquad_step(p1, x, x1, x2, y1, y2);
                            x2 = x1;
                            x1 = x;
                            y2 = y1;
                            y1 = z1;
                            x2b = x1b;
                            x1b = z1;
                            y2b = y1b;
                            y1b = y;
                        }
                        _ => {
                            x2 = x1;
                            x1 = x;
                            y2 = y1;
                            y1 = y;
                        }
                    }

                    last_t = Some(t);
                    out.push([t, y]);
                }
                // No persistent state updates: this variant reconstructs state each call.
            }
            MathKind::MinMax {
                input,
                decay_per_sec,
                mode,
            } => {
                if let Some(src) = sources.get(input) {
                    let mut prev: Option<(f64, f64)> = None;
                    let mut minmax = if out.is_empty() {
                        src.first().map(|p| p[1]).unwrap_or(0.0)
                    } else {
                        out.last().unwrap()[1]
                    };

                    for &p in src.iter() {
                        let t = p[0];
                        let v = p[1];

                        if out.last().map_or(false, |p| p[0] >= t) {
                            prev = Some((t, v));
                            continue;
                        }
                        minmax = match mode {
                            MinMaxMode::Min => minmax.min(v),
                            MinMaxMode::Max => minmax.max(v),
                        };

                        if let (Some((t0, _v0)), Some(decay)) = (prev, decay_per_sec) {
                            let dt = t - t0;
                            if dt > 0.0 {
                                let k = (-decay * dt).exp();
                                minmax = match mode {
                                    MinMaxMode::Min => minmax * k + v * (1.0 - k),
                                    MinMaxMode::Max => minmax * k + v * (1.0 - k),
                                };
                            }
                        }

                        out.push([t, minmax.clone()]);
                        prev = Some((t, v));
                    }
                } else {
                    return out;
                }

                // let data = match sources.get(&input.0) {
                //     Some(v) => v,
                //     None => return out,
                // };
                // let mut min_v = self.runtime_state.min_val;
                // let mut max_v = self.runtime_state.max_val;
                // let mut last_decay_t = self.runtime_state.last_decay_t;
                // let mut start_idx = 0usize;
                // if let Some(t0) = self.runtime_state.prev_in_t {
                //     start_idx = match data.binary_search_by(|p| p[0].partial_cmp(&t0).unwrap()) {
                //         Ok(mut i) => {
                //             while i < data.len() && data[i][0] <= t0 {
                //                 i += 1;
                //             }
                //             i
                //         }
                //         Err(i) => i,
                //     };
                // }

                // for p in data.iter().skip(start_idx) {
                //     let t = p[0];
                //     let v = p[1];

                //     // Apply exponential decay to previous min/max between the
                //     // stored last_decay_t and the current timestamp. The decay
                //     // factor k = exp(-decay * dt) multiplicatively reduces the
                //     // influence of the historic extremum.
                //     if let Some(decay) = decay_per_sec {
                //         if let Some(t0) = last_decay_t {
                //             let dt = (t - t0).max(0.0);
                //             if dt > 0.0 {
                //                 let k = (-decay * dt).exp();
                //                 min_v = min_v.min(v) * k + v * (1.0 - k);
                //                 max_v = max_v.max(v) * k + v * (1.0 - k);
                //             }
                //         }
                //     }

                //     // If we have infinities (initial state) set them from the
                //     // current value to bootstrap the running min/max.
                //     if min_v.is_infinite() {
                //         min_v = v;
                //     }
                //     if max_v.is_infinite() {
                //         max_v = v;
                //     }
                //     min_v = min_v.min(v);
                //     max_v = max_v.max(v);
                //     last_decay_t = Some(t);
                //     let y = match mode {
                //         MinMaxMode::Min => min_v,
                //         MinMaxMode::Max => max_v,
                //     };
                //     out.push([t, y]);
                // }

                // // Persist state after processing.
                // self.runtime_state.min_val = min_v;
                // self.runtime_state.max_val = max_v;
                // self.runtime_state.last_decay_t = last_decay_t;
                // self.runtime_state.prev_in_t = data.last().map(|p| p[0]);
                // self.runtime_state.prev_in_v = data
                //     .last()
                //     .map(|p| p[1])
                //     .unwrap_or(self.runtime_state.prev_in_v);
            }
        }

        // let paused = data.paused;
        // let tr = data.get_trace_or_new(self.name.as_str());
        // if paused {
        //     tr.snap = Some(out.iter().copied().collect());
        // } else {
        //     tr.live = out.iter().copied().collect();
        // }
        out
    }

    fn union_times<'a>(sources: Vec<Vec<[f64; 2]>>) -> Vec<f64> {
        let mut v = Vec::new();
        for s in sources {
            v.extend(s.iter().map(|p| p[0]));
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Consider timestamps equal if they differ by < 1e-15.
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-15);
        v
    }

    fn interpolate_value_at(t: f64, data: Vec<[f64; 2]>, last_idx: &mut usize) -> Option<f64> {
        if data.is_empty() {
            return None;
        }
        // Return None if out of range
        let n = data.len();
        if n == 0 {
            return None;
        }
        let first_t = data[0][0];
        let last_t = data[n - 1][0];
        if t < first_t || t > last_t {
            return None;
        }

        // Clamp last_idx into valid range
        if *last_idx >= n {
            *last_idx = n - 1;
        }

        // If our cached index is ahead of the requested time, use binary search
        // to find the first index j where time[j] >= t, then interpolate with j-1.
        if data[*last_idx][0] > t {
            // upper_bound: first j such that data[j][0] >= t
            let mut lo = 0usize;
            let mut hi = n; // exclusive
            while lo < hi {
                let mid = (lo + hi) / 2;
                let tm = data[mid][0];
                if tm < t {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            let j = lo;
            if j == 0 {
                // t <= first_t and we already handled t < first_t; so t == first_t here
                return Some(data[0][1]);
            }
            if j == n {
                // t >= last_t and we already handled t > last_t; so t == last_t here
                return Some(data[n - 1][1]);
            }
            let i = j - 1;
            *last_idx = i;
            let t0 = data[i][0];
            let v0 = data[i][1];
            let t1 = data[j][0];
            let v1 = data[j][1];
            if t1 == t0 {
                return Some(v0);
            }
            let alpha = (t - t0) / (t1 - t0);
            return Some(v0 + alpha * (v1 - v0));
        }

        // Move forward while the next sample time is still before t
        while *last_idx + 1 < n && data[*last_idx + 1][0] < t {
            *last_idx += 1;
        }

        // Exact match at current or next index
        if data[*last_idx][0] == t {
            return Some(data[*last_idx][1]);
        }
        if *last_idx + 1 < n && data[*last_idx + 1][0] == t {
            *last_idx += 1;
            return Some(data[*last_idx][1]);
        }

        // Interpolate between last_idx and last_idx + 1 if possible
        if *last_idx + 1 < n {
            let t0 = data[*last_idx][0];
            let v0 = data[*last_idx][1];
            let t1 = data[*last_idx + 1][0];
            let v1 = data[*last_idx + 1][1];
            if t1 == t0 {
                return Some(v0);
            }
            let alpha = (t - t0) / (t1 - t0);
            Some(v0 + alpha * (v1 - v0))
        } else {
            // No next point; since t <= last_t and not equal, return None
            None
        }
    }

    // NOTE: The following functions were removed as duplicates of main crate src/math.rs:
    // - first_order_lowpass() -> see src/math.rs line ~699
    // - first_order_highpass() -> see src/math.rs line ~718
    // - biquad_step() -> see src/math.rs line ~728
    // - biquad_lowpass() -> see src/math.rs line ~751
    // - biquad_highpass() -> see src/math.rs line ~778
    // - biquad_bandpass() -> see src/math.rs line ~804

    /// Build a human-readable formula description for a math trace.
    pub fn math_formula_string(&self) -> String {
        match &self.kind {
            MathKind::Add { inputs } => {
                if inputs.is_empty() {
                    "0".to_string()
                } else {
                    let mut s = String::new();
                    for (i, (r, g)) in inputs.iter().enumerate() {
                        if i > 0 {
                            s.push_str(" + ");
                        }
                        if (*g - 1.0).abs() < 1e-12 {
                            s.push_str(&r.0);
                        } else {
                            s.push_str(&format!("{:.3}*{}", g, r.0));
                        }
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
                    FilterKind::Bandpass {
                        low_cut_hz,
                        high_cut_hz,
                    } => format!("BP [{:.3},{:.3}] Hz", low_cut_hz, high_cut_hz),
                    FilterKind::BiquadLowpass { cutoff_hz, q } => {
                        format!("BQ-LP fc={:.3} Q={:.3}", cutoff_hz, q)
                    }
                    FilterKind::BiquadHighpass { cutoff_hz, q } => {
                        format!("BQ-HP fc={:.3} Q={:.3}", cutoff_hz, q)
                    }
                    FilterKind::BiquadBandpass { center_hz, q } => {
                        format!("BQ-BP f0={:.3} Q={:.3}", center_hz, q)
                    }
                    FilterKind::Custom { .. } => "Custom biquad".to_string(),
                };
                format!("{} -> {}", input.0, k)
            }
            MathKind::MinMax {
                input,
                decay_per_sec,
                mode,
            } => {
                let mm = match mode {
                    MinMaxMode::Min => "Min",
                    MinMaxMode::Max => "Max",
                };
                match decay_per_sec {
                    Some(d) => format!("{}({}) with decay {:.3} 1/s", mm, input.0, d),
                    None => format!("{}({})", mm, input.0),
                }
            }
        }
    }
}
