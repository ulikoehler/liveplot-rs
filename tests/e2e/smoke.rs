use egui_kittest::kittest::Queryable;

use crate::harness::{feed_sine, make_test_bed};

#[test]
fn app_starts_and_renders() {
    let mut bed = make_test_bed();
    bed.harness.step();
    // The accesskit tree should contain at least some nodes.
    let tree = bed.harness.query_all_by(|n| n.label().is_some()).count();
    assert!(tree > 10, "expected a populated UI tree, got {tree} nodes");
}

#[test]
fn trace_is_ingested_into_scope() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "sig_a", 1000, 3.0, 1000.0, 1000.0);
    bed.harness.step();
    let panel = bed.harness.state();
    assert!(panel
        .traces_data
        .traces_iter()
        .any(|(n, t)| n.0 == "sig_a" && t.live.len() == 1000));
}
