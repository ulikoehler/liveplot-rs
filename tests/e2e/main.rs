mod harness;
mod signals;

mod a11y;
mod app_level;
mod axes;
mod config_flags;
mod controllers;
mod controllers2;
mod events;
mod export;
mod hotkeys_e2e;
mod hotkeys_more;
mod legend;
mod legend2;
mod math;
mod math2;
mod meas_events;
mod measurement;
mod panels;
mod persistence;
mod reorder;
mod scopes;
mod sink;
mod smoke;
mod snapshots;
mod soak;
mod stress;
mod thresholds;
mod traces_panel;
mod triggers;
mod xy_scope;

#[cfg(feature = "fft")]
mod fft;
#[cfg(feature = "fft")]
mod fft_controller;
