// ==============================================================================
// MERGE STATUS: COMPLETE - ALL PANEL UIs MERGED TO MAIN CRATE (2024-12-01)
// ==============================================================================
//
// All files merged to main crate src/panels/:
//   - panel_trait.rs -> src/panels/panel_trait.rs (Panel trait, PanelState)
//   - scope_ui.rs -> src/panels/scope_ui.rs (ScopePanel)
//   - traces_ui.rs -> src/panels/traces_ui.rs (TracesPanel)
//   - math_ui.rs -> src/panels/math_ui.rs (MathPanel)
//   - thresholds_ui.rs -> src/panels/thresholds_ui.rs (ThresholdsPanel)
//   - triggers_ui.rs -> src/panels/triggers_ui.rs (TriggersPanel)
//   - export_ui.rs -> src/panels/export_ui.rs (ExportPanel)
//   - liveplot_ui.rs -> src/panels/liveplot_ui.rs (LiveplotPanel)
//   - measurment_ui.rs -> src/panels/measurment_ui.rs (MeasurementPanel)
//   - trace_look_ui.rs -> src/panels/trace_look_ui.rs (render_trace_look_editor)
//   - fft_ui.rs -> (NOT merged - main crate has separate fft_panel.rs)

pub mod scope_ui;
// Minimal build: only include Scope panel for now
pub mod math_ui;
pub mod panel_trait;
pub mod thresholds_ui;
pub mod traces_ui;
pub mod triggers_ui;
// pub mod fft_ui;
pub mod export_ui;
pub mod liveplot_ui;
pub mod measurment_ui;
pub mod trace_look_ui;

#[cfg(feature = "fft")]
pub mod fft_ui;
