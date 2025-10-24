//! LivePlot crate root: re-exports and module wiring.

mod point_selection;
#[cfg(feature = "fft")]
mod fft;
mod line_draw;
mod math;
mod thresholds;
mod types;
mod trace_look;
mod app;
mod ui;
mod panel;
mod export_helpers;

pub mod sink;
pub mod controllers;
pub mod config;
pub mod export;

// Public re-exports for a compact external API
#[cfg(feature = "fft")]
pub use fft::FFTWindow;
pub use config::{LivePlotConfig, XDateFormat};
pub use controllers::{FFTController, FFTPanelInfo, WindowController, WindowInfo, UiActionController, RawExportFormat, FFTRawData, FFTDataRequest};
pub use controllers::{TracesController, TracesInfo, TraceInfo};
pub use sink::{channel_plot, PlotSink, PlotPoint, PlotCommand, Trace, TraceId};
pub use app::{run_liveplot, LivePlotApp};
pub use math::{MathTraceDef, MathKind, FilterKind, TraceRef};
pub use thresholds::{ThresholdDef, ThresholdKind, ThresholdEvent, ThresholdController};

