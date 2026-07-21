use liveplot::events::*;
use liveplot::TraceRef;

#[test]
fn event_kind_union_and_intersection() {
    let click = EventKind::CLICK;
    let dbl = EventKind::DOUBLE_CLICK;
    let combined = click | dbl;
    assert!(combined.contains(click));
    assert!(combined.contains(dbl));
    assert!(combined.intersects(click));
    assert!(!EventKind::PAUSE.intersects(click));
}

#[test]
fn event_kind_all_matches_everything() {
    assert!(EventKind::ALL.contains(EventKind::CLICK));
    assert!(EventKind::ALL.contains(EventKind::ZOOM));
    assert!(EventKind::ALL.contains(EventKind::THRESHOLD_EXCEEDED));
}

#[test]
fn event_filter_matches() {
    let filter = EventFilter::only(EventKind::CLICK | EventKind::DOUBLE_CLICK);
    let mut evt = PlotEvent::new(EventKind::CLICK);
    evt.timestamp = 1.0;
    assert!(filter.matches(&evt));

    let evt2 = PlotEvent::new(EventKind::ZOOM);
    assert!(!filter.matches(&evt2));

    let evt3 = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
    assert!(filter.matches(&evt3));
}

#[test]
fn event_filter_all_matches_everything() {
    let filter = EventFilter::all();
    let evt = PlotEvent::new(EventKind::THRESHOLD_EXCEEDED);
    assert!(filter.matches(&evt));
}

#[test]
fn event_controller_subscribe_and_emit() {
    let ctrl = EventController::new();
    let rx_all = ctrl.subscribe_all();
    let rx_clicks = ctrl.subscribe(EventFilter::only(EventKind::CLICK));
    let rx_zoom = ctrl.subscribe(EventFilter::only(EventKind::ZOOM));

    // Emit a click event
    let evt = PlotEvent::new(EventKind::CLICK);
    ctrl.emit_filtered(evt);

    // All subscriber should get it
    assert!(rx_all.try_recv().is_ok());
    // Click subscriber should get it
    assert!(rx_clicks.try_recv().is_ok());
    // Zoom subscriber should not
    assert!(rx_zoom.try_recv().is_err());
}

#[test]
fn event_controller_combined_kinds() {
    let ctrl = EventController::new();
    let rx_click = ctrl.subscribe(EventFilter::only(EventKind::CLICK));
    let rx_meas = ctrl.subscribe(EventFilter::only(EventKind::MEASUREMENT_POINT));

    // Emit event that is both a click AND a measurement point
    let evt = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
    ctrl.emit_filtered(evt);

    assert!(rx_click.try_recv().is_ok());
    assert!(rx_meas.try_recv().is_ok());
}

#[test]
fn event_controller_timestamp_set_on_emit() {
    let ctrl = EventController::new();
    let rx = ctrl.subscribe_all();

    std::thread::sleep(std::time::Duration::from_millis(10));
    ctrl.emit_filtered(PlotEvent::new(EventKind::CLICK));

    let evt = rx.try_recv().unwrap();
    assert!(evt.timestamp > 0.0);
}

#[test]
fn event_kind_display() {
    // Single bit
    assert_eq!(format!("{}", EventKind::CLICK), "CLICK");
    assert_eq!(format!("{}", EventKind::DOUBLE_CLICK), "DOUBLE_CLICK");
    // Combined bits are joined with '|'
    let combo = EventKind::CLICK | EventKind::DOUBLE_CLICK;
    assert_eq!(format!("{}", combo), "CLICK|DOUBLE_CLICK");
    // ALL should print as "ALL"
    assert_eq!(format!("{}", EventKind::ALL), "ALL");
    // Unknown bits still produce hex representation
    let unknown = EventKind(1 << 63);
    assert!(format!("{}", unknown).starts_with("0x"));
}

#[test]
fn event_kinds_do_not_overlap() {
    // Verify that all defined constants have unique bit positions.
    let all_kinds = [
        EventKind::CLICK,
        EventKind::DOUBLE_CLICK,
        EventKind::CLICK_ON_TRACE,
        EventKind::PAUSE,
        EventKind::RESUME,
        EventKind::MEASUREMENT_POINT,
        EventKind::MEASUREMENT_COMPLETE,
        EventKind::MEASUREMENT_CLEARED,
        EventKind::TRACE_SHOWN,
        EventKind::TRACE_HIDDEN,
        EventKind::TRACE_COLOR_CHANGED,
        EventKind::MATH_TRACE_ADDED,
        EventKind::MATH_TRACE_REMOVED,
        EventKind::ZOOM,
        EventKind::FIT_TO_VIEW,
        EventKind::PAN,
        EventKind::RESIZE,
        EventKind::DATA_UPDATED,
        EventKind::DATA_CLEARED,
        EventKind::THRESHOLD_EXCEEDED,
        EventKind::THRESHOLD_ADDED,
        EventKind::THRESHOLD_REMOVED,
        EventKind::KEY_PRESSED,
        EventKind::EXPORT,
        EventKind::SCREENSHOT,
        EventKind::SCOPE_ADDED,
        EventKind::SCOPE_REMOVED,
        EventKind::TRIGGER_FIRED,
        EventKind::TRACE_OFFSET_CHANGED,
        EventKind::Y_LOG_CHANGED,
        EventKind::Y_UNIT_CHANGED,
    ];
    for (i, a) in all_kinds.iter().enumerate() {
        for (j, b) in all_kinds.iter().enumerate() {
            if i != j {
                assert!(
                    !a.intersects(*b),
                    "EventKind bits {} and {} overlap: {:b} & {:b}",
                    i,
                    j,
                    a.0,
                    b.0
                );
            }
        }
    }
}

#[test]
fn dropped_receiver_is_cleaned_up() {
    let ctrl = EventController::new();
    let rx1 = ctrl.subscribe_all();
    let rx2 = ctrl.subscribe_all();

    // Drop rx1
    drop(rx1);

    ctrl.emit_filtered(PlotEvent::new(EventKind::CLICK));
    // rx2 should still work
    assert!(rx2.try_recv().is_ok());

    // Emit again – the dead subscriber should have been pruned
    ctrl.emit_filtered(PlotEvent::new(EventKind::ZOOM));
    assert!(rx2.try_recv().is_ok());
}

#[test]
fn plot_event_carries_metadata() {
    let mut evt = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
    evt.click = Some(ClickMeta {
        screen_pos: Some(ScreenPos { x: 100.0, y: 200.0 }),
        plot_pos: Some(PlotPos { x: 1.5, y: 2.5 }),
        trace: Some(TraceRef("signal".into())),
        scope_id: Some(0),
    });
    evt.measurement = Some(MeasurementMeta {
        point_index: 0,
        point: [1.5, 2.5],
        p1: Some([1.5, 2.5]),
        p2: None,
        delta_x: None,
        delta_y: None,
        slope: None,
        distance: None,
        measurement_name: Some("M1".into()),
        trace: Some(TraceRef("signal".into())),
    });

    assert!(evt.kinds.contains(EventKind::CLICK));
    assert!(evt.click.is_some());
    assert!(evt.measurement.is_some());
    assert_eq!(evt.click.as_ref().unwrap().plot_pos.unwrap().x, 1.5);
}
