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
    // Save a debug PNG for manual inspection.
    if let Ok(img) = bed.harness.render() {
        img.save("/tmp/volcano_epoch_scope.png").ok();
    }
}
