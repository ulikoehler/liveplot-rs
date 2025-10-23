//! Data source types and channels for feeding points into the plotter UI.
//!
//! New API (breaking change):
//! - First create a `Trace` (with name and optional info). The library assigns a numeric ID.
//! - Send `PlotPoint { x, y }` to a given trace, either singly or in chunks for efficiency.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};

/// Numeric identifier for a trace, assigned by the library when creating a `Trace`.
pub type TraceId = u32;

/// A single point on a plot: x is typically time (in seconds), y is the value.
#[derive(Debug, Clone, Copy)]
pub struct PlotPoint {
    pub x: f64,
    pub y: f64,
}

/// Declaration of a trace; returned to the caller after registration.
#[derive(Debug, Clone)]
pub struct Trace {
    pub id: TraceId,
    pub name: String,
    pub info: Option<String>,
}

/// Messages sent over the channel to drive the UI.
#[derive(Debug, Clone)]
pub enum PlotCommand {
    /// Register a new trace with a numeric ID and optional info string.
    RegisterTrace { id: TraceId, name: String, info: Option<String> },
    /// Append a single point to the given trace ID.
    Point { trace_id: TraceId, point: PlotPoint },
    /// Append a chunk of points to the given trace ID.
    Points { trace_id: TraceId, points: Vec<PlotPoint> },
}

/// Convenience sender for feeding points into the multi-trace plotter.
#[derive(Clone)]
pub struct PlotSink {
    tx: Sender<PlotCommand>,
}

impl PlotSink {
    /// Create and register a new `Trace` with a unique numeric ID.
    pub fn create_trace<S: Into<String>>(&self, name: S, info: Option<S>) -> Trace {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let name = name.into();
        let info_str = info.map(|s| s.into());
        // Inform the UI about the new trace
        let _ = self.tx.send(PlotCommand::RegisterTrace { id, name: name.clone(), info: info_str.clone() });
        Trace { id, name, info: info_str }
    }

    /// Send a single `PlotPoint` for a given `Trace`.
    pub fn send_point(&self, trace: &Trace, point: PlotPoint) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::Point { trace_id: trace.id, point })
    }

    /// Send a single `PlotPoint` for a given trace ID.
    pub fn send_point_by_id(&self, trace_id: TraceId, point: PlotPoint) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::Point { trace_id, point })
    }

    /// Send a chunk of points for a given `Trace` (more efficient than point-by-point).
    pub fn send_points<I>(&self, trace: &Trace, points: I) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::Points { trace_id: trace.id, points: points.into() })
    }

    /// Send a chunk of points for a given trace ID (more efficient than point-by-point).
    pub fn send_points_by_id<I>(&self, trace_id: TraceId, points: I) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::Points { trace_id, points: points.into() })
    }
}

/// Create a new channel pair for plotting: `(PlotSink, Receiver<PlotCommand>)`.
pub fn channel_plot() -> (PlotSink, Receiver<PlotCommand>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (PlotSink { tx }, rx)
}
