use liveplot::data::math::{MathKind, MathTrace, MinMaxMode};
use liveplot::data::traces::TraceRef;
use std::collections::HashMap;

fn make_sources(pairs: &[(&str, Vec<[f64; 2]>)]) -> HashMap<TraceRef, Vec<[f64; 2]>> {
    let mut m = HashMap::new();
    for (name, data) in pairs {
        m.insert(TraceRef::new(*name), data.clone());
    }
    m
}

#[test]
fn test_add_incremental_preserves_old_points() {
    let mut trace = MathTrace::new(
        TraceRef::new("sum"),
        MathKind::Add {
            inputs: vec![(TraceRef::new("a"), 1.0), (TraceRef::new("b"), 1.0)],
        },
    );

    // Initial data
    let sources = make_sources(&[
        ("a", vec![[0.0, 1.0], [1.0, 2.0], [2.0, 3.0]]),
        ("b", vec![[0.0, 10.0], [1.0, 20.0], [2.0, 30.0]]),
        ("sum", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 3);
    assert_eq!(out1[0], [0.0, 11.0]);
    assert_eq!(out1[1], [1.0, 22.0]);
    assert_eq!(out1[2], [2.0, 33.0]);

    // New data arrives — only append new points to sources
    let sources2 = make_sources(&[
        (
            "a",
            vec![[0.0, 1.0], [1.0, 2.0], [2.0, 3.0], [3.0, 4.0], [4.0, 5.0]],
        ),
        (
            "b",
            vec![
                [0.0, 10.0],
                [1.0, 20.0],
                [2.0, 30.0],
                [3.0, 40.0],
                [4.0, 50.0],
            ],
        ),
        ("sum", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);

    // Old points must be preserved exactly
    assert_eq!(out2.len(), 5);
    assert_eq!(out2[0], [0.0, 11.0]);
    assert_eq!(out2[1], [1.0, 22.0]);
    assert_eq!(out2[2], [2.0, 33.0]);
    // New points computed correctly
    assert_eq!(out2[3], [3.0, 44.0]);
    assert_eq!(out2[4], [4.0, 55.0]);
}

#[test]
fn test_multiply_incremental() {
    let mut trace = MathTrace::new(
        TraceRef::new("prod"),
        MathKind::Multiply {
            a: TraceRef::new("x"),
            b: TraceRef::new("y"),
        },
    );

    let sources = make_sources(&[
        ("x", vec![[0.0, 2.0], [1.0, 3.0]]),
        ("y", vec![[0.0, 4.0], [1.0, 5.0]]),
        ("prod", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 2);
    assert_eq!(out1[0], [0.0, 8.0]);
    assert_eq!(out1[1], [1.0, 15.0]);

    let sources2 = make_sources(&[
        ("x", vec![[0.0, 2.0], [1.0, 3.0], [2.0, 6.0]]),
        ("y", vec![[0.0, 4.0], [1.0, 5.0], [2.0, 7.0]]),
        ("prod", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);
    assert_eq!(out2.len(), 3);
    assert_eq!(out2[0], [0.0, 8.0]);
    assert_eq!(out2[1], [1.0, 15.0]);
    assert_eq!(out2[2], [2.0, 42.0]);
}

#[test]
fn test_differentiate_incremental() {
    let mut trace = MathTrace::new(
        TraceRef::new("deriv"),
        MathKind::Differentiate {
            input: TraceRef::new("sig"),
        },
    );

    let sources = make_sources(&[
        ("sig", vec![[0.0, 0.0], [1.0, 1.0], [2.0, 4.0]]),
        ("deriv", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 2);
    assert_eq!(out1[0], [1.0, 1.0]);
    assert_eq!(out1[1], [2.0, 3.0]);

    let sources2 = make_sources(&[
        ("sig", vec![[0.0, 0.0], [1.0, 1.0], [2.0, 4.0], [3.0, 9.0]]),
        ("deriv", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);
    assert_eq!(out2.len(), 3);
    assert_eq!(out2[0], [1.0, 1.0]);
    assert_eq!(out2[1], [2.0, 3.0]);
    assert_eq!(out2[2], [3.0, 5.0]);
}

#[test]
fn test_integrate_incremental() {
    let mut trace = MathTrace::new(
        TraceRef::new("integral"),
        MathKind::Integrate {
            input: TraceRef::new("sig"),
            y0: 0.0,
        },
    );

    let sources = make_sources(&[
        ("sig", vec![[0.0, 1.0], [1.0, 1.0], [2.0, 1.0]]),
        ("integral", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 3);
    // Trapezoidal: first sample = y0 = 0, then +0.5*(1+1)*1=1, then +0.5*(1+1)*1=2
    assert_eq!(out1[0], [0.0, 0.0]);
    assert!((out1[1][1] - 1.0).abs() < 1e-9);
    assert!((out1[2][1] - 2.0).abs() < 1e-9);

    let sources2 = make_sources(&[
        ("sig", vec![[0.0, 1.0], [1.0, 1.0], [2.0, 1.0], [3.0, 1.0]]),
        ("integral", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);
    assert_eq!(out2.len(), 4);
    // Old values preserved
    assert_eq!(out2[0], [0.0, 0.0]);
    assert!((out2[1][1] - 1.0).abs() < 1e-9);
    assert!((out2[2][1] - 2.0).abs() < 1e-9);
    // New value: 2.0 + 0.5*(1+1)*1 = 3.0
    assert!((out2[3][1] - 3.0).abs() < 1e-9);
}

#[test]
fn test_minmax_incremental() {
    let mut trace = MathTrace::new(
        TraceRef::new("maxtrace"),
        MathKind::MinMax {
            input: TraceRef::new("sig"),
            decay_per_sec: None,
            mode: MinMaxMode::Max,
        },
    );

    let sources = make_sources(&[
        ("sig", vec![[0.0, 3.0], [1.0, 7.0], [2.0, 5.0]]),
        ("maxtrace", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 3);
    assert_eq!(out1[0], [0.0, 3.0]);
    assert_eq!(out1[1], [1.0, 7.0]);
    assert_eq!(out1[2], [2.0, 7.0]);

    let sources2 = make_sources(&[
        (
            "sig",
            vec![[0.0, 3.0], [1.0, 7.0], [2.0, 5.0], [3.0, 9.0], [4.0, 2.0]],
        ),
        ("maxtrace", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);
    assert_eq!(out2.len(), 5);
    assert_eq!(out2[0], [0.0, 3.0]);
    assert_eq!(out2[1], [1.0, 7.0]);
    assert_eq!(out2[2], [2.0, 7.0]);
    assert_eq!(out2[3], [3.0, 9.0]);
    assert_eq!(out2[4], [4.0, 9.0]);
}

#[test]
fn test_divide_incremental() {
    let mut trace = MathTrace::new(
        TraceRef::new("ratio"),
        MathKind::Divide {
            a: TraceRef::new("num"),
            b: TraceRef::new("den"),
        },
    );

    let sources = make_sources(&[
        ("num", vec![[0.0, 10.0], [1.0, 20.0]]),
        ("den", vec![[0.0, 2.0], [1.0, 4.0]]),
        ("ratio", vec![]),
    ]);
    let out1 = trace.compute_math_trace(&sources);
    assert_eq!(out1.len(), 2);
    assert_eq!(out1[0], [0.0, 5.0]);
    assert_eq!(out1[1], [1.0, 5.0]);

    let sources2 = make_sources(&[
        ("num", vec![[0.0, 10.0], [1.0, 20.0], [2.0, 30.0]]),
        ("den", vec![[0.0, 2.0], [1.0, 4.0], [2.0, 6.0]]),
        ("ratio", out1.clone()),
    ]);
    let out2 = trace.compute_math_trace(&sources2);
    assert_eq!(out2.len(), 3);
    assert_eq!(out2[0], [0.0, 5.0]);
    assert_eq!(out2[1], [1.0, 5.0]);
    assert_eq!(out2[2], [2.0, 5.0]);
}

#[test]
fn test_no_math_traces_no_crash() {
    // Verify that compute_math_trace with empty inputs doesn't panic
    let mut trace = MathTrace::new(TraceRef::new("empty"), MathKind::Add { inputs: vec![] });
    let sources = make_sources(&[("empty", vec![])]);
    let out = trace.compute_math_trace(&sources);
    assert!(out.is_empty());
}

#[test]
fn test_input_trace_names() {
    let trace = MathTrace::new(
        TraceRef::new("result"),
        MathKind::Add {
            inputs: vec![(TraceRef::new("a"), 1.0), (TraceRef::new("b"), -1.0)],
        },
    );
    let names = trace.input_trace_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&TraceRef::new("a")));
    assert!(names.contains(&TraceRef::new("b")));
}

#[test]
fn test_formula_trace() {
    let mut trace = MathTrace::new(
        TraceRef::new("f"),
        MathKind::Formula {
            expr: "{a} * 2 + {b}".to_string(),
        },
    );
    let sources = make_sources(&[
        ("a", vec![[0.0, 1.0], [1.0, 2.0], [2.0, 3.0]]),
        ("b", vec![[0.0, 10.0], [2.0, 30.0]]),
        ("f", vec![]),
    ]);
    let out = trace.compute_math_trace(&sources);
    // Union grid of a and b: t = 0,1,2; b interpolates to 20 at t=1.
    assert_eq!(out, vec![[0.0, 12.0], [1.0, 24.0], [2.0, 36.0]]);
}

#[test]
fn test_formula_time_only() {
    // Pure f(t) formula: evaluates on the union of all source timestamps.
    let mut trace = MathTrace::new(
        TraceRef::new("ramp"),
        MathKind::Formula {
            expr: "2 * t".to_string(),
        },
    );
    let sources = make_sources(&[
        ("a", vec![[0.0, 1.0], [1.0, 2.0], [2.0, 3.0]]),
        ("ramp", vec![]),
    ]);
    let out = trace.compute_math_trace(&sources);
    assert_eq!(out, vec![[0.0, 0.0], [1.0, 2.0], [2.0, 4.0]]);
}

#[test]
fn test_formula_invalid_keeps_old_output() {
    let mut trace = MathTrace::new(
        TraceRef::new("bad"),
        MathKind::Formula {
            expr: "sqrt(".to_string(),
        },
    );
    let sources = make_sources(&[("a", vec![[0.0, 1.0]]), ("bad", vec![[5.0, 9.0]])]);
    let out = trace.compute_math_trace(&sources);
    assert_eq!(out, vec![[5.0, 9.0]]);
}

#[test]
fn test_formula_input_trace_names() {
    let trace = MathTrace::new(
        TraceRef::new("f"),
        MathKind::Formula {
            expr: "{sig 1} + t * sin({sig.2}) + {sig 1}".to_string(),
        },
    );
    let names = trace.input_trace_names();
    assert_eq!(names, vec![TraceRef::new("sig 1"), TraceRef::new("sig.2")]);
}

#[test]
fn test_formula_serde_roundtrip() {
    let trace = MathTrace::new(
        TraceRef::new("f"),
        MathKind::Formula {
            expr: "log({a}, 2) ^ 2".to_string(),
        },
    );
    let s = serde_json::to_string(&trace).unwrap();
    let back: MathTrace = serde_json::from_str(&s).unwrap();
    assert!(matches!(back.kind, MathKind::Formula { .. }));
}

#[test]
fn test_set_live_points_updates_render_caches() {
    use liveplot::data::traces::TraceData;

    let mut tr = TraceData::default();
    for i in 0..100 {
        tr.live.push_back([i as f64, i as f64]);
    }
    // Build the render caches like a zoomed-out render would (len > screen
    // width / max_pts).
    let live = tr.live.clone();
    tr.recompute_envelope_from(&live, 10, 100.0, Some((0.0, 100.0)));
    tr.recompute_decimation_from(false, 10);
    assert!(!tr.envelope_needs_recompute(10, 100.0, (0.0, 100.0)));

    // A math-trace update is a pure tail extension — the caches must be
    // updated incrementally, otherwise the drawn trace freezes once the
    // point count exceeds the screen width.
    let mut next: Vec<[f64; 2]> = tr.live.iter().copied().collect();
    next.push([100.0, 5.0]);
    next.push([101.0, 999.0]);
    tr.set_live_points(&next);

    assert_eq!(tr.live.len(), 102);
    assert!(!tr.envelope_needs_recompute(10, 100.0, (0.0, 100.0)));
    let cache = tr.envelope_cache.as_ref().unwrap();
    let total: usize = cache.buckets.iter().map(|b| b.count).sum();
    assert_eq!(total, 102);
    let last = cache.buckets.iter().rev().find(|b| b.count > 0).unwrap();
    assert_eq!(last.y_max, 999.0);
    assert_eq!(tr.decimation_cache.as_ref().unwrap().add_counter, 102);
}

#[test]
fn test_set_live_points_rewrite_invalidates_caches() {
    use liveplot::data::traces::TraceData;

    let mut tr = TraceData::default();
    for i in 0..100 {
        tr.live.push_back([i as f64, i as f64]);
    }
    let live = tr.live.clone();
    tr.recompute_envelope_from(&live, 10, 100.0, Some((0.0, 100.0)));
    tr.recompute_decimation_from(false, 10);

    // Rewritten history is not a tail extension → caches are invalidated so
    // the next render rebuilds them.
    let different: Vec<[f64; 2]> = (0..100).map(|i| [i as f64, -(i as f64)]).collect();
    tr.set_live_points(&different);

    assert!(tr.envelope_cache.is_none());
    assert!(tr.decimation_cache.is_none());
    assert!(tr.density_cache.is_none());
}

#[test]
fn test_set_snap_points_noop_when_unchanged() {
    use liveplot::data::traces::TraceData;

    let mut tr = TraceData {
        snap: Some([[0.0, 1.0], [1.0, 2.0]].into_iter().collect()),
        ..Default::default()
    };
    let mut sentinel = TraceData::default();
    for i in 0..100 {
        sentinel.live.push_back([i as f64, i as f64]);
    }
    let live = sentinel.live.clone();
    sentinel.recompute_envelope_from(&live, 10, 100.0, Some((0.0, 100.0)));
    tr.envelope_cache = sentinel.envelope_cache.take();

    // Identical rewrite → no cache invalidation (steady state while paused).
    tr.set_snap_points(&[[0.0, 1.0], [1.0, 2.0]]);
    assert!(tr.envelope_cache.is_some());

    // Changed content → caches invalidated.
    tr.set_snap_points(&[[0.0, 1.0], [1.0, 3.0]]);
    assert!(tr.envelope_cache.is_none());
}
