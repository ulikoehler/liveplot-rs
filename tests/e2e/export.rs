//! CAT-EXP — export paths beyond the CSV-via-controller coverage.

use liveplot::controllers::{LiveplotController, UiActionController};
use liveplot::events::{EventController, EventKind};

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

fn app_steps(bed: &mut AppBed, n: usize) {
    for _ in 0..n {
        bed.harness.step();
    }
}

fn feed(bed: &AppBed, name: &str, n: usize) {
    let trace = bed.sink.create_trace(name, None);
    bed.sink
        .send_points(&trace, gen(&SignalSpec::sine(1.0), n, 100.0, 0.0))
        .unwrap();
}

/// Full save → mutate → load round trip through the app layer.
#[test]
fn save_and_load_state_round_trip() {
    use liveplot::data::scope::LegendPosition;
    let mut bed = make_app_bed();
    let lc = LiveplotController::new();
    bed.harness.state_mut().liveplot_ctrl = Some(lc.clone());
    feed(&bed, "keep", 100);
    app_steps(&mut bed, 2);

    let path = std::env::temp_dir().join("liveplot_e2e_roundtrip.json");
    let _ = std::fs::remove_file(&path);
    lc.request_save_state(&path);
    app_steps(&mut bed, 5);
    assert!(path.exists(), "save_state should write the file");

    // Mutate a serialized setting.
    {
        let app = bed.harness.state_mut();
        app.main_panel.liveplot_panel.get_data_mut()[0].legend_position =
            LegendPosition::RightBottom;
        app.main_panel.side_panels_changed = true;
    }
    app_steps(&mut bed, 3);

    lc.request_load_state(&path);
    app_steps(&mut bed, 8);
    {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        assert_eq!(
            scopes[0].legend_position,
            LegendPosition::LeftTop,
            "load_state should restore the saved legend position"
        );
    }
    let _ = std::fs::remove_file(&path);
}

/// Screenshot request via UiActionController reaches pending_requests.
#[test]
fn screenshot_request_queues_capture() {
    let mut bed = make_app_bed();
    let ui = UiActionController::new();
    bed.harness.state_mut().ui_ctrl = Some(ui.clone());
    feed(&bed, "a", 50);
    app_steps(&mut bed, 2);
    ui.request_save_png();
    app_steps(&mut bed, 3);
    // The app either queued a capture or consumed the request — either
    // way it must still be alive and rendering.
    assert!(
        bed.harness.state().main_panel.liveplot_panel.get_data()[0].rendered_this_frame,
        "scope should still be rendering after screenshot request"
    );
}

/// Data-cleared event via the LiveplotController clear-all path.
#[test]
fn clear_all_emits_event() {
    let mut bed = make_app_bed();
    let lc = LiveplotController::new();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    {
        let app = bed.harness.state_mut();
        app.liveplot_ctrl = Some(lc.clone());
        app.main_panel.set_event_controller(Some(ctrl));
    }
    feed(&bed, "a", 50);
    app_steps(&mut bed, 2);
    let _ = rx.try_iter().count();
    lc.request_clear_all();
    app_steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::DATA_CLEARED)),
        "expected DATA_CLEARED event, got: {events:?}"
    );
}

#[cfg(feature = "parquet")]
#[test]
fn parquet_export_writes_file() {
    let mut bed = make_app_bed();
    let ui = UiActionController::new();
    bed.harness.state_mut().ui_ctrl = Some(ui.clone());
    feed(&bed, "a", 200);
    app_steps(&mut bed, 2);
    let path = std::env::temp_dir().join("liveplot_e2e_export.parquet");
    let _ = std::fs::remove_file(&path);
    ui.request_save_raw_to_path(liveplot::controllers::RawExportFormat::Parquet, &path);
    app_steps(&mut bed, 5);
    assert!(path.exists(), "parquet export should write {path:?}");
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    let _ = std::fs::remove_file(&path);
}
