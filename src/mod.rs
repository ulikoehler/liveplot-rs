//! Multi-trace oscilloscope UI (modularized).

mod types;
mod app;
mod data;
mod ui;
mod hotkeys_ui;
mod menu_ui;
mod plot;
mod math;
mod thresholds;
mod math_ui;
mod traces_ui;
mod thresholds_ui;
mod trace_look;
pub mod hotkeys;
mod fft_panel;
mod export_helpers;
mod panel;

pub use app::{LivePlotApp, run_liveplot};
