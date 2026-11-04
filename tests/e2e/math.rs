//! CAT-MATH — math traces computed inside the live app.

use liveplot::data::math::{MathKind, MathTrace};
use liveplot::panels::MathPanel;
use liveplot::TraceRef;

use crate::harness::*;

fn math_points(bed: &TestBed, name: &str) -> Vec<[f64; 2]> {
    bed.harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == name)
        .map(|(_, t)| t.live.iter().copied().collect())
        .unwrap_or_default()
}

#[test]
fn add_math_trace_computes_sum() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0);
    feed_spec(
        &bed,
        "b",
        &crate::signals::SignalSpec::Constant { v: 2.0 },
        100,
        100.0,
        0.0,
    );
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![MathTrace::new(
            TraceRef("sum".into()),
            MathKind::Add {
                inputs: vec![(TraceRef("a".into()), 1.0), (TraceRef("b".into()), 1.0)],
            },
        )]);
    }
    steps(&mut bed, 4);
    let out = math_points(&bed, "sum");
    assert!(!out.is_empty(), "math trace produced no output");
    // sum = sin(2π·1·t) + 2 → check midpoint value roughly.
    for p in out.iter().take(10) {
        let t = p[0];
        let expect = (2.0 * std::f64::consts::PI * t).sin() + 2.0;
        assert!(
            (p[1] - expect).abs() < 0.05,
            "sum mismatch at t={t}: {} vs {expect}",
            p[1]
        );
    }
}

#[test]
fn multiply_and_divide() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "two",
        &crate::signals::SignalSpec::Constant { v: 2.0 },
        50,
        50.0,
        0.0,
    );
    feed_spec(
        &bed,
        "three",
        &crate::signals::SignalSpec::Constant { v: 3.0 },
        50,
        50.0,
        0.0,
    );
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![
            MathTrace::new(
                TraceRef("prod".into()),
                MathKind::Multiply {
                    a: TraceRef("two".into()),
                    b: TraceRef("three".into()),
                },
            ),
            MathTrace::new(
                TraceRef("quot".into()),
                MathKind::Divide {
                    a: TraceRef("three".into()),
                    b: TraceRef("two".into()),
                },
            ),
        ]);
    }
    steps(&mut bed, 4);
    assert!(math_points(&bed, "prod")
        .iter()
        .all(|p| (p[1] - 6.0).abs() < 1e-9));
    assert!(math_points(&bed, "quot")
        .iter()
        .all(|p| (p[1] - 1.5).abs() < 1e-9));
}

#[test]
fn differentiate_ramp_yields_slope() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "ramp",
        &crate::signals::SignalSpec::Ramp {
            slope: 5.0,
            offset: 0.0,
        },
        200,
        100.0,
        0.0,
    );
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![MathTrace::new(
            TraceRef("dramp".into()),
            MathKind::Differentiate {
                input: TraceRef("ramp".into()),
            },
        )]);
    }
    steps(&mut bed, 4);
    let out = math_points(&bed, "dramp");
    assert!(out.len() > 10);
    let tail = &out[out.len() - 50..];
    assert!(
        tail.iter().all(|p| (p[1] - 5.0).abs() < 0.1),
        "derivative should approach 5.0, got {:?}",
        tail.last()
    );
}

#[test]
fn formula_pure_t() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 100, 1.0, 100.0, 0.0); // any trace so panel updates
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![MathTrace::new(
            TraceRef("f".into()),
            MathKind::Formula {
                expr: "2*t".to_string(),
            },
        )]);
    }
    steps(&mut bed, 4);
    let out = math_points(&bed, "f");
    assert!(
        out.iter().all(|p| (p[1] - 2.0 * p[0]).abs() < 1e-6),
        "formula 2*t wrong: {:?}",
        out.first()
    );
}
