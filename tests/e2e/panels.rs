//! CAT-UI + CAT-PAUSE — side-panel chrome and pause/resume.

use egui_kittest::kittest::Queryable;

use liveplot::panels::{MeasurementPanel, ThresholdsPanel, TracesPanel};
use liveplot::Panel;

use crate::harness::*;

#[test]
fn top_bar_renders_all_default_buttons() {
    // Wide enough that every button keeps its text label (narrow windows
    // collapse some buttons to icon-only).
    let mut bed = make_test_bed_with(egui::vec2(1800.0, 800.0));
    bed.harness.step();
    for label in [
        "Scopes",
        "Traces",
        "Math",
        "Hotkeys",
        "Thresholds",
        "Triggers",
        "Measurement",
        "Color Scheme",
        "Export",
        "Pause",
        "Clear All",
        "Undo",
        "Redo",
    ] {
        assert!(
            bed.harness
                .query_all_by_label_contains(label)
                .next()
                .is_some(),
            "missing top-bar button {label:?}"
        );
    }
}

#[test]
fn panel_visibility_toggles_render() {
    let mut bed = make_test_bed();
    assert!(!panel_visible::<TracesPanel>(&bed));

    {
        let panel = find_panel_mut::<TracesPanel>(&mut bed).unwrap();
        panel.state_mut().visible = true;
    }
    steps(&mut bed, 2);
    assert!(panel_visible::<TracesPanel>(&bed));

    // Toggling back hides it again.
    {
        let panel = find_panel_mut::<TracesPanel>(&mut bed).unwrap();
        panel.state_mut().visible = false;
    }
    steps(&mut bed, 2);
    assert!(!panel_visible::<TracesPanel>(&bed));
}

#[test]
fn pause_and_resume_via_state() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 200, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);
    assert!(!scope(&bed, 0).paused);

    // Pause through the panel API used by the Pause button.
    bed.harness.state_mut().pause_all();
    steps(&mut bed, 2);
    assert!(scope(&bed, 0).paused);
    // Snapshot exists while paused.
    let snap_len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .and_then(|(_, t)| t.snap.as_ref().map(|s| s.len()));
    assert_eq!(snap_len, Some(200));

    // Live data still accumulates while paused.
    feed_sine(&bed, "a", 100, 1.0, 100.0, 100.0);
    steps(&mut bed, 2);
    let live_len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "a")
        .map(|(_, t)| t.live.len());
    assert_eq!(live_len, Some(300));

    bed.harness.state_mut().resume_all();
    steps(&mut bed, 2);
    assert!(!scope(&bed, 0).paused);
}

#[test]
fn pause_hotkey_toggles() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    key_press(&mut bed, egui::Key::P);
    steps(&mut bed, 2);
    assert!(scope(&bed, 0).paused, "P should pause");
    key_press(&mut bed, egui::Key::P);
    steps(&mut bed, 2);
    assert!(!scope(&bed, 0).paused, "P again should resume");
}

#[test]
fn clear_all_empties_traces() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 50, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .map(|(_, t)| t.live.len());
    assert_eq!(len, Some(50));
    // Trigger the ClearAll button in the top bar (label contains "Clear").
    let clicked = {
        if let Some(btn) = bed.harness.query_all_by_label_contains("Clear").next() {
            btn.click();
            true
        } else {
            false
        }
    };
    if clicked {
        steps(&mut bed, 3);
    } else {
        bed.harness.state_mut().traces_data.clear_all();
    }
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .map(|(_, t)| t.live.len())
        .unwrap_or(0);
    assert_eq!(len, 0);
}

#[test]
fn panels_have_expected_types() {
    let bed = make_test_bed();
    // Default panel set exists (hidden or visible).
    assert!(find_panel::<TracesPanel>(&bed).is_some());
    assert!(find_panel::<ThresholdsPanel>(&bed).is_some());
    assert!(find_panel::<MeasurementPanel>(&bed).is_some());
}
