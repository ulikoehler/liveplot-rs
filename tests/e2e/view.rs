//! CAT-VIEW — zoom, pan, box-zoom, fit via real pointer/scroll input.

use crate::harness::*;

fn plot_rect(bed: &TestBed) -> egui::Rect {
    let r = scope(bed, 0).last_plot_screen_rect.unwrap();
    egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]))
}

fn bounds(bed: &TestBed) -> ([f64; 2], [f64; 2]) {
    scope(bed, 0).last_plot_bounds.unwrap()
}

fn drag(bed: &mut TestBed, from: egui::Pos2, to: egui::Pos2, button: egui::PointerButton) {
    use egui::Event;
    let ev = |pos, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    for e in [
        Event::PointerMoved(from),
        ev(from, true),
        Event::PointerMoved(to),
        ev(to, false),
    ] {
        bed.harness.input_mut().events.push(e);
        bed.harness.step();
    }
}

#[test]
fn scroll_zooms_around_pointer() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    steps(&mut bed, 3);
    let (xb, _) = bounds(&bed);
    let span0 = xb[1] - xb[0];

    let center = plot_rect(&bed).center();
    bed.harness.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -80.0),
        modifiers: egui::Modifiers::NONE,
    });
    bed.harness.step();
    steps(&mut bed, 1);
    let (xb2, _) = bounds(&bed);
    assert!(
        xb2[1] - xb2[0] < span0,
        "zoom-out expected smaller span: {span0} -> {}",
        xb2[1] - xb2[0]
    );
}

#[test]
fn primary_drag_pans() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    steps(&mut bed, 3);
    let (xb, _) = bounds(&bed);
    let rect = plot_rect(&bed);
    drag(
        &mut bed,
        rect.center(),
        rect.center() + egui::vec2(120.0, 0.0),
        egui::PointerButton::Primary,
    );
    steps(&mut bed, 2);
    let (xb2, _) = bounds(&bed);
    assert!(
        (xb2[0] - xb[0]).abs() > 1e-6 || (xb2[1] - xb[1]).abs() > 1e-6,
        "pan should move x bounds: {xb:?} -> {xb2:?}"
    );
}

#[test]
fn boxed_zoom_selects_region() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    steps(&mut bed, 3);
    let (xb, _) = bounds(&bed);
    let span0 = xb[1] - xb[0];
    let rect = plot_rect(&bed);
    // Drag a small box in the middle → bounds shrink to the box.
    let from = rect.center();
    let to = rect.center() + egui::vec2(80.0, 60.0);
    drag(&mut bed, from, to, egui::PointerButton::Secondary);
    steps(&mut bed, 2);
    let (xb2, _) = bounds(&bed);
    assert!(
        xb2[1] - xb2[0] < span0 * 0.9,
        "box zoom should shrink span: {span0} -> {}",
        xb2[1] - xb2[0]
    );
}
