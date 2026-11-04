//! CAT-SINK — coverage of every `PlotCommand` through the live channel.

use liveplot::PlotPoint;

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

fn tr_len(bed: &TestBed, name: &str) -> usize {
    bed.harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == name)
        .map(|(_, t)| t.live.len())
        .unwrap_or(usize::MAX)
}

fn get(bed: &TestBed, name: &str) -> Vec<[f64; 2]> {
    bed.harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == name)
        .map(|(_, t)| t.live.iter().copied().collect())
        .unwrap_or_default()
}

#[test]
fn point_and_chunk_ingestion() {
    let mut bed = make_test_bed();
    let t = bed.sink.create_trace("a", Some("info-a"));
    bed.sink
        .send_point(&t, PlotPoint { x: 1.0, y: 2.0 })
        .unwrap();
    bed.sink
        .send_points(&t, gen(&SignalSpec::sine(1.0), 10, 10.0, 2.0))
        .unwrap();
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 11);
    assert_eq!(
        bed.harness
            .state()
            .traces_data
            .traces_iter()
            .find(|(n, _)| n.0 == "a")
            .map(|(_, t)| t.info.as_str()),
        Some("info-a")
    );
}

#[test]
fn set_data_replaces_buffer() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 100);
    let t = bed.sink.create_trace("a", None);
    bed.sink
        .set_data(&t, gen(&SignalSpec::Constant { v: 7.0 }, 5, 1.0, 0.0))
        .unwrap();
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 5);
    assert!(get(&bed, "a").iter().all(|p| p[1] == 7.0));
}

#[test]
fn clear_and_delete_commands() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let t = bed.sink.create_trace("a", None);

    // DeletePointsX
    bed.sink.delete_point_x(&t, 0.5).unwrap();
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 9);

    // DeleteXRange [0.2, 0.4]
    bed.sink.delete_x_range(&t, 0.2, 0.4).unwrap();
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 6);

    // SetPointsY
    bed.sink.set_point_y(&t, 0.0, 42.0).unwrap();
    steps(&mut bed, 2);
    assert_eq!(get(&bed, "a")[0], [0.0, 42.0]);

    // ApplyYFnAtX
    bed.sink
        .apply_y_fn_at_x(&t, 0.0, Box::new(|y| y + 1.0))
        .unwrap();
    steps(&mut bed, 2);
    assert_eq!(get(&bed, "a")[0], [0.0, 43.0]);

    // ClearData
    bed.sink.clear_data(&t).unwrap();
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 0);
}

#[test]
fn max_points_eviction() {
    let mut bed = make_test_bed();
    bed.harness.state_mut().traces_data.max_points = 100;
    feed_sine(&bed, "a", 250, 1.0, 100.0, 0.0);
    steps(&mut bed, 2);
    assert_eq!(tr_len(&bed, "a"), 100);
    let pts = get(&bed, "a");
    // Newest 100 of 250: last x must be the newest.
    assert!((pts.last().unwrap()[0] - 249.0 / 100.0).abs() < 1e-9);
}

#[test]
fn set_trace_info_updates_label() {
    let mut bed = make_test_bed();
    let t = bed.sink.create_trace("a", None);
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    bed.sink.set_trace_info(&t, "hello".into());
    steps(&mut bed, 2);
    let info = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "a")
        .map(|(_, t)| t.info.clone());
    assert_eq!(info.as_deref(), Some("hello"));
}

#[test]
fn interleaved_multi_trace_feed() {
    let mut bed = make_test_bed();
    let traces: Vec<_> = (0..8)
        .map(|i| bed.sink.create_trace(format!("t{i}"), None))
        .collect();
    for i in 0..100usize {
        for (j, t) in traces.iter().enumerate() {
            bed.sink
                .send_point(
                    t,
                    PlotPoint {
                        x: i as f64,
                        y: j as f64,
                    },
                )
                .unwrap();
        }
    }
    steps(&mut bed, 2);
    for j in 0..8 {
        assert_eq!(tr_len(&bed, &format!("t{j}")), 100);
        assert!(get(&bed, &format!("t{j}")).iter().all(|p| p[1] == j as f64));
    }
}
