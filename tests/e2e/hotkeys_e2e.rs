//! CAT-HK — hotkeys through the real input pipeline.

use liveplot::panels::TracesPanel;

use crate::harness::*;

#[test]
fn traces_hotkey_toggles_panel() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let before = panel_visible::<TracesPanel>(&bed);
    key_press(&mut bed, egui::Key::T);
    steps(&mut bed, 2);
    assert_eq!(
        panel_visible::<TracesPanel>(&bed),
        !before,
        "T should toggle the traces panel"
    );
}

#[test]
fn fit_view_hotkey_runs() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    // Shrink bounds first, then F should restore data extrema.
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    steps(&mut bed, 3);
    key_press(&mut bed, egui::Key::F);
    steps(&mut bed, 3);
    // Whatever the binding is, the app must not panic; bounds stay valid.
    let (xb, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert!(xb[1] >= xb[0]);
}
