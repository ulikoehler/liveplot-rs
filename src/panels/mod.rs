pub mod scope_ui;
// Minimal build: only include Scope panel for now
pub mod panel_trait;
pub mod traces_ui;
pub mod math_ui;
pub mod thresholds_ui;
pub mod triggers_ui;
// pub mod fft_ui;
pub mod export_ui;
pub mod trace_look_ui;
pub mod measurment_ui;
pub mod liveplot_ui;

#[cfg(feature = "fft")]
pub mod fft_ui;
