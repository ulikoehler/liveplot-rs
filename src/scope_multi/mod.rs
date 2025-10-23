//! Multi-trace oscilloscope UI (modularized).

mod types;
mod app;
mod data;
mod ui;
mod plot;
mod math;
mod thresholds;
mod math_ui;
mod traces_ui;
mod thresholds_ui;
mod traceslook_ui;
mod fft_panel;
mod export_helpers;
mod panel;

pub use app::{ScopeAppMulti, run_liveplot};
// traceslook_ui now provides TraceLook with a render_editor() method
