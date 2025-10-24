//! Multi-trace oscilloscope UI (modularized).

mod types;
mod app;
mod data;
mod ui;
mod hotkeys_ui;
mod plot;
mod math;
mod thresholds;
mod math_ui;
mod traces_ui;
mod thresholds_ui;
mod traceslook_ui;
pub mod hotkeys;
mod fft_panel;
mod export_helpers;
mod panel;

pub use app::{LivePlotApp, run_liveplot};
// traceslook_ui now provides TraceLook with a render_editor() method
