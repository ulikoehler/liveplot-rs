//! CAT-SINK — edge cases: unknown-x ops, out-of-order, far timestamps.

use liveplot::PlotPoint;

use crate::harness::*;

fn get(bed: &TestBed, name: &str) -> Vec<[f64; 2]> {
    bed.harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == name)
        .map(|(_, t)| t.live.iter().copied().collect())
        .unwrap_or_default()
}

/// SINK-06: SetPointsY on a missing x is a graceful no-op.
#[test]
fn set_y_unknown_x_noop() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let t = bed.sink.create_trace("a", None);
    bed.sink.set_point_y(&t, 999.0, 1.0).unwrap();
    steps(&mut bed, 2);
    assert_eq!(get(&bed, "a").len(), 10, "no-op must not modify buffer");
}

/// SINK-08: DeleteXRange with NaN bounds removes to the ends.
#[test]
fn delete_x_range_nan_bounds() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 20, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let t = bed.sink.create_trace("a", None);
    // NaN start → delete everything up to x=0.5.
    bed.sink.delete_x_range(&t, f64::NAN, 0.5).unwrap();
    steps(&mut bed, 2);
    let pts = get(&bed, "a");
    assert!(
        pts.iter().all(|p| p[0] > 0.5),
        "NaN bound should delete to the end, got {pts:?}"
    );
}

/// SINK-17: points far in past/future don't break bounds.
#[test]
fn extreme_timestamps_ok() {
    let mut bed = make_test_bed();
    let t = bed.sink.create_trace("a", None);
    bed.sink
        .send_points(
            &t,
            vec![
                PlotPoint { x: -1e9, y: 0.0 },
                PlotPoint { x: 0.0, y: 1.0 },
                PlotPoint { x: 1e9, y: 0.0 },
            ],
        )
        .unwrap();
    steps(&mut bed, 3);
    let s = scope(&bed, 0);
    assert!(s.rendered_this_frame);
    assert!(s.last_plot_bounds.is_some());
}

/// Duplicate-x feeding doesn't panic and keeps a sane buffer.
#[test]
fn duplicate_x_points_ok() {
    let mut bed = make_test_bed();
    let t = bed.sink.create_trace("a", None);
    for i in 0..20 {
        bed.sink
            .send_point(
                &t,
                PlotPoint {
                    x: (i % 5) as f64,
                    y: i as f64,
                },
            )
            .unwrap();
    }
    steps(&mut bed, 3);
    assert_eq!(get(&bed, "a").len(), 20);
}
