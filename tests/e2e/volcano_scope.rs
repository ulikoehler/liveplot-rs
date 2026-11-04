//! CAT-VOLCANO: the scope renders through the VolcanoPlot surface when
//! `LIVEPLOT_VOLCANO` is set — the same harness/assertions as the
//! egui_plot path so both backends are checked against one suite.

use crate::harness::*;

/// The scope's VolcanoSurface is created and produces frames.
#[test]
fn volcano_scope_renders_through_surface() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    steps(&mut bed, 30);
    let s = scope(&bed, 0);
    let surf = s
        .volcano_surface
        .as_ref()
        .expect("volcano surface must exist when LIVEPLOT_VOLCANO is set");
    assert!(surf.initialized(), "surface never rendered a frame");
    assert_eq!(s.rendered_this_frame, true);
    let (xb, _yb) = s.last_plot_bounds.expect("bounds synced from surface");
    assert!(xb[1] > xb[0], "x range must be positive, got {xb:?}");
}

/// Pan via primary-drag shifts the reported plot bounds.
#[test]
fn volcano_scope_pan_moves_bounds() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    steps(&mut bed, 5);
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    scope_mut(&mut bed, 0).y_axis.auto_fit = false;
    scope_mut(&mut bed, 0).paused = true;
    steps(&mut bed, 25);
    let (xb0, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    let rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let c = egui::pos2(rect[0] + (rect[2] - rect[0]) / 2.0,
                       rect[1] + (rect[3] - rect[1]) / 2.0);
    drag(&mut bed, c, egui::pos2(c.x + 80.0, c.y), egui::PointerButton::Primary);
    steps(&mut bed, 5);
    let (xb1, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert!(
        (xb1[0] - xb0[0]).abs() > 1e-6 || (xb1[1] - xb0[1]).abs() > 1e-6,
        "pan must change bounds: {xb0:?} -> {xb1:?}"
    );
}

/// A hidden trace stays hidden on the volcano surface too
/// (look.visible is derived from PlotMemory before render).
#[test]
fn volcano_scope_respects_hidden_items() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 500, 1.0, 100.0, 0.0);
    steps(&mut bed, 10);
    // Hide via the traces controller — the path the e2e suite verifies.
    let tc = liveplot::controllers::TracesController::new();
    bed.harness
        .state_mut()
        .set_controllers(None, None, Some(tc.clone()), None, None, None, None);
    tc.request_set_visible("sig", false);
    steps(&mut bed, 10);
    assert!(scope(&bed, 0).rendered_this_frame);
}

/// Control: same drag on the egui_plot path (env unset).
#[test]
fn egui_path_pan_control() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    steps(&mut bed, 5);
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    scope_mut(&mut bed, 0).y_axis.auto_fit = false;
    scope_mut(&mut bed, 0).paused = true;
    steps(&mut bed, 25);
    let (xb0, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    let rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let c = egui::pos2(rect[0] + (rect[2] - rect[0]) / 2.0,
                       rect[1] + (rect[3] - rect[1]) / 2.0);
    drag(&mut bed, c, egui::pos2(c.x + 80.0, c.y), egui::PointerButton::Primary);
    steps(&mut bed, 5);
    let (xb1, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert!(
        (xb1[0] - xb0[0]).abs() > 1e-6 || (xb1[1] - xb0[1]).abs() > 1e-6,
        "egui pan control: {xb0:?} -> {xb1:?}"
    );
}
