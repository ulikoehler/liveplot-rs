//! LivePlot crate root: re-exports and module wiring.
//!
//! This crate provides two ready-to-use plotting UIs built on egui/eframe:
//! - Single-trace oscilloscope (`scope`)
//! - Multi-trace oscilloscope (`scope_multi`)
//!
//! The monolithic implementation has been refactored into cohesive modules:
//! - `sink`: data types and channels to feed samples
//! - `controllers`: external control of window/FFT panel
//! - `config`: shared configuration and time formatting
//! - `scope`: single-trace UI and run helpers
//! - `scope_multi`: multi-trace UI and run helpers

mod point_selection;
mod fft;
mod line_draw;

pub mod sink;
pub mod controllers;
pub mod config;
pub mod scope_multi;

// Public re-exports for a compact external API
pub use fft::FftWindow;
pub use config::{LivePlotConfig, XDateFormat};
pub use controllers::{FftController, FftPanelInfo, WindowController, WindowInfo};
pub use sink::{channel_multi, MultiPlotSink, MultiSample};
pub use scope_multi::{run_multi, run_multi_with_config, run_multi_with_options, run_multi_with_options_and_controllers, ScopeAppMulti};

