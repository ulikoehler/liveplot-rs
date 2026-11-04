//! CAT-EVT (app level) — measurement events through the full app.

use liveplot::events::{EventController, EventKind};
use liveplot::panels::MeasurementPanel;

use crate::harness::*;

fn app_steps(bed: &mut AppBed, n: usize) {
    for _ in 0..n {
        bed.harness.step();
    }
}

#[test]
fn measurement_clicks_emit_events() {
    let mut bed = make_app_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    {
        let app = bed.harness.state_mut();
        app.main_panel.set_event_controller(Some(ctrl));
        for p in app
            .main_panel
            .left_side_panels
            .iter_mut()
            .chain(app.main_panel.right_side_panels.iter_mut())
            .chain(app.main_panel.bottom_panels.iter_mut())
        {
            if let Some(mp) = p.downcast_mut::<MeasurementPanel>() {
                mp.add_measurement();
            }
        }
    }
    feed_sine_app(&mut bed, "a", 100, 1.0, 100.0, 0.0);
    app_steps(&mut bed, 3);
    let _ = rx.try_iter().count();

    // Two clicks inside the plot.
    let r = {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        scopes[0].last_plot_screen_rect.unwrap()
    };
    let rect = egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]));
    for pos in [
        rect.center() + egui::vec2(-60.0, 0.0),
        rect.center() + egui::vec2(60.0, 0.0),
    ] {
        use egui::Event;
        for ev in [
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos,
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
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::MEASUREMENT_POINT)
                || e.kinds.contains(EventKind::MEASUREMENT_COMPLETE)),
        "expected measurement event, got: {events:?}"
    );
}

fn feed_sine_app(bed: &AppBed, name: &str, n: usize, freq: f64, fs: f64, t0: f64) {
    let trace = bed.sink.create_trace(name, None);
    bed.sink
        .send_points(
            &trace,
            crate::signals::gen(&crate::signals::SignalSpec::sine(freq), n, fs, t0),
        )
        .unwrap();
}
