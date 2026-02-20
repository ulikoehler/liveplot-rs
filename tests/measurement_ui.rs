use liveplot::data::data::{LivePlotData, LivePlotRequests};
use liveplot::data::scope::ScopeData;
use liveplot::data::traces::TracesCollection;
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

    let mut scope_refs: Vec<&mut ScopeData> = vec![&mut scope];
    let mut live = LivePlotData {
        scope_data: scope_refs,
        traces: &mut traces,
        pending_requests: &mut requests,
    };

    // precondition: no measurements -> measurement_active should be false
    assert!(!live.scope_by_id(0).unwrap().measurement_active);
    panel.update_data(&mut live);
    assert!(!live.scope_by_id(0).unwrap().measurement_active);
}

#[test]
fn measurement_menu_labels_include_icons() {
    assert_eq!(
        MeasurementPanel::SHOW_MEASUREMENTS_LABEL,
        "👁 Show Measurements"
    );
    assert_eq!(MeasurementPanel::TAKE_P1_LABEL, "⌖ Take P1 at click");
    assert_eq!(MeasurementPanel::TAKE_P2_LABEL, "⌖ Take P2 at click");
}
