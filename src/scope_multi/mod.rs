//! Multi-trace oscilloscope UI (modularized).

mod types;
mod app;
mod math_ui;
mod thresholds_ui;
mod fft_panel;
mod export_helpers;

pub use app::{ScopeAppMulti, run_liveplot};
