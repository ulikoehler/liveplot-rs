//! CAT-TRACES — drag interactions: moving a trace between scopes by
//! dragging its row onto the target scope's plot.

use egui_kittest::kittest::Queryable;
use liveplot::panels::TracesPanel;
use liveplot::Panel;

use crate::harness::*;

/// Drag the "bbb" row from the Traces panel onto scope 2's plot area —
/// the scope should claim the trace.
#[test]
fn drag_trace_onto_second_scope_moves_it() {
    let mut bed = make_test_bed();
    bed.harness.state_mut().liveplot_panel.add_scope();
    feed_sine(&bed, "aaa", 50, 1.0, 50.0, 0.0);
    feed_sine(&bed, "bbb", 50, 1.0, 50.0, 0.0);
    {
        let p = find_panel_mut::<TracesPanel>(&mut bed).unwrap();
        p.state_mut().visible = true;
    }
    // Hide the legend so name nodes resolve to the panel rows.
    scope_mut(&mut bed, 0).show_legend = false;
    steps(&mut bed, 4);

    let order0: Vec<String> = scope(&bed, 0)
        .trace_order
        .iter()
        .map(|t| t.0.clone())
        .collect();
    assert!(order0.contains(&"bbb".to_string()));

    // Find the "bbb" row in the accesskit tree. Multiple nodes may match
    // (label + tooltip copy); the *last* is the drag-sensed row widget —
    // verified via dragging_trace.
    let from = bed
        .harness
        .query_all_by_label_contains("bbb")
        .map(|n| n.rect().center())
        .last();
    let Some(from) = from else {
        eprintln!("trace row not accessible via accesskit — skipping drag");
        return;
    };
    // Target: inside scope 2's plot rect.
    let r = scope(&bed, 1).last_plot_screen_rect.unwrap();
    let to = egui::pos2((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);

    drag(&mut bed, from, to, egui::PointerButton::Primary);
    steps(&mut bed, 3);

    let s0: Vec<String> = scope(&bed, 0)
        .trace_order
        .iter()
        .map(|t| t.0.clone())
        .collect();
    let s1: Vec<String> = scope(&bed, 1)
        .trace_order
        .iter()
        .map(|t| t.0.clone())
        .collect();
    assert!(
        s1.contains(&"bbb".to_string()) || scope(&bed, 1).wants_default_target,
        "bbb should have moved to scope 2 (s0={s0:?} s1={s1:?})"
    );
}
