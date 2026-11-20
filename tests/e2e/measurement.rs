//! CAT-MEAS — measurement click routing through the real plot.

use liveplot::panels::MeasurementPanel;

use crate::harness::*;

#[test]
fn measurement_captures_plot_clicks() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 500, 1.0, 100.0, 0.0);
    {
        let mp = find_panel_mut::<MeasurementPanel>(&mut bed).unwrap();
        mp.add_measurement();
    }
    steps(&mut bed, 3);
    assert!(
        scope(&bed, 0).measurement_active,
        "scope should be measurement-active"
    );

    // Two clicks at different positions inside the plot.
    let r = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let rect = egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]));
    let p1 = rect.center() + egui::vec2(-80.0, 0.0);
    let p2 = rect.center() + egui::vec2(80.0, 0.0);
    click_at(&mut bed, p1);
    steps(&mut bed, 2);
    click_at(&mut bed, p2);
    steps(&mut bed, 2);

    let (a, b) = find_panel::<MeasurementPanel>(&bed).unwrap().measurements()[0].get_points();
    assert!(a.is_some(), "first click should set p1");
    assert!(b.is_some(), "second click should set p2");
    // Clicks were ~160px apart → x distance must be positive.
    assert!(b.unwrap()[0] > a.unwrap()[0]);
}

#[test]
fn deselected_measurement_does_not_capture_clicks() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 500, 1.0, 100.0, 0.0);
    {
        scope_mut(&mut bed, 0).pause_on_click = true;
        let mp = find_panel_mut::<MeasurementPanel>(&mut bed).unwrap();
        mp.add_measurement();
    }
    steps(&mut bed, 3);
    assert!(scope(&bed, 0).measurement_active);

    // Deselect everything — clicks must behave exactly as if no
    // measurement were configured at all.
    let ms = find_panel::<MeasurementPanel>(&bed)
        .unwrap()
        .measurements()
        .to_vec();
    find_panel_mut::<MeasurementPanel>(&mut bed)
        .unwrap()
        .restore_measurements(ms, None);
    steps(&mut bed, 2);
    assert!(
        !scope(&bed, 0).measurement_active,
        "nothing armed -> scope must not be measurement-active"
    );

    let r = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let rect = egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]));
    click_at(&mut bed, rect.center());
    steps(&mut bed, 2);

    let (a, b) = find_panel::<MeasurementPanel>(&bed).unwrap().measurements()[0].get_points();
    assert!(
        a.is_none() && b.is_none(),
        "deselected measurement must not capture plot clicks"
    );
    assert!(
        scope(&bed, 0).paused,
        "unarmed click should pause the scope like a normal click"
    );

    // A second rapid click at the same position is a double-click —
    // use a different position so it counts as a fresh single click.
    click_at(&mut bed, rect.center() + egui::vec2(60.0, 0.0));
    steps(&mut bed, 2);
    assert!(
        !scope(&bed, 0).paused,
        "second unarmed click should resume the scope"
    );

    let (a, b) = find_panel::<MeasurementPanel>(&bed).unwrap().measurements()[0].get_points();
    assert!(a.is_none() && b.is_none());
}
