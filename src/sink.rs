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
pub enum PlotCommand {
    /// Register a new trace with a numeric ID and optional info string.
    RegisterTrace {
        id: TraceId,
        name: String,
        info: Option<String>,
    },
    /// Append a single point to the given trace ID.
    Point { trace_id: TraceId, point: PlotPoint },
    /// Append a chunk of points to the given trace ID.
    Points {
        trace_id: TraceId,
        points: Vec<PlotPoint>,
    },
    /// Set the Y value for specific points identified by their exact X coordinates.
    SetPointsY {
        trace_id: TraceId,
        xs: Vec<f64>,
        y: f64,
    },
    /// Delete specific points identified by their exact X coordinates.
    DeletePointsX { trace_id: TraceId, xs: Vec<f64> },
    /// Delete all points within the inclusive X range [x_min, x_max].
    DeleteXRange {
        trace_id: TraceId,
        x_min: f64,
        x_max: f64,
    },
    /// Apply a Y-transform function to specific points (by exact X coordinates).
    ApplyYFnAtX {
        trace_id: TraceId,
        xs: Vec<f64>,
        f: YTransform,
    },
    /// Apply a Y-transform function to all points within an inclusive X range.
    ApplyYFnInXRange {
        trace_id: TraceId,
        x_min: f64,
        x_max: f64,
        f: YTransform,
    },
    /// Remove all data points for the given trace (resulting trace is empty).
    ClearData { trace_id: TraceId },
    /// Replace the entire data vector for the given trace with the provided points.
    ///
    /// This is intended as an efficient overwrite operation: any existing points
    /// for the trace are discarded and replaced atomically with `points`.
    SetData {
        trace_id: TraceId,
        points: Vec<PlotPoint>,
    },
}

/// Convenience sender for feeding points into the multi-trace plotter.
#[derive(Clone)]
pub struct PlotSink {
    tx: Sender<PlotCommand>,
}

/// A function that transforms a point's Y value.
///
/// Note: This function will be applied in the UI thread that processes incoming plot commands.
pub type YTransform = Box<dyn Fn(f64) -> f64 + Send + 'static>;

impl PlotSink {
    /// Create and register a new `Trace` with a unique numeric ID.
    pub fn create_trace<S: Into<String>>(&self, name: S, info: Option<S>) -> Trace {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let name = name.into();
        let info_str = info.map(|s| s.into());
        // Inform the UI about the new trace
        let _ = self.tx.send(PlotCommand::RegisterTrace {
            id,
            name: name.clone(),
            info: info_str.clone(),
        });
        Trace {
            id,
            name,
            info: info_str,
        }
    }

