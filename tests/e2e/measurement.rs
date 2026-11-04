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
