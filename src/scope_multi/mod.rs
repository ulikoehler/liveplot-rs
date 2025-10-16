//! Multi-trace oscilloscope UI (modularized).

mod types;
mod app;
mod math_ui;
mod traces_ui;
mod thresholds_ui;
mod traceslook_ui;
mod fft_panel;
mod export_helpers;
mod panel;

pub use app::{ScopeAppMulti, run_liveplot};
// use functions from traceslook_ui by path: crate::scope_multi::traceslook_ui::trace_look_editor_inline
