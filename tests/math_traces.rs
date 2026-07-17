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
        ("a", vec![[0.0, 1.0], [1.0, 2.0], [2.0, 3.0], [3.0, 4.0], [4.0, 5.0]]),
        ("b", vec![[0.0, 10.0], [1.0, 20.0], [2.0, 30.0], [3.0, 40.0], [4.0, 50.0]]),
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
        ("sig", vec![[0.0, 3.0], [1.0, 7.0], [2.0, 5.0], [3.0, 9.0], [4.0, 2.0]]),
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
    let mut trace = MathTrace::new(
        TraceRef::new("empty"),
        MathKind::Add { inputs: vec![] },
    );
    let sources = make_sources(&[("empty", vec![])]);
    let out = trace.compute_math_trace(&sources);
    assert!(out.is_empty());
}

#[test]
fn test_input_trace_names() {
    let trace = MathTrace::new(
        TraceRef::new("result"),
        MathKind::Add {
            inputs: vec![
                (TraceRef::new("a"), 1.0),
                (TraceRef::new("b"), -1.0),
            ],
        },
    );
    let names = trace.input_trace_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&&TraceRef::new("a")));
    assert!(names.contains(&&TraceRef::new("b")));
}
