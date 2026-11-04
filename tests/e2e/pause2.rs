//! CAT-PAUSE — edge cases: explicit pause, per-scope pause, snapshots.

use crate::harness::*;

/// PAUSE-09: pausing scope 1 leaves scope 0 live.
#[test]
fn per_scope_pause_independent() {
    let mut bed = make_test_bed();
    bed.harness.state_mut().liveplot_panel.add_scope();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 3);
    scope_mut(&mut bed, 1).paused = true;
    bed.harness.state_mut().traces_data.take_snapshot();
    steps(&mut bed, 3);
    let data = scopes(&bed);
    assert!(!data[0].paused && data[1].paused);
}

/// PAUSE-08: `take_explicit_pause` consumed exactly once.
#[test]
fn explicit_pause_consumed_once() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);
    // Pause via state → snapshot taken; resume → cleared.
    bed.harness.state_mut().pause_all();
    steps(&mut bed, 2);
    let snap1 = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .and_then(|(_, t)| t.snap.as_ref().map(|s| s.len()));
    assert_eq!(snap1, Some(100));
    bed.harness.state_mut().resume_all();
    steps(&mut bed, 2);
    let snap2 = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .and_then(|(_, t)| t.snap.as_ref().map(|s| s.len()));
    assert!(
        snap2.is_none() || snap2 == Some(0),
        "resume clears snapshot"
    );
}

/// Paused scope ignores new points on screen but keeps ingesting.
#[test]
fn paused_scope_still_ingests() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    bed.harness.state_mut().pause_all();
    steps(&mut bed, 2);
    feed_sine(&bed, "a", 50, 1.0, 100.0, 100.0);
    steps(&mut bed, 2);
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "a")
        .map(|(_, t)| t.live.len());
    assert_eq!(len, Some(150), "live buffer grows while paused");
}
