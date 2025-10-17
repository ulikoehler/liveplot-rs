//! New modular LivePlot architecture (work-in-progress reorg)

pub mod sink; // keep existing sink API unchanged
pub mod config;
pub mod controllers;

pub mod data;
pub mod panels;

pub mod app; // standalone runner and embedding entrypoints

// Re-exports for external API compatibility with examples
pub use config::{LivePlotConfig, XDateFormat};
pub use controllers::{FftController, FftPanelInfo, WindowController, WindowInfo, UiActionController, RawExportFormat, FftRawData, FftDataRequest};
pub use controllers::{TracesController, TracesInfo, TraceInfo};
pub use sink::{channel_multi, MultiPlotSink, MultiSample};
pub use data::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdKind};
pub use data::math::TraceRef; // reuse TraceRef type

pub use app::{run_liveplot, MainApp as ScopeAppMulti};
