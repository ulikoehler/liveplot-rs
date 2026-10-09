use liveplot::panels::scope_ui::grid_step_sizes;

/// Regression test: egui_plot's `log_grid_spacer` returns no marks once
/// `base_step_size` (axis_span / frame_px * grid_spacing.min) drops below
/// `f64::EPSILON` — a visible span of ~1e-14 at typical plot heights — which
/// made all axis tick labels and grid lines disappear at deep zoom. Our
/// spacer must keep emitting marks at extreme zoom levels.
#[test]
fn grid_spacer_emits_marks_at_extreme_zoom() {
    let input = || egui_plot::GridInput {
        bounds: (-1e-15, 1e-15),
        base_step_size: 1e-16,
    };

    // Sanity: the built-in spacer really is degenerate here.
    assert!(egui_plot::log_grid_spacer(10)(input()).is_empty());

    let marks = egui_plot::uniform_grid_spacer(grid_step_sizes)(input());
    assert!(!marks.is_empty());
    assert!(marks.iter().all(|m| m.value.is_finite()));
}

#[test]
fn grid_spacer_handles_degenerate_input() {
    for base in [0.0, f64::NAN, f64::INFINITY, -1e-16] {
        let marks = egui_plot::uniform_grid_spacer(grid_step_sizes)(egui_plot::GridInput {
            bounds: (-1.0, 1.0),
            base_step_size: base,
        });
        assert!(marks.len() < 10_000, "unexpected mark count for {base}");
    }
}
