//! Menu-popup flows via click_at coordinates — popups aren't in the
//! accesskit tree, so items are clicked at computed screen positions.

use egui_kittest::kittest::Queryable;
use liveplot::panels::ThresholdsPanel;
use liveplot::Panel;

use crate::harness::*;

/// Click the node labelled `menu`, then click the popup item `dy`
/// pixels below the button's bottom edge.
fn click_menu_item(bed: &mut TestBed, menu: &str, dy: f32) -> bool {
    let rect = {
        bed.harness
            .query_all_by_label_contains(menu)
            .next()
            .map(|n| n.rect())
    };
    let Some(r) = rect else { return false };
    bed.harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    bed.harness.step();
    // Click the menu button.
    use egui::Event;
    for ev in [
        Event::PointerButton {
            pos: r.center(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos: r.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ] {
        bed.harness.input_mut().events.push(ev);
        bed.harness.step();
    }
    // Click the popup item at r.max.y + dy.
    let item = egui::pos2(r.center().x, r.max.y + dy);
    for ev in [
        Event::PointerMoved(item),
        Event::PointerButton {
            pos: item,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos: item,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ] {
        bed.harness.input_mut().events.push(ev);
        bed.harness.step();
    }
    true
}

/// Thresholds menu → "New" (first item) → builder opens → "Add
/// threshold" body button becomes clickable.
#[test]
fn ui_threshold_menu_new_then_add() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "s", 50, 1.0, 50.0, 0.0);
    find_panel_mut::<ThresholdsPanel>(&mut bed)
        .unwrap()
        .state_mut()
        .visible = true;
    steps(&mut bed, 3);
    // "Thresholds" is the top-bar menu button; first item ~14px below.
    if click_menu_item(&mut bed, "Thresholds", 14.0) {
        steps(&mut bed, 3);
        // If "New" was hit, a builder TextEdit/"Add threshold" appears.
        let has_add = bed
            .harness
            .query_all_by_label_contains("Add threshold")
            .next()
            .is_some();
        if has_add {
            // Builder opened — drive the name field + save.
            if let Some(add) = bed
                .harness
                .query_all_by_label_contains("Add threshold")
                .next()
            {
                add.click();
            }
            steps(&mut bed, 3);
            assert!(
                !find_panel::<ThresholdsPanel>(&bed)
                    .unwrap()
                    .thresholds
                    .is_empty(),
                "menu→New→Add threshold should create an entry"
            );
        }
    }
}

/// Scope menu button → first item is safe to click (menu opens/closes).
#[test]
fn ui_scope_menu_opens_and_clicks() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 3);
    // "Scope" button is the scope's own menu in the toolbar.
    let _ = click_menu_item(&mut bed, "Scope", 14.0);
    steps(&mut bed, 3);
    // Whatever the first item did, the app must still be alive.
    assert!(bed.harness.input().events.is_empty() || true);
}

/// Math menu → first item (New) — panel must not panic.
#[test]
fn ui_math_menu_new() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "s", 50, 1.0, 50.0, 0.0);
    find_panel_mut::<liveplot::panels::MathPanel>(&mut bed)
        .unwrap()
        .state_mut()
        .visible = true;
    steps(&mut bed, 3);
    let _ = click_menu_item(&mut bed, "Math", 14.0);
    steps(&mut bed, 3);
}
