//! LivePlot crate root: re-exports and module wiring.

mod point_selection;
#[cfg(feature = "fft")]
mod fft;
mod line_draw;
mod math;
mod thresholds;
mod types;
mod data;
mod hotkeys_ui;
mod trace_look;
mod app;
mod plot;
mod menu_ui;
mod math_ui;
mod thresholds_ui;
mod traces_ui;
mod hotkeys;
mod ui;
mod panel;
mod export_helpers;

pub mod sink;
pub mod controllers;
pub mod config;
pub mod export;

#[cfg(feature = "fft")]
mod fft_panel;

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

