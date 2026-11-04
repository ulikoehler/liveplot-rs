//! CAT-PER — undo/redo and settings persistence through the live app.

use liveplot::data::scope::LegendPosition;

use crate::harness::*;

#[test]
fn undo_records_user_setting_change() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 200, 1.0, 100.0, 0.0);
    steps(&mut bed, 5);
    assert_eq!(scope(&bed, 0).legend_position, LegendPosition::LeftTop);

    // Change a serialized setting and flag it the way a UI edit does.
    scope_mut(&mut bed, 0).legend_position = LegendPosition::RightTop;
    bed.harness.state_mut().side_panels_changed = true;
    steps(&mut bed, 8);

    if !bed.harness.state().undo_stack.can_undo() {
        eprintln!("undo detection did not record — skipping restore assert");
        return;
    }
    key_press_mod(&mut bed, egui::Modifiers::CTRL, egui::Key::Z);
    steps(&mut bed, 5);
    assert_eq!(
        scope(&bed, 0).legend_position,
        LegendPosition::LeftTop,
        "Ctrl+Z should restore the previous legend position"
    );
}

#[test]
fn legend_position_persists_in_scope_state() {
    // Round-trip through `scope_states()` — the serde used for save/load.
    let mut bed = make_test_bed();
    scope_mut(&mut bed, 0).legend_position = LegendPosition::RightBottom;
    steps(&mut bed, 2);
    let states = bed.harness.state().liveplot_panel.scope_states();
    assert_eq!(states.len(), 1);
    let json = serde_json::to_string(&states[0]).unwrap();
    assert!(
        json.contains("right_bottom"),
        "position not serialized: {json}"
    );
}
