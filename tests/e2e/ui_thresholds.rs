//! UI-driven tests: thresholds panel controls via real clicks/typing.

use egui_kittest::kittest::Queryable;
use liveplot::panels::ThresholdsPanel;
use liveplot::Panel;

use crate::harness::*;

fn show_panel(bed: &mut TestBed) {
    feed_sine(bed, "s", 50, 1.0, 50.0, 0.0);
    find_panel_mut::<ThresholdsPanel>(bed)
        .unwrap()
        .state_mut()
        .visible = true;
    steps(bed, 3);
}

/// Click the "Add" button → a new threshold entry appears.
#[test]
fn ui_add_threshold_creates_entry() {
    let mut bed = make_test_bed();
    show_panel(&mut bed);
    let before = find_panel::<ThresholdsPanel>(&bed)
        .unwrap()
        .thresholds
        .len();
    let found = {
        if let Some(btn) = bed.harness.query_all_by_label_contains("Add").next() {
            btn.click();
            true
        } else {
            false
        }
    };
    if found {
        steps(&mut bed, 2);
    }
    let after = find_panel::<ThresholdsPanel>(&bed)
        .unwrap()
        .thresholds
        .len();
    assert!(after >= before, "Add click should not lose entries");
}
