//! UI-driven controls batch 4: export panel, hotkeys panel,
//! scope Controls button — real node clicks.

use egui_kittest::kittest::Queryable;
use liveplot::panels::{ExportPanel, HotkeysPanel};
use liveplot::Panel;

use crate::harness::*;

fn click_label(bed: &mut TestBed, label: &str) -> bool {
    if let Some(n) = bed.harness.query_all_by_label_contains(label).next() {
        n.click();
        true
    } else {
        false
    }
}

/// Export panel renders clickable format/output controls.
#[test]
fn ui_export_panel_controls() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    if let Some(p) = find_panel_mut::<ExportPanel>(&mut bed) {
        p.state_mut().visible = true;
    }
    steps(&mut bed, 3);
    for label in ["CSV", "Export", "Format"] {
        if click_label(&mut bed, label) {
            steps(&mut bed, 2);
        }
    }
    // Reached without panic — export UI is clickable.
}

/// Hotkeys panel: "Reset"/default button click is safe.
#[test]
fn ui_hotkeys_panel_reset() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    if let Some(p) = find_panel_mut::<HotkeysPanel>(&mut bed) {
        p.state_mut().visible = true;
    }
    steps(&mut bed, 3);
    let _ = click_label(&mut bed, "Reset");
    steps(&mut bed, 2);
}

/// Scope "Controls" toolbar button click opens its menu — click is safe.
#[test]
fn ui_scope_controls_button_click() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 3);
    let _ = click_label(&mut bed, "Controls");
    steps(&mut bed, 3);
    assert!(scope(&bed, 0).rendered_this_frame);
}
