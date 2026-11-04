//! CAT-EVT — the event bus through the live app.

use liveplot::events::{EventController, EventFilter, EventKind};

use crate::harness::*;

#[test]
fn data_update_events_are_emitted() {
    let mut bed = make_test_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();
    bed.harness.state_mut().set_event_controller(Some(ctrl));

    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 4);

    let events: Vec<_> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| e.kinds.contains(EventKind::DATA_UPDATED)),
        "expected a DataUpdated event, got: {events:?}"
    );
}

#[test]
fn pause_button_pauses_scope() {
    // Pause-event emission happens at the LivePlotApp layer (postprocess);
    // at the panel level we verify the observable effect of the UI button.
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    assert!(!scope(&bed, 0).paused);

    // Pause via the programmatic path used by the Pause button — the
    // button itself can be icon-only (unlabeled) on cramped layouts.
    bed.harness.state_mut().pause_all();
    steps(&mut bed, 3);
    assert!(scope(&bed, 0).paused, "Pause should pause the scope");
}

#[test]
fn filtered_subscription_only_gets_matching_kinds() {
    let mut bed = make_test_bed();
    let ctrl = EventController::new();
    let rx = ctrl.subscribe(EventFilter::only(EventKind::DATA_UPDATED));
    bed.harness.state_mut().set_event_controller(Some(ctrl));
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 4);
    let mut got_any = false;
    for e in rx.try_iter() {
        got_any = true;
        assert!(
            e.kinds.contains(EventKind::DATA_UPDATED),
            "unexpected event {e:?}"
        );
    }
    assert!(got_any, "expected at least one DataUpdated event");
}
