//! CAT-MATH — filters, min/max, formula edge cases.

use liveplot::data::math::{FilterKind, MathKind, MathTrace, MinMaxMode};
use liveplot::panels::MathPanel;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::SignalSpec;

fn add(bed: &mut TestBed, name: &str, kind: MathKind) {
    find_panel_mut::<MathPanel>(bed)
        .unwrap()
        .set_math_traces(vec![MathTrace::new(TraceRef(name.into()), kind)]);
}

fn pts(bed: &TestBed, name: &str) -> Vec<[f64; 2]> {
    bed.harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == name)
        .map(|(_, t)| t.live.iter().copied().collect())
        .unwrap_or_default()
}

#[test]
fn integrate_constant_grows_linearly() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "c", &SignalSpec::Constant { v: 2.0 }, 200, 100.0, 0.0);
    add(
        &mut bed,
        "integ",
        MathKind::Integrate {
            input: TraceRef("c".into()),
            y0: 0.0,
        },
    );
    steps(&mut bed, 5);
    let out = pts(&bed, "integ");
    assert!(out.len() > 50, "integrate produced no output");
    // ∫2 dt over ~2s → last ≈ 2·t_end.
    let (t, y) = (out.last().unwrap()[0], out.last().unwrap()[1]);
    assert!(
        (y - 2.0 * t).abs() < 0.2,
        "integral of 2 over t={t} should be ~{} got {y}",
        2.0 * t
    );
}

#[test]
fn minmax_tracks_extrema() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "s",
        &SignalSpec::Step {
            levels: vec![0.0, 3.0, -2.0, 1.0],
            per: 25,
        },
        100,
        100.0,
        0.0,
    );
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![
            MathTrace::new(
                TraceRef("mx".into()),
                MathKind::MinMax {
                    input: TraceRef("s".into()),
                    decay_per_sec: None,
                    mode: MinMaxMode::Max,
                },
            ),
            MathTrace::new(
                TraceRef("mn".into()),
                MathKind::MinMax {
                    input: TraceRef("s".into()),
                    decay_per_sec: None,
                    mode: MinMaxMode::Min,
                },
            ),
        ]);
    }
    steps(&mut bed, 5);
    assert_eq!(pts(&bed, "mx").last().unwrap()[1], 3.0);
    assert_eq!(pts(&bed, "mn").last().unwrap()[1], -2.0);
}

#[test]
fn lowpass_smooths_step() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "step",
        &SignalSpec::Step {
            levels: vec![0.0, 1.0],
            per: 100,
        },
        200,
        200.0,
        0.0,
    );
    add(
        &mut bed,
        "lp",
        MathKind::Filter {
            input: TraceRef("step".into()),
            kind: FilterKind::Lowpass { cutoff_hz: 1.0 },
        },
    );
    steps(&mut bed, 5);
    let out = pts(&bed, "lp");
    assert!(out.len() > 100);
    // Never overshoots the 1.0 step; converges toward it.
    assert!(out.iter().all(|p| p[1] <= 1.0 + 1e-6));
    assert!(out.last().unwrap()[1] > 0.5, "lowpass should converge");
}

#[test]
fn highpass_kills_dc() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "dc",
        &SignalSpec::Constant { v: 5.0 },
        200,
        100.0,
        0.0,
    );
    add(
        &mut bed,
        "hp",
        MathKind::Filter {
            input: TraceRef("dc".into()),
            kind: FilterKind::Highpass { cutoff_hz: 1.0 },
        },
    );
    steps(&mut bed, 5);
    let out = pts(&bed, "hp");
    assert!(
        out.last().is_some_and(|p| p[1].abs() < 0.5),
        "highpass of DC should decay to 0, got {:?}",
        out.last()
    );
}

#[test]
fn formula_with_trace_refs() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "a", &SignalSpec::Constant { v: 4.0 }, 50, 50.0, 0.0);
    add(
        &mut bed,
        "f",
        MathKind::Formula {
            expr: "{a} * 2 + 1".into(),
        },
    );
    steps(&mut bed, 5);
    let out = pts(&bed, "f");
    assert!(
        out.iter().all(|p| (p[1] - 9.0).abs() < 1e-9),
        "formula {{a}}*2+1 should yield 9"
    );
}
