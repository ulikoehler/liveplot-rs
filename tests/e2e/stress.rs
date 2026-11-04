//! §5 — signal-count and data-rate stress tests.
//! Heavy cases are `#[ignore]`d; run with
//! `LIVEPLOT_STRESS=1 cargo test --test e2e stress -- --ignored`.

use std::time::Instant;

use liveplot::PlotPoint;

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

fn trace_count(bed: &TestBed) -> usize {
    bed.harness.state().traces_data.traces_iter().count()
}

#[test]
fn many_traces_light() {
    // STR-N-01: 8 traces render without trouble.
    let mut bed = make_test_bed();
    feed_n_traces(&bed, 8, 2000, 1000.0);
    steps(&mut bed, 5);
    assert_eq!(trace_count(&bed), 8);
    let s = scope(&bed, 0);
    assert_eq!(s.trace_order.len(), 8);
}

#[test]
fn many_traces_medium() {
    // STR-N-02/03: 64 traces — legend must stay inside the plot.
    let mut bed = make_test_bed();
    feed_n_traces(&bed, 64, 1000, 1000.0);
    steps(&mut bed, 5);
    assert_eq!(trace_count(&bed), 64);
    let legend = bed.harness.state().liveplot_panel.scope_panels()[0].legend_overlay_rect();
    let rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    if let Some(l) = legend {
        assert!(
            l.height() <= (rect[3] - rect[1]) + 1.0,
            "legend {l:?} taller than plot {rect:?}"
        );
    }
}

#[test]
fn high_rate_single_trace() {
    // STR-R-04: 1M points in one chunk, max_points eviction.
    let mut bed = make_test_bed();
    bed.harness.state_mut().traces_data.max_points = 10_000;
    feed_spec(
        &bed,
        "hot",
        &SignalSpec::WhiteNoise { seed: 42, amp: 1.0 },
        1_000_000,
        1_000_000.0,
        0.0,
    );
    let t = Instant::now();
    steps(&mut bed, 3);
    assert!(
        t.elapsed().as_secs() < 30,
        "ingesting 1M points took too long"
    );
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .map(|(_, t)| t.live.len());
    assert_eq!(len, Some(10_000));
}

#[test]
fn many_point_msgs_drain_per_frame() {
    // STR-R-03-ish: thousands of individual Point commands.
    let mut bed = make_test_bed();
    bed.harness.state_mut().traces_data.max_points = 30_000;
    let t = bed.sink.create_trace("a", None);
    for i in 0..20_000 {
        bed.sink
            .send_point(
                &t,
                PlotPoint {
                    x: i as f64 / 1000.0,
                    y: (i % 100) as f64,
                },
            )
            .unwrap();
    }
    steps(&mut bed, 3);
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .map(|(_, t)| t.live.len());
    assert_eq!(len, Some(20_000));
}

#[test]
fn producer_thread_faster_than_consumer() {
    // STR-R-06: channel backlog drains when frames resume.
    let mut bed = make_test_bed();
    let sink = bed.sink.clone();
    let trace = sink.create_trace("bg", None);
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let producer = std::thread::spawn(move || {
        let mut i = 0u64;
        while stop_rx.try_recv().is_err() {
            let pts: Vec<PlotPoint> = (0..1000)
                .map(|k| {
                    let x = (i + k) as f64 / 10_000.0;
                    PlotPoint { x, y: x.sin() }
                })
                .collect();
            i += 1000;
            let _ = sink.send_points(&trace, pts);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    steps(&mut bed, 5);
    let _ = stop_tx.send(());
    producer.join().unwrap();
    // After a few steps the backlog must be fully drained (bounded buffer).
    steps(&mut bed, 5);
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .next()
        .map(|(_, t)| t.live.len())
        .unwrap_or(0);
    let max = bed.harness.state().traces_data.max_points;
    assert!(len > 0 && len <= max, "len={len} max={max}");
}

#[test]
fn four_scopes_many_traces() {
    // STR-S-01: 2×2 scopes with 4 traces each.
    let mut bed = make_test_bed();
    for _ in 0..3 {
        bed.harness.state_mut().liveplot_panel.add_scope();
    }
    feed_n_traces(&bed, 16, 500, 1000.0);
    steps(&mut bed, 5);
    assert_eq!(scopes(&bed).len(), 4);
    assert_eq!(trace_count(&bed), 16);
}

// ── Heavy / ignored ─────────────────────────────────────────────────────────

#[test]
#[ignore = "stress: 256 traces (run with LIVEPLOT_STRESS=1)"]
fn many_traces_heavy() {
    let mut bed = make_test_bed();
    feed_n_traces(&bed, 256, 500, 1000.0);
    steps(&mut bed, 5);
    assert_eq!(trace_count(&bed), 256);
}

#[test]
#[ignore = "stress: 1M pts/s aggregate"]
fn aggregate_rate_heavy() {
    let mut bed = make_test_bed();
    let traces: Vec<_> = (0..50)
        .map(|i| bed.sink.create_trace(format!("r{i}"), None))
        .collect();
    // 50 × 20k samples = 1M points total.
    for t in &traces {
        bed.sink
            .send_points(t, gen(&SignalSpec::sine(1.0), 20_000, 1000.0, 0.0))
            .unwrap();
    }
    let start = Instant::now();
    steps(&mut bed, 10);
    assert!(start.elapsed().as_secs() < 60, "aggregate ingest too slow");
    assert_eq!(trace_count(&bed), 50);
}
