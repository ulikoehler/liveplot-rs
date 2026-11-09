use crate::harness::*;
/// Epoch-magnitude x values (live timestamps ~1.79e9) must still render
/// — f32-precision translate in the surface. Regression for the
/// "all points on one timestamp" bug.
#[test]
fn volcano_epoch_data_renders() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_794_220_000.0);
    scope_mut(&mut bed, 0).volcano_enabled = true;
    steps(&mut bed, 20);
    let s = scope(&bed, 0);
    let surf = s.volcano_surface.as_ref().expect("surface exists");
    assert!(surf.initialized(), "surface must render with epoch x");
    assert!(s.rendered_this_frame);
    assert!(s.legend_overlay_rect.is_some(), "legend must render");
    if let Ok(img) = bed.harness.render() {
        img.save("/tmp/volcano_epoch_scope.png").ok();
    }
}

/// Autoscroll must advance the axis while data streams — regression
/// for "time scale doesn't update with scrolling".
#[test]
fn volcano_epoch_autoscroll_advances() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_794_220_000.0);
    scope_mut(&mut bed, 0).volcano_enabled = true;
    steps(&mut bed, 15);
    let b0 = scope(&bed, 0).x_axis.bounds.1;
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_794_220_040.0);
    steps(&mut bed, 15);
    let b1 = scope(&bed, 0).x_axis.bounds.1;
    assert!(b1 > b0, "autoscroll must advance: {b0} -> {b1}");
    // The rendered viewport (bounds - shift_ref) must also advance —
    // the label space must not be frozen at 0..window.
    let surf = scope(&bed, 0).volcano_surface.as_ref().unwrap();
    eprintln!("bounds {b0} -> {b1}, surf.bounds {:?}", surf.bounds);
    if let Ok(img) = bed.harness.render() {
        img.save("/tmp/volcano_epoch_scroll.png").ok();
    }
}

/// Scroll-zoom on the volcano path must change AND keep the bounds —
/// regression for "cannot zoom on the x axis" (live_update rewrote
/// data.x_axis.bounds every frame, reverting the user's zoom).
#[test]
fn volcano_zoom_persists_past_live_update() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_794_220_000.0);
    scope_mut(&mut bed, 0).volcano_enabled = true;
    steps(&mut bed, 15);
    let rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let c = egui::pos2(
        rect[0] + (rect[2] - rect[0]) / 2.0,
        rect[1] + (rect[3] - rect[1]) / 2.0,
    );
    let before = scope(&bed, 0).x_axis.bounds;
    bed.harness.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -80.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    bed.harness.input_mut().events.push(egui::Event::PointerMoved(c));
    // smooth_scroll_delta decays over ~8 frames — let it settle so
    // user_bounds reaches the final zoomed view before we capture it.
    steps(&mut bed, 15);
    // `data.x_axis.bounds` is always the live window — live_update
    // owns it. The ZOOMED viewport lives on the surface (user_bounds)
    // and must survive further streaming.
    let zoomed = scope(&bed, 0)
        .volcano_surface
        .as_ref()
        .and_then(|s| s.user_bounds);
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_794_220_040.0);
    steps(&mut bed, 10);
    let after = scope(&bed, 0)
        .volcano_surface
        .as_ref()
        .and_then(|s| s.user_bounds);
    let live = scope(&bed, 0).x_axis.bounds;
    let Some(zb) = zoomed else { panic!("zoom must take ownership of the view") };
    assert_ne!(
        (live.0, live.1), (zb.0[0], zb.0[1]),
        "zoomed view must differ from the live window"
    );
    assert_eq!(
        zoomed, after,
        "zoomed viewport must persist across live_update rewrites"
    );
}
