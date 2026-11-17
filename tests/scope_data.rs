use liveplot::data::scope::{ScopeData, ScopeType};
use liveplot::data::traces::TracesCollection;
use liveplot::persistence::ScopeStateSerde;

/// Regression: an XY scope (e.g. the FFT panel) must refit y bounds when
/// the data values change even though the point count and x-range stay
/// identical — the old `(total_pts, max_ts)` cache key never changed for a
/// recomputed spectrum, freezing autofit so growing peaks clipped at the
/// top edge.
#[test]
fn xy_autofit_refits_when_magnitudes_change() {
    let mut scope = ScopeData::default();
    scope.scope_type = ScopeType::XYScope;
    scope.x_axis.auto_fit = true;
    scope.y_axis.auto_fit = true;
    scope.trace_order.push("fft".into());

    let mut traces = TracesCollection::default();
    let name: liveplot::data::traces::TraceRef = "fft".into();
    let fill = |traces: &mut TracesCollection, peak: f64| {
        let td = traces.get_trace_or_new(&name);
        td.live.clear();
        // Fixed frequency bins — same count, same x extent, only the
        // magnitudes change (like a recomputed FFT spectrum).
        for i in 0..512 {
            let f = i as f64;
            let mag = if i == 100 { peak } else { 0.01 };
            td.live.push_back([f, mag]);
        }
    };

    fill(&mut traces, 1.0);
    scope.update(&traces);
    let fit1 = scope.y_axis.bounds;
    assert!(fit1.1 >= 1.0, "first fit should cover peak 1.0: {fit1:?}");

    // Same bins, taller peak — bounds must grow, not freeze.
    fill(&mut traces, 4.0);
    scope.update(&traces);
    let fit2 = scope.y_axis.bounds;
    assert!(
        fit2.1 >= 4.0,
        "autofit must refit after magnitude change: {fit2:?}"
    );

    // And shrink back down when the peak falls.
    fill(&mut traces, 0.5);
    scope.update(&traces);
    let fit3 = scope.y_axis.bounds;
    assert!(
        fit3.1 < fit2.1,
        "autofit should shrink when peaks fall: {fit3:?} vs {fit2:?}"
    );
}

#[test]
fn default_pause_on_click_false() {
    let data = ScopeData::default();
    assert!(
        !data.pause_on_click,
        "new scopes should default to pause-on-click disabled"
    );
}

#[test]
fn persistence_round_trip_pause_on_click() {
    let mut data = ScopeData::default();
    data.pause_on_click = false;

    let serde: ScopeStateSerde = (&data).into();
    let mut restored = ScopeData::default();
    serde.apply_to(&mut restored);
    assert!(
        !restored.pause_on_click,
        "persistence should round-trip the pause_on_click flag"
    );
}

#[test]
fn scope_panel_api_controls_pause_click() {
    let mut panel = liveplot::panels::ScopePanel::new(0);
    // default should follow ScopeData, which is disabled now
    assert!(!panel.pause_on_click());
    panel.set_pause_on_click(false);
    assert!(!panel.pause_on_click());
}