    /// Send a single `PlotPoint` for a given `Trace`.
    pub fn send_point(
        &self,
        trace: &Trace,
        point: PlotPoint,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::Point {
            trace_id: trace.id,
            point,
        })
    }

    /// Send a single `PlotPoint` for a given trace ID.
    pub fn send_point_by_id(
        &self,
        trace_id: TraceId,
        point: PlotPoint,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::Point { trace_id, point })
    }

    /// Send a chunk of points for a given `Trace` (more efficient than point-by-point).
    pub fn send_points<I>(
        &self,
        trace: &Trace,
        points: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::Points {
            trace_id: trace.id,
            points: points.into(),
        })
    }

    /// Send a chunk of points for a given trace ID (more efficient than point-by-point).
    pub fn send_points_by_id<I>(
        &self,
        trace_id: TraceId,
        points: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::Points {
            trace_id,
            points: points.into(),
        })
    }

    /// Set the Y value for a specific point (by exact X) on a given `Trace`.
    #[inline]
    pub fn set_point_y(
        &self,
        trace: &Trace,
        x: f64,
        y: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::SetPointsY {
            trace_id: trace.id,
            xs: vec![x],
            y,
        })
    }

    /// Set the Y value for a specific point (by exact X) by trace ID.
    #[inline]
    pub fn set_point_y_by_id(
        &self,
        trace_id: TraceId,
        x: f64,
        y: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::SetPointsY {
            trace_id,
            xs: vec![x],
            y,
        })
    }

    /// Set the Y value for multiple specific points (by exact X) on a given `Trace`.
    #[inline]
    pub fn set_points_y<I>(
        &self,
        trace: &Trace,
        xs: I,
        y: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::SetPointsY {
            trace_id: trace.id,
            xs: xs.into(),
            y,
        })
    }

    /// Set the Y value for multiple specific points (by exact X) by trace ID.
    #[inline]
    pub fn set_points_y_by_id<I>(
        &self,
        trace_id: TraceId,
        xs: I,
        y: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::SetPointsY {
            trace_id,
            xs: xs.into(),
            y,
        })
    }

    /// Delete a specific point (by exact X) on a given `Trace`.
    #[inline]
    pub fn delete_point_x(
        &self,
        trace: &Trace,
        x: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::DeletePointsX {
            trace_id: trace.id,
            xs: vec![x],
        })
    }

    /// Delete a specific point (by exact X) by trace ID.
    #[inline]
    pub fn delete_point_x_by_id(
        &self,
        trace_id: TraceId,
        x: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::DeletePointsX {
            trace_id,
            xs: vec![x],
        })
    }

    /// Delete multiple specific points (by exact X) on a given `Trace`.
    #[inline]
    pub fn delete_points_x<I>(
        &self,
        trace: &Trace,
        xs: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::DeletePointsX {
            trace_id: trace.id,
            xs: xs.into(),
        })
    }

    /// Delete multiple specific points (by exact X) by trace ID.
    #[inline]
    pub fn delete_points_x_by_id<I>(
        &self,
        trace_id: TraceId,
        xs: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::DeletePointsX {
            trace_id,
            xs: xs.into(),
        })
    }

    /// Delete all points in the inclusive X range [x_min, x_max] on a given `Trace`.
    ///
    /// Special values: pass `f64::NAN` for `x_min` or `x_max` to mean the start or end of the
    /// trace's current data vector respectively (i.e. `x_min = NaN` → earliest sample, `x_max = NaN` → latest sample).
    #[inline]
    pub fn delete_x_range(
        &self,
        trace: &Trace,
        x_min: f64,
        x_max: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::DeleteXRange {
            trace_id: trace.id,
            x_min,
            x_max,
        })
    }

    /// Delete all points in the inclusive X range [x_min, x_max] by trace ID.
    #[inline]
    pub fn delete_x_range_by_id(
        &self,
        trace_id: TraceId,
        x_min: f64,
        x_max: f64,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::DeleteXRange {
            trace_id,
            x_min,
            x_max,
        })
    }

    /// Apply a Y-transform function to a specific point (by exact X) on a given `Trace`.
    #[inline]
    pub fn apply_y_fn_at_x(
        &self,
        trace: &Trace,
        x: f64,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ApplyYFnAtX {
            trace_id: trace.id,
            xs: vec![x],
            f,
        })
    }

    /// Apply a Y-transform function to a specific point (by exact X) by trace ID.
    #[inline]
    pub fn apply_y_fn_at_x_by_id(
        &self,
        trace_id: TraceId,
        x: f64,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ApplyYFnAtX {
            trace_id,
            xs: vec![x],
            f,
        })
    }

    /// Apply a Y-transform function to multiple specific points (by exact X) on a given `Trace`.
    #[inline]
    pub fn apply_y_fn_at_xs<I>(
        &self,
        trace: &Trace,
        xs: I,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::ApplyYFnAtX {
            trace_id: trace.id,
            xs: xs.into(),
            f,
        })
    }

    /// Apply a Y-transform function to multiple specific points (by exact X) by trace ID.
    #[inline]
    pub fn apply_y_fn_at_xs_by_id<I>(
        &self,
        trace_id: TraceId,
        xs: I,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<f64>>,
    {
        self.tx.send(PlotCommand::ApplyYFnAtX {
            trace_id,
            xs: xs.into(),
            f,
        })
    }

    /// Apply a Y-transform function to all points in the inclusive X range [x_min, x_max] on a given `Trace`.
    ///
    /// Special values: pass `f64::NAN` for `x_min` or `x_max` to mean the start or end of the
    /// trace's current data vector respectively (i.e. `x_min = NaN` → earliest sample, `x_max = NaN` → latest sample).
    #[inline]
    pub fn apply_y_fn_in_x_range(
        &self,
        trace: &Trace,
        x_min: f64,
        x_max: f64,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ApplyYFnInXRange {
            trace_id: trace.id,
            x_min,
            x_max,
            f,
        })
    }

    /// Apply a Y-transform function to all points in the inclusive X range [x_min, x_max] by trace ID.
    #[inline]
    pub fn apply_y_fn_in_x_range_by_id(
        &self,
        trace_id: TraceId,
        x_min: f64,
        x_max: f64,
        f: YTransform,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ApplyYFnInXRange {
            trace_id,
            x_min,
            x_max,
            f,
        })
    }

    /// Remove all data points for a given `Trace`.
    #[inline]
    pub fn clear_data(&self, trace: &Trace) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ClearData { trace_id: trace.id })
    }

    /// Remove all data points for a given trace id.
    #[inline]
    pub fn clear_data_by_id(
        &self,
        trace_id: TraceId,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>> {
        self.tx.send(PlotCommand::ClearData { trace_id })
    }

    /// Replace the entire data vector for a given `Trace` with the provided points.
    /// This discards any existing points for the trace.
    pub fn set_data<I>(
        &self,
        trace: &Trace,
        points: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::SetData {
            trace_id: trace.id,
            points: points.into(),
        })
    }

    /// Replace the entire data vector for a given trace id with the provided points.
    pub fn set_data_by_id<I>(
        &self,
        trace_id: TraceId,
        points: I,
    ) -> Result<(), std::sync::mpsc::SendError<PlotCommand>>
    where
        I: Into<Vec<PlotPoint>>,
    {
        self.tx.send(PlotCommand::SetData {
            trace_id,
            points: points.into(),
        })
    }
}

/// Create a new channel pair for plotting: `(PlotSink, Receiver<PlotCommand>)`.
pub fn channel_plot() -> (PlotSink, Receiver<PlotCommand>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (PlotSink { tx }, rx)
}
