// ==============================================================================
// MERGE STATUS: Janosch crate - work-in-progress merge to main liveplot-rs
// ==============================================================================
//
// MERGED to main crate (src/):
//   - data/data.rs       → src/data/data.rs (LivePlotData)
//   - data/scope.rs      → src/data/scope.rs (AxisSettings, ScopeData)
//   - data/trace_look.rs → src/data/trace_look.rs (TraceLook)
//   - data/traces.rs     → src/data/traces.rs (TraceRef, TracesCollection, TraceData)
//   - panels/panel_trait.rs → src/panels/panel_trait.rs (Panel trait, PanelState)
//   - controllers.rs     → equivalent in main (TracesController, etc.)
//   - config.rs          → equivalent in main
//
// UNIQUE to Janosch (consider merging):
//   - data/triggers.rs   - Trigger system (different from thresholds)
//   - data/math.rs       - MathTrace with compute method on struct
//   - persistence.rs     - State save/load system (serde-based)
//   - app.rs             - MainPanel architecture with panel lists
//   - panels/*_ui.rs     - Panel UI implementations
//
// Main crate status: Uses LivePlotApp (monolithic) vs Janosch MainPanel (modular)
// ==============================================================================

pub mod config;
pub mod controllers;
pub mod sink; // keep existing sink API unchanged

pub mod data;
pub mod panels;

pub mod app; // standalone runner and embedding entrypoints
pub mod persistence;

// Re-exports for external API compatibility with examples
// pub use config::{LivePlotConfig, XDateFormat};
pub use controllers::{
    FftController, FftDataRequest, FftPanelInfo, FftRawData, RawExportFormat, TraceInfo,
    TracesController, TracesInfo, UiActionController, WindowController, WindowInfo,
};
pub use sink::{channel_multi, MultiPlotSink, MultiSample};
// pub use data::thresholds::{ThresholdController, ThresholdDef, ThresholdEvent, ThresholdKind};
// pub use data::math::TraceRef; // reuse TraceRef type

pub use app::{run_liveplot, run_liveplot_with_controllers, MainApp as ScopeAppMulti};
