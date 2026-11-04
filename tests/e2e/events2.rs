//! CAT-EVT — more event kinds: view-change, scope-manage.

use liveplot::events::{EventController, EventKind};

use crate::harness::*;

fn app_steps(bed: &mut AppBed, n: usize) {
    for _ in 0..n {
        bed.harness.step();
    }
}

fn feed(bed: &AppBed, name: &str, n: usize) {
    let trace = bed.sink.create_trace(name, None);
    bed.sink
        .send_points(
            &trace,
            crate::signals::gen(&crate::signals::SignalSpec::sine(1.0), n, 100.0, 0.0),
        )
        .unwrap();
}

/// EVT-04: zoom/pan emits ViewChange with bounds meta.
#[test]
fn view_change_event_emitted() {
    let mut bed = make_app_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    {
        let app = bed.harness.state_mut();
        app.main_panel.set_event_controller(Some(ctrl));
        app.main_panel.liveplot_panel.get_data_mut()[0]
            .x_axis
            .auto_fit = false;
    }
    feed(&bed, "a", 500);
    app_steps(&mut bed, 3);
    let _ = rx.try_iter().count();

    // Scroll-zoom inside the plot.
    let r = {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        scopes[0].last_plot_screen_rect.unwrap()
    };
    let center = egui::pos2((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    bed.harness
        .input_mut()
        .events
        .push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -80.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
    bed.harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(center));
    bed.harness.step();
    app_steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events.iter().any(|e| e.kinds.contains(EventKind::ZOOM)),
        "expected VIEW_CHANGE event, got: {events:?}"
    );
}

/// EVT-07: add/remove scope emits ScopeManage events.
#[test]
fn scope_manage_events_emitted() {
    let mut bed = make_app_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    {
        let app = bed.harness.state_mut();
        app.main_panel.set_event_controller(Some(ctrl));
    }
    app_steps(&mut bed, 2);
    let _ = rx.try_iter().count();

    bed.harness
        .state_mut()
        .main_panel
        .liveplot_panel
        .add_scope();
    app_steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::SCOPE_ADDED)),
        "expected SCOPE_MANAGE event, got: {events:?}"
    );
}
