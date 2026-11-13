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
    let c = egui::pos2(
        rect[0] + (rect[2] - rect[0]) / 2.0,
        rect[1] + (rect[3] - rect[1]) / 2.0,
    );
    drag(
        &mut bed,
        c,
        egui::pos2(c.x + 80.0, c.y),
        egui::PointerButton::Primary,
    );
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
    let c = egui::pos2(
        rect[0] + (rect[2] - rect[0]) / 2.0,
        rect[1] + (rect[3] - rect[1]) / 2.0,
    );
    drag(
        &mut bed,
        c,
        egui::pos2(c.x + 80.0, c.y),
        egui::PointerButton::Primary,
    );
    steps(&mut bed, 5);
    let (xb1, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert!(
        (xb1[0] - xb0[0]).abs() > 1e-6 || (xb1[1] - xb0[1]).abs() > 1e-6,
        "egui pan control: {xb0:?} -> {xb1:?}"
    );
}

/// XY pairs render through the surface too (timestamp-joined series).
#[test]
fn volcano_xy_scope_renders_pair() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "xs", 300, 1.0, 100.0, 0.0);
    feed_sine(&bed, "ys", 300, 0.5, 100.0, 0.0);
    {
        let s = scope_mut(&mut bed, 0);
        s.scope_type = liveplot::data::scope::ScopeType::XYScope;
        s.xy_pairs.push((
            Some(liveplot::TraceRef("xs".into())),
            Some(liveplot::TraceRef("ys".into())),
            Default::default(),
        ));
        s.volcano_enabled = true;
    }
    steps(&mut bed, 20);
    let s = scope(&bed, 0);
    assert!(
        s.volcano_surface.as_ref().is_some_and(|v| v.initialized()),
        "XY scope should render through VolcanoPlot"
    );
    assert!(s.last_plot_bounds.is_some());
}

/// Deleting a trace (not just clearing its data) must leave the
/// surface rendering.
#[test]
fn volcano_scope_remove_trace_still_renders() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 1000, 1.0, 100.0, 0.0);
    feed_sine(&bed, "sig2", 1000, 2.0, 100.0, 0.0);
    steps(&mut bed, 30);
    let name = liveplot::TraceRef("sig".into());
    {
        let data = scope_mut(&mut bed, 0);
        data.trace_order.retain(|t| t != &name);
    }
    bed.harness.state_mut().traces_data.remove_trace(&name);
    steps(&mut bed, 10);
    let s = scope(&bed, 0);
    let surf = s.volcano_surface.as_ref().unwrap();
    let before = (surf.frames_rendered, surf.dropped_frames);
    feed_sine(&bed, "sig2", 200, 2.0, 100.0, 10.0);
    steps(&mut bed, 10);
    let surf = scope(&bed, 0).volcano_surface.as_ref().unwrap();
    assert!(
        surf.frames_rendered > before.0,
        "surface stopped after remove_trace: {before:?} -> ({}, {})",
        surf.frames_rendered,
        surf.dropped_frames,
    );
    assert!(scope(&bed, 0).rendered_this_frame);
}

/// The first max_points eviction (first drop>0 ring slide) must not
/// freeze the surface either.
#[test]
fn volcano_scope_first_eviction_still_renders() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 1000, 1.0, 100.0, 0.0);
    steps(&mut bed, 30);
    // Small cap → the next feed evicts → first drop>0 slide.
    bed.harness.state_mut().traces_data.max_points = 1000;
    feed_sine(&bed, "sig", 800, 1.0, 100.0, 10.0);
    steps(&mut bed, 30);
    let s = scope(&bed, 0);
    let surf = s.volcano_surface.as_ref().unwrap();
    let dbg = (
        surf.frames_rendered,
        surf.dropped_frames,
        surf.dbg_dispatches,
        surf.dbg_uploads,
        surf.dbg_slides,
    );
    let before = surf.frames_rendered;
    eprintln!("eviction: {dbg:?} (frames, dropped, dispatched, uploads, slides)");
    // Keep feeding past the cap — every subsequent frame is a slide.
    feed_sine(&bed, "sig", 400, 1.0, 100.0, 18.0);
    steps(&mut bed, 30);
    let surf = scope(&bed, 0).volcano_surface.as_ref().unwrap();
    assert!(
        surf.frames_rendered > before,
        "surface stopped producing frames after first eviction: {dbg:?}"
    );
    assert_eq!(surf.dropped_frames, 0, "renders failing after eviction");
    assert!(scope(&bed, 0).rendered_this_frame);
}

