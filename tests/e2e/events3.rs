//! CAT-EVT — trace visibility/color events via the controller path.

use liveplot::controllers::TracesController;
use liveplot::events::{EventController, EventKind};

use crate::harness::*;

/// EVT-08: controller visibility change emits TRACE_HIDDEN/SHOWN.
#[test]
fn visibility_change_emits_trace_event() {
    let mut bed = make_test_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    let tc = TracesController::new();
    {
        let panel = bed.harness.state_mut();
        panel.set_event_controller(Some(ctrl));
        panel.set_controllers(None, None, Some(tc.clone()), None, None, None, None);
    }
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 3);
    let _ = rx.try_iter().count();

    tc.request_set_visible("a", false);
    steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::TRACE_HIDDEN)),
        "expected TRACE_HIDDEN event, got: {events:?}"
    );

    tc.request_set_visible("a", true);
    steps(&mut bed, 3);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::TRACE_SHOWN)),
        "expected TRACE_SHOWN event, got: {events:?}"
    );
}
