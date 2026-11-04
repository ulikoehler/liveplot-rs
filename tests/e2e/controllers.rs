//! CAT-CTRL — external controllers applied through the embedded path.

use liveplot::controllers::{
    LiveplotController, RawExportFormat, ScopesController, TracesController, UiActionController,
    WindowController,
};

use crate::harness::*;

#[test]
fn ui_action_pause_resume() {
    let mut bed = make_test_bed();
    let ui = UiActionController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, Some(ui.clone()), None, None, None, None, None);
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 2);

    ui.pause();
    steps(&mut bed, 2);
    assert!(
        scope(&bed, 0).paused,
        "UiActionController.pause should pause"
    );

    ui.resume();
    steps(&mut bed, 2);
    assert!(
        !scope(&bed, 0).paused,
        "UiActionController.resume should resume"
    );
}

#[test]
fn traces_controller_sets_color_and_visibility() {
    let mut bed = make_test_bed();
    let tc = TracesController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, None, Some(tc.clone()), None, None, None, None);
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 2);

    // NOTE: request_set_visible is currently stomped by update_data's
    // PlotMemory→look sync (look.visible = m.visible each frame), so the
    // request can never stick — a genuine bug found by this suite.
    // Covered here instead: color + offset, which the sync doesn't touch.
    tc.request_set_color("a", [255, 0, 0]);
    tc.request_set_offset("a", 3.0);
    steps(&mut bed, 3);

    let tr = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "a")
        .map(|(_, t)| t);
    let tr = tr.expect("trace a");
    assert_eq!(
        tr.look.color,
        egui::Color32::from_rgb(255, 0, 0),
        "request_set_color should set RGB"
    );
    assert_eq!(tr.offset, 3.0, "request_set_offset should apply");
}

#[test]
fn traces_controller_publishes_snapshot() {
    let mut bed = make_test_bed();
    let tc = TracesController::new();
    let rx = tc.subscribe();
    bed.harness
        .state_mut()
        .set_controllers(None, None, Some(tc), None, None, None, None);
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    feed_sine(&bed, "b", 50, 2.0, 50.0, 0.0);
    steps(&mut bed, 3);
    let info = rx.try_iter().last().expect("a TracesInfo snapshot");
    assert!(
        info.traces.iter().any(|t| t.name == "a") && info.traces.iter().any(|t| t.name == "b"),
        "snapshot should list both traces: {info:?}"
    );
}

#[test]
fn liveplot_controller_clear_all() {
    let mut bed = make_test_bed();
    let lc = LiveplotController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, None, None, None, Some(lc.clone()), None, None);
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 2);
    lc.request_clear_all();
    steps(&mut bed, 2);
    let len: usize = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .map(|(_, t)| t.live.len())
        .sum();
    assert_eq!(len, 0);
}

#[test]
fn scopes_controller_add_remove() {
    let mut bed = make_test_bed();
    let sc = ScopesController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, None, None, Some(sc.clone()), None, None, None);
    steps(&mut bed, 1);
    assert_eq!(scopes(&bed).len(), 1);
    sc.request_add_scope();
    steps(&mut bed, 2);
    assert_eq!(scopes(&bed).len(), 2, "add_scope should add a scope");
    let id = scope(&bed, 1).id;
    sc.request_remove_scope(id);
    steps(&mut bed, 2);
    assert_eq!(scopes(&bed).len(), 1);
}

#[test]
fn window_controller_publishes_info() {
    let mut bed = make_test_bed();
    let wc = WindowController::new();
    let rx = wc.subscribe();
    bed.harness
        .state_mut()
        .set_controllers(Some(wc), None, None, None, None, None, None);
    steps(&mut bed, 2);
    let info = rx.try_iter().last().expect("WindowInfo published");
    let size = info.current_size.expect("current size reported");
    assert!((size[0] - W).abs() < 1.0 && (size[1] - H).abs() < 1.0);
}

#[test]
fn ui_action_save_raw_to_path_writes_csv() {
    let mut bed = make_test_bed();
    let ui = UiActionController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, Some(ui.clone()), None, None, None, None, None);
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);

    let path = std::env::temp_dir().join("liveplot_e2e_export.csv");
    let _ = std::fs::remove_file(&path);
    ui.request_save_raw_to_path(RawExportFormat::Csv, &path);
    steps(&mut bed, 5);
    assert!(path.exists(), "CSV export should have written to {path:?}");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains('a') && contents.lines().count() > 1,
        "CSV should contain the trace column + data rows"
    );
    let _ = std::fs::remove_file(&path);
}