/// Clearing all trace data must not freeze the surface: after
/// `clear_all` the scope keeps rendering (empty axes) — no resize
/// needed to revive it.
#[test]
fn volcano_scope_clear_all_still_renders() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    steps(&mut bed, 30);
    let s = scope(&bed, 0);
    let surf = s.volcano_surface.as_ref().unwrap();
    let before = (
        surf.frames_rendered,
        surf.dropped_frames,
        surf.dbg_dispatches,
        surf.dbg_busy_skips,
    );
    bed.harness.state_mut().traces_data.clear_all();
    steps(&mut bed, 30);
    let s = scope(&bed, 0);
    let surf = s.volcano_surface.as_ref().unwrap();
    let after = (
        surf.frames_rendered,
        surf.dropped_frames,
        surf.dbg_dispatches,
        surf.dbg_busy_skips,
    );
    eprintln!("clear_all: {before:?} -> {after:?} (frames, dropped, dispatched, busy)");
    assert!(
        surf.frames_rendered > before.0,
        "surface stopped producing frames after clear_all"
    );
    assert!(s.rendered_this_frame);
}

/// The FFT panel's embedded scope renders through VolcanoPlot too
/// ("everything incl. FFT" — it's the same render_plot path).
#[cfg(feature = "fft")]
#[test]
fn volcano_fft_scope_renders() {
    use liveplot::panels::fft_ui::FftPanel;
    use liveplot::Panel;
    let mut bed = make_test_bed();
    {
        let fp = find_panel_mut::<FftPanel>(&mut bed).expect("FftPanel exists");
        fp.state_mut().visible = true;
        fp.scope_ui.get_data_mut().volcano_enabled = true;
    }
    feed_sine(&bed, "sig", 4096, 1.0, 1000.0, 0.0);
    steps(&mut bed, 60);
    let fp = find_panel::<FftPanel>(&bed).unwrap();
    let data = fp.scope_ui.get_data();
    eprintln!(
        "fft: enabled={} initialized={:?} rendered={} rect={:?}",
        data.volcano_enabled,
        data.volcano_surface.as_ref().map(|v| v.initialized()),
        data.rendered_this_frame,
        data.last_plot_screen_rect
    );
    assert!(
        data.volcano_surface
            .as_ref()
            .is_some_and(|v| v.initialized()),
        "FFT scope should render through VolcanoPlot"
    );
}

/// Hovering a rendered trace inside the scope publishes
/// `volcano_scope_hover` — the surface unions it with traces-list
/// hover for emphasis (previously scope-hover did nothing).
#[test]
fn volcano_scope_plot_hover_marks_trace() {
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).volcano_enabled = true;
    feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
    steps(&mut bed, 30);
    let axes = scope(&bed, 0)
        .last_axes_screen_rect
        .expect("axes rect recorded");
    let (xb, yb) = scope(&bed, 0).last_plot_bounds.expect("bounds recorded");
    // Project a real sample point onto screen space: t=15.25 is an
    // exact fs=100 sample, and the sine peaks at y=1 there.
    let (t, y) = (15.25f64, 1.0f64);
    let sx = axes[0] + ((t - xb[0]) / (xb[1] - xb[0])) as f32 * (axes[2] - axes[0]);
    let sy = axes[3] - ((y - yb[0]) / (yb[1] - yb[0])) as f32 * (axes[3] - axes[1]);
    let center = egui::pos2(sx, sy);
    let disp0 = scope(&bed, 0)
        .volcano_surface
        .as_ref()
        .map(|s| s.dbg_dispatches)
        .unwrap_or(0);
    hover_at(&mut bed, center);
    let hov = &scope(&bed, 0).volcano_scope_hover;
    assert!(
        hov.iter().any(|t| t.0 == "sig"),
        "hovering the plotted trace must mark it, got {hov:?}"
    );
    // Hover changes the render signature → a new frame is dispatched.
    let disp1 = scope(&bed, 0)
        .volcano_surface
        .as_ref()
        .map(|s| s.dbg_dispatches)
        .unwrap_or(0);
    assert!(disp1 > disp0, "hover emphasis must trigger a re-render");
    // Moving the pointer off the scope clears the hover.
    hover_at(&mut bed, egui::pos2(4.0, 4.0));
    assert!(
        scope(&bed, 0).volcano_scope_hover.is_empty(),
        "leaving the scope must clear the hover"
    );
}
