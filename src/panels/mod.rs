//! Panels module: modular UI components.
//!
//! MERGED from Janosch: liveplot-rs-janosch/src/panels/

pub mod panel_trait;
pub mod scope_ui;
pub mod trace_look_ui;
pub mod traces_ui;
pub mod math_ui;
pub mod thresholds_ui;
pub mod triggers_ui;
pub mod export_ui;
pub mod liveplot_ui;
pub mod measurment_ui;

pub use panel_trait::{Panel, PanelState};
pub use scope_ui::ScopePanel;
pub use liveplot_ui::LiveplotPanel;
pub use traces_ui::TracesPanel;
pub use math_ui::MathPanel;
pub use thresholds_ui::ThresholdsPanel;
pub use triggers_ui::TriggersPanel;
pub use export_ui::ExportPanel;
pub use measurment_ui::MeasurementPanel;
