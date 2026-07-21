use egui_phosphor_icons::icons::{CROSSHAIR, EYE};
use liveplot::data::data::{LivePlotData, LivePlotRequests};
use liveplot::data::scope::ScopeData;
use liveplot::data::traces::TracesCollection;
use liveplot::panels::measurment_ui::{SHOW_MEASUREMENTS_LABEL, TAKE_P1_LABEL, TAKE_P2_LABEL};
use liveplot::panels::MeasurementPanel;
use liveplot::panels::Panel;

#[test]
fn default_measurement_panel_has_no_measurements() {
    // Verify via the public API: an empty MeasurementPanel should *not* mark
    // scopes as measurement-active after update_data.
    let mut panel = MeasurementPanel::default();
    let mut scope = ScopeData::default();
    let mut traces = TracesCollection::default();
    let mut requests = LivePlotRequests::default();

    let scope_refs: Vec<&mut ScopeData> = vec![&mut scope];
    let mut live = LivePlotData {
        scope_data: scope_refs,
        traces: &mut traces,
        pending_requests: &mut requests,
        event_ctrl: None,
        settings_changed: false,
    };

    // precondition: no measurements -> measurement_active should be false
    assert!(!live.scope_by_id(0).unwrap().measurement_active);
    panel.update_data(&mut live);
    assert!(!live.scope_by_id(0).unwrap().measurement_active);
}
