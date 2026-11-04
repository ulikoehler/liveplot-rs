mod harness;
mod signals;

mod app_level;
mod axes;
mod controllers;
mod events;
mod hotkeys_e2e;
mod legend;
mod math;
mod measurement;
mod panels;
mod persistence;
mod scopes;
mod sink;
mod smoke;
mod snapshots;
mod stress;
mod thresholds;
mod triggers;

#[cfg(feature = "fft")]
mod fft;
