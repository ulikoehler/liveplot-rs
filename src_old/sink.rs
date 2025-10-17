//! Data source types and channels for feeding samples into the plotter UIs.
//!
//! This module provides the lightweight data structures used to represent
//! time-stamped samples (single trace and multi-trace) and convenience
//! senders for pushing those samples to the UI through standard mpsc
//! channels.

use std::sync::mpsc::{Receiver, Sender};

// Single-trace API removed: use multi-trace `MultiSample`/`MultiPlotSink` instead.

/// Multi-trace input sample with an associated trace label.
///
/// For multi-trace plotting, every sample carries a `trace` name which maps to
/// a unique series (color) in the UI.
#[derive(Debug, Clone)]
pub struct MultiSample {
    /// Monotonic sample index (producer-defined).
    pub index: u64,
    /// Measurement value for this trace at the given time.
    pub value: f64,
    /// Timestamp in microseconds since UNIX epoch (UTC).
    pub timestamp_micros: i64,
    /// Name of the trace this sample belongs to. A new name creates a new
    /// series automatically.
    pub trace: String,
    /// Optional metadata string shown in UI (e.g., appended to legend if enabled).
    pub info: Option<String>,
}

/// Convenience sender for feeding `MultiSample`s into the multi-trace plotter.
#[derive(Clone)]
pub struct MultiPlotSink {
    tx: Sender<MultiSample>,
}

impl MultiPlotSink {
    /// Send a `MultiSample` to the plotter.
    pub fn send(&self, sample: MultiSample) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        self.tx.send(sample)
    }

    /// Convenience helper to send using raw fields.
    pub fn send_value<S: Into<String>>(
        &self,
        index: u64,
        value: f64,
        timestamp_micros: i64,
        trace: S,
    ) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        let s = MultiSample { index, value, timestamp_micros, trace: trace.into(), info: None };
        self.send(s)
    }

    /// Convenience helper to send using raw fields with optional info metadata.
    pub fn send_value_with_info<S: Into<String>, I: Into<String>>(
        &self,
        index: u64,
        value: f64,
        timestamp_micros: i64,
        trace: S,
        info: I,
    ) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        let s = MultiSample { index, value, timestamp_micros, trace: trace.into(), info: Some(info.into()) };
        self.send(s)
    }
}

/// Create a new channel pair for multi-trace plotting: `(MultiPlotSink, Receiver<MultiSample>)`.
pub fn channel_multi() -> (MultiPlotSink, Receiver<MultiSample>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (MultiPlotSink { tx }, rx)
}
