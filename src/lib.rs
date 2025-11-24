//! LivePlot crate root: re-exports and module wiring.

mod app;
mod data;
mod export_helpers;
#[cfg(feature = "fft")]
mod fft;
mod hotkeys;
mod hotkeys_ui;
mod line_draw;
mod math;
mod math_ui;
mod menu_ui;
mod panel;
mod plot;
mod point_selection;
mod thresholds;
mod thresholds_ui;
#[cfg(feature = "tiles")]
pub mod tiles;
mod trace_look;
mod traces_ui;
mod types;
mod ui;

pub mod config;
pub mod controllers;
pub mod export;
pub mod sink;

#[cfg(feature = "fft")]
mod fft_panel;

// Public re-exports for a compact external API
pub use app::{run_liveplot, LivePlotApp};
pub use config::{LivePlotConfig, XDateFormat};
pub use controllers::{
    FFTController, FFTDataRequest, FFTPanelInfo, FFTRawData, RawExportFormat, UiActionController,
    WindowController, WindowInfo,
};
pub use controllers::{TraceInfo, TracesController, TracesInfo};
#[cfg(feature = "fft")]
pub use fft::FFTWindow;
pub use math::{FilterKind, MathKind, MathTraceDef, TraceRef};
pub use sink::{channel_plot, PlotCommand, PlotPoint, PlotSink, Trace, TraceId};
pub use thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdKind};
