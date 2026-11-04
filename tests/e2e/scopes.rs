//! CAT-SCP / CAT-XY — multi-scope management and XY plotting.

use liveplot::data::scope::ScopeType;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::SignalSpec;

#[test]
fn add_and_remove_scope() {
    let mut bed = make_test_bed();
    assert_eq!(scopes(&bed).len(), 1);
    bed.harness.state_mut().liveplot_panel.add_scope();
    steps(&mut bed, 3);
    assert_eq!(scopes(&bed).len(), 2);
    let id = scope(&bed, 1).id;
    assert!(bed
        .harness
        .state_mut()
        .liveplot_panel
        .remove_scope_by_id(id));
    steps(&mut bed, 3);
    assert_eq!(scopes(&bed).len(), 1);
}

#[test]
fn xy_scope_pairs_two_traces() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "xs",
        &SignalSpec::Custom(std::sync::Arc::new(|i, _| i as f64)),
        100,
        100.0,
        0.0,
    );
    feed_spec(&bed, "ys", &SignalSpec::sine(1.0), 100, 100.0, 0.0);
    {
        let s = scope_mut(&mut bed, 0);
        s.scope_type = ScopeType::XYScope;
        s.xy_pairs.push((
            Some(TraceRef("xs".into())),
            Some(TraceRef("ys".into())),
            Default::default(),
        ));
    }
    steps(&mut bed, 4);
    // No panic, scope rendered — legend includes the "ys vs xs" pair entry.
    let legend = bed.harness.state().liveplot_panel.scope_panels()[0].legend_overlay_rect();
    assert!(legend.is_some(), "XY scope should show a legend entry");
}

#[test]
fn scopes_have_independent_state() {
    let mut bed = make_test_bed();
    bed.harness.state_mut().liveplot_panel.add_scope();
    steps(&mut bed, 2);
    {
        let mut data = bed.harness.state_mut().liveplot_panel.get_data_mut();
        data[1].paused = true;
        data[1].show_legend = false;
    }
    steps(&mut bed, 2);
    let data = scopes(&bed);
    assert!(!data[0].paused && data[1].paused);
}
