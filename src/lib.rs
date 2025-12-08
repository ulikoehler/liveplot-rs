//! LivePlot crate root: re-exports and module wiring.

mod app;
pub mod data;
pub use data::hotkeys;
pub mod panels;
pub mod persistence;
// #[cfg(feature = "tiles")]
// pub mod tiles;

pub mod config;
pub mod controllers;
pub mod sink;

// Public re-exports for a compact external API
pub use app::run_liveplot;
pub use controllers::{
    FFTController, FFTDataRequest, FFTPanelInfo, FFTRawData, RawExportFormat, ThresholdController,
    TraceInfo, TracesController, TracesInfo, UiActionController, WindowController, WindowInfo,
};
pub use data::traces::TraceRef;
pub use panels::{Panel, PanelState};
pub use sink::{channel_plot, PlotCommand, PlotPoint, PlotSink, Trace, TraceId};
// Re-export individual panel types from panels module
pub use panels::{
    ExportPanel, HotkeysPanel, LiveplotPanel, MathPanel, MeasurementPanel,
    ScopePanel as PanelScopePanel, ThresholdsPanel, TracesPanel, TriggersPanel,
};

// Re-exports from new modules
pub use data::triggers::{Trigger, TriggerSlope};

// Convenience re-export for examples & embedded use
pub use config::LivePlotConfig;
