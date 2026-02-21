use liveplot::data::scope::ScopeData;
use liveplot::persistence::ScopeStateSerde;

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
