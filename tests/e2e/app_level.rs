//! App-level e2e — runs the full `LivePlotApp` (eframe::App) so
//! postprocess logic (event emission, save/load, screenshot routing) is
//! exercised, not just the panel widget.

use egui_kittest::kittest::Queryable;
use liveplot::controllers::LiveplotController;
use liveplot::events::{EventController, EventKind};

use crate::harness::*;
use crate::signals::SignalSpec;

fn feed_sine_app(bed: &AppBed, name: &str, n: usize, freq: f64, fs: f64, t0: f64) {
    let trace = bed.sink.create_trace(name, None);
    bed.sink
        .send_points(
            &trace,
            crate::signals::gen(&SignalSpec::sine(freq), n, fs, t0),
        )
        .unwrap();
}

fn app_steps(bed: &mut AppBed, n: usize) {
    for _ in 0..n {
        bed.harness.step();
    }
}

#[test]
fn click_to_pause_emits_click_and_pause_events() {
    // The scope emits CLICK|PAUSE when a plot click pauses it
    // (`pause_on_click`), and CLICK|RESUME when clicking again resumes.
    let mut bed = make_app_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    {
        let app = bed.harness.state_mut();
        app.main_panel.set_event_controller(Some(ctrl));
        let mut scopes = app.main_panel.liveplot_panel.get_data_mut();
        for s in scopes.iter_mut() {
            s.pause_on_click = true;
        }
    }
    feed_sine_app(&bed, "a", 10, 1.0, 10.0, 0.0);
    app_steps(&mut bed, 3);
    let _ = rx.try_iter().count(); // drain setup events

    // Click in the middle of the plot.
    let r = {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        scopes[0].last_plot_screen_rect.unwrap()
    };
    let center = egui::pos2((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    {
        use egui::Event;
        for ev in [
            Event::PointerMoved(center),
            Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ] {
            bed.harness.input_mut().events.push(ev);
            bed.harness.step();
        }
    }
    app_steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events.iter().any(|e| e.kinds.contains(EventKind::CLICK)),
        "expected CLICK event, got: {events:?}"
    );
    assert!(
        events.iter().any(|e| e.kinds.contains(EventKind::PAUSE)),
        "expected PAUSE event, got: {events:?}"
    );
}

#[test]
fn liveplot_controller_save_state_writes_file() {
    let mut bed = make_app_bed();
    let lc = LiveplotController::new();
    {
        let app = bed.harness.state_mut();
        app.liveplot_ctrl = Some(lc.clone());
    }
    feed_sine_app(&bed, "persist_me", 50, 1.0, 50.0, 0.0);
    app_steps(&mut bed, 2);

    let path = std::env::temp_dir().join("liveplot_e2e_state.json");
    let _ = std::fs::remove_file(&path);
    lc.request_save_state(&path);
    app_steps(&mut bed, 5);
    assert!(path.exists(), "save_state should write {path:?}");
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.len() > 10, "state file unexpectedly small");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn screenshot_request_does_not_panic() {
    let mut bed = make_app_bed();
    let lc = LiveplotController::new();
    bed.harness.state_mut().liveplot_ctrl = Some(lc.clone());
    feed_sine_app(&bed, "a", 10, 1.0, 10.0, 0.0);
    app_steps(&mut bed, 2);
    // Route a screenshot request; without a real GPU frame the capture
    // simply won't produce an image — the app must stay alive.
    bed.harness
        .state_mut()
        .main_panel
        .pending_requests
        .screenshot = Some(liveplot::data::data::ScreenshotRequest {
        target: liveplot::data::data::ScreenshotTarget::CenterPanel,
        path: None,
    });
    app_steps(&mut bed, 5);
    assert!(bed
        .harness
        .query_all_by_label_contains("Scopes")
        .next()
        .is_some());
}
