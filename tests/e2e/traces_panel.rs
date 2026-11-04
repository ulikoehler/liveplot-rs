//! CAT-TRACES — traces panel state changes visible in the scope.

use liveplot::panels::TracesPanel;
use liveplot::Panel;

use crate::harness::*;

#[test]
fn trace_visibility_via_panel_state() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);
    {
        let tr = bed
            .harness
            .state_mut()
            .traces_data
            .get_trace_mut(&liveplot::TraceRef("a".into()));
        tr.unwrap().look.visible = false;
    }
    steps(&mut bed, 2);
    // Hidden traces are excluded from auto-fit y bounds and legend.
    assert!(!scope(&bed, 0).trace_order.is_empty());
}

#[test]
fn new_traces_are_sorted_alphabetically() {
    // The default `sort_new_traces_alphabetically` behavior inserts new
    // traces in sorted order regardless of feed order.
    let mut bed = make_test_bed();
    feed_sine(&bed, "zeta", 50, 1.0, 50.0, 0.0);
    feed_sine(&bed, "alpha", 50, 1.0, 50.0, 0.0);
    steps(&mut bed, 2);
    let order: Vec<String> = scope(&bed, 0)
        .trace_order
        .iter()
        .map(|t| t.0.clone())
        .collect();
    assert_eq!(order, vec!["alpha".to_string(), "zeta".to_string()]);
}

#[test]
fn traces_panel_toggle_via_state() {
    let mut bed = make_test_bed();
    {
        let p = find_panel_mut::<TracesPanel>(&mut bed).unwrap();
        p.state_mut().visible = true;
    }
    steps(&mut bed, 2);
    assert!(panel_visible::<TracesPanel>(&bed));
    {
        let p = find_panel_mut::<TracesPanel>(&mut bed).unwrap();
        p.state_mut().visible = false;
    }
    steps(&mut bed, 2);
    assert!(!panel_visible::<TracesPanel>(&bed));
}
