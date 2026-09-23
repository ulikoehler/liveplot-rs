use liveplot::data::data::{LivePlotData, LivePlotRequests};
use liveplot::data::marker::Marker;
use liveplot::data::measurement::Measurement;
use liveplot::data::scope::ScopeData;
use liveplot::data::traces::{TraceRef, TracesCollection};
use liveplot::panels::MeasurementPanel;
use liveplot::panels::Panel;

fn make_live<'a>(
    scopes: Vec<&'a mut ScopeData>,
    traces: &'a mut TracesCollection,
    requests: &'a mut LivePlotRequests,
) -> LivePlotData<'a> {
    LivePlotData {
        scope_data: scopes,
        traces,
        pending_requests: requests,
        event_ctrl: None,
        settings_changed: false,
    }
}

#[test]
fn marker_makes_scope_measurement_active() {
    // A panel containing markers must mark scopes as measurement-active so
    // clicks capture a clicked_point instead of resuming.
    let mut panel = MeasurementPanel::default();
    panel.set_markers(vec![Marker::new("K1", 0)]);

    let mut scope = ScopeData::default();
    let mut traces = TracesCollection::default();
    let mut requests = LivePlotRequests::default();
    let mut live = make_live(vec![&mut scope], &mut traces, &mut requests);

    panel.update_data(&mut live);
    assert!(live.scope_by_id(0).unwrap().measurement_active);
}

#[test]
fn selected_marker_receives_click() {
    let mut panel = MeasurementPanel::default();
    panel.restore_markers(vec![Marker::new("K1", 0)], Some(0));

    let mut scope = ScopeData::default();
    scope.clicked_point = Some([1.5, -2.0]);
    let mut traces = TracesCollection::default();
    let mut requests = LivePlotRequests::default();

    {
        let mut live = make_live(vec![&mut scope], &mut traces, &mut requests);
        panel.update_data(&mut live);
    }

    let markers = panel.markers();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].point, Some([1.5, -2.0]));
    assert_eq!(markers[0].scope_id, Some(0));
    assert!(panel.take_markers_dirty());
}

#[test]
fn unselected_marker_click_goes_to_measurement() {
    // With both a measurement and an unselected marker present, the click
    // must go to the measurement (default routing) and not the marker.
    let mut panel = MeasurementPanel::default();
    panel.restore_measurements(vec![Measurement::new("M1")], Some(0));
    panel.restore_markers(vec![Marker::new("K1", 0)], None);

    let mut scope = ScopeData::default();
    scope.clicked_point = Some([3.0, 4.0]);
    let mut traces = TracesCollection::default();
    let mut requests = LivePlotRequests::default();

    {
        let mut live = make_live(vec![&mut scope], &mut traces, &mut requests);
        panel.update_data(&mut live);
    }

    let (p1, p2) = panel.measurements()[0].get_points();
    assert_eq!(p1, Some([3.0, 4.0]));
    assert!(p2.is_none());
    assert!(panel.markers()[0].point.is_none());
}

#[test]
fn marker_catch_trace_snaps_to_nearest_point() {
    let mut panel = MeasurementPanel::default();
    let mut marker = Marker::new("K1", 0);
    marker.catch_trace = Some(TraceRef("sig".to_string()));
    panel.restore_markers(vec![marker], Some(0));

    let mut traces = TracesCollection::default();
    {
        let t = traces.get_trace_or_new(&TraceRef("sig".to_string()));
        t.live.push_back([1.0, 10.0]);
        t.live.push_back([2.0, 20.0]);
    }

    let mut scope = ScopeData::default();
    // Click near (2, 20) — the marker should snap to the trace point.
    scope.clicked_point = Some([1.9, 19.0]);
    let mut requests = LivePlotRequests::default();

    {
        let mut live = make_live(vec![&mut scope], &mut traces, &mut requests);
        panel.update_data(&mut live);
    }

    assert_eq!(panel.markers()[0].point, Some([2.0, 20.0]));
}

#[test]
fn clear_markers_request_clears_points() {
    let mut panel = MeasurementPanel::default();
    let mut marker = Marker::new("K1", 0);
    marker.point = Some([1.0, 2.0]);
    marker.scope_id = Some(0);
    panel.restore_markers(vec![marker], Some(0));

    let mut scope = ScopeData::default();
    let mut traces = TracesCollection::default();
    let mut requests = LivePlotRequests {
        clear_markers: true,
        ..Default::default()
    };

    {
        let mut live = make_live(vec![&mut scope], &mut traces, &mut requests);
        panel.update_data(&mut live);
    }

    assert!(panel.markers()[0].point.is_none());
    assert!(panel.markers()[0].scope_id.is_none());
    assert!(panel.selected_marker_index().is_none());
    assert!(panel.take_markers_dirty());
}

#[test]
fn set_markers_does_not_mark_dirty() {
    let mut panel = MeasurementPanel::default();
    panel.set_markers(vec![Marker::new("K1", 0)]);
    // External application of a marker list must not re-emit a change.
    assert!(!panel.take_markers_dirty());
}

#[test]
fn marker_serde_roundtrip() {
    let mut marker = Marker::new("K7", 3);
    marker.point = Some([4.0, 5.0]);
    marker.scope_id = Some(2);
    marker.show_hline = true;
    marker.show_vline = true;
    marker.shape = egui_plot::MarkerShape::Diamond;
    marker.catch_trace = Some(TraceRef("sig".to_string()));

    let serde = liveplot::persistence::MarkerSerde::from(&marker);
    let json = serde_json::to_string(&serde).unwrap();
    let back: liveplot::persistence::MarkerSerde = serde_json::from_str(&json).unwrap();
    let restored = back.into_marker();

    assert_eq!(restored.name, "K7");
    assert_eq!(restored.point, Some([4.0, 5.0]));
    assert_eq!(restored.scope_id, Some(2));
    assert!(restored.show_hline);
    assert!(restored.show_vline);
    assert!(restored.show_point);
    assert_eq!(restored.shape, egui_plot::MarkerShape::Diamond);
    assert_eq!(restored.catch_trace, Some(TraceRef("sig".to_string())));
    assert_eq!(restored.color, marker.color);
}
