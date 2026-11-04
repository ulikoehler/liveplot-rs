//! §5.4 — endurance/soak tests. All `#[ignore]`d — run with
//! `LIVEPLOT_STRESS=1 cargo test --test e2e soak -- --ignored`.

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

/// SOAK-01: continuous feed over many steps — buffers stay bounded.
#[test]
#[ignore = "soak: 10k steps continuous feed"]
fn continuous_feed_soak() {
    let mut bed = make_test_bed();
    bed.harness.state_mut().traces_data.max_points = 5_000;
    let traces: Vec<_> = (0..4)
        .map(|i| bed.sink.create_trace(format!("s{i}"), None))
        .collect();
    let mut t = 0.0f64;
    for step in 0..10_000 {
        for tr in &traces {
            bed.sink
                .send_points(tr, gen(&SignalSpec::sine(1.0), 4, 1000.0, t))
                .unwrap();
        }
        t += 0.004;
        if step % 10 == 0 {
            bed.harness.step();
        }
    }
    steps(&mut bed, 5);
    for i in 0..4 {
        let len = bed
            .harness
            .state()
            .traces_data
            .traces_iter()
            .find(|(n, _)| n.0 == format!("s{i}"))
            .map(|(_, t)| t.live.len())
            .unwrap_or(0);
        assert_eq!(len, 5_000, "trace s{i} should be capped at max_points");
    }
}

/// SOAK-02: rapid pause/resume cycles — snapshot consistency.
#[test]
#[ignore = "soak: 500 pause/resume cycles"]
fn pause_resume_soak() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    steps(&mut bed, 3);
    for i in 0..500 {
        if i % 2 == 0 {
            bed.harness.state_mut().pause_all();
        } else {
            bed.harness.state_mut().resume_all();
        }
        if i % 20 == 0 {
            bed.harness.step();
        }
    }
    steps(&mut bed, 5);
    assert!(!scope(&bed, 0).paused);
}

/// SOAK-03: math + thresholds active during a long soak.
#[test]
#[ignore = "soak: derived channels over 5k steps"]
fn derived_channels_soak() {
    use liveplot::data::math::{MathKind, MathTrace};
    use liveplot::data::thresholds::{ThresholdDef, ThresholdKind};
    use liveplot::panels::{MathPanel, ThresholdsPanel};
    use liveplot::TraceRef;

    let mut bed = make_test_bed();
    bed.harness.state_mut().traces_data.max_points = 2_000;
    {
        let mp = find_panel_mut::<MathPanel>(&mut bed).unwrap();
        mp.set_math_traces(vec![MathTrace::new(
            TraceRef("sum".into()),
            MathKind::Add {
                inputs: vec![(TraceRef("a".into()), 1.0), (TraceRef("b".into()), 1.0)],
            },
        )]);
    }
    {
        let tp = find_panel_mut::<ThresholdsPanel>(&mut bed).unwrap();
        let mut def = ThresholdDef::default();
        def.name = "thr".into();
        def.target = TraceRef("a".into());
        def.kind = ThresholdKind::GreaterThan { value: 0.5 };
        tp.thresholds.insert("thr".into(), def);
    }
    let a = bed.sink.create_trace("a", None);
    let b = bed.sink.create_trace("b", None);
    for i in 0..5_000u64 {
        let t = i as f64 / 100.0;
        bed.sink
            .send_points(
                &a,
                gen(&SignalSpec::sine(1.0), 4, 100.0, t)
                    .into_iter()
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        bed.sink
            .send_points(
                &b,
                gen(
                    &SignalSpec::Square {
                        freq: 0.5,
                        amp: 0.5,
                        duty: 0.5,
                    },
                    4,
                    100.0,
                    t,
                )
                .into_iter()
                .collect::<Vec<_>>(),
            )
            .unwrap();
        if i % 10 == 0 {
            bed.harness.step();
        }
    }
    steps(&mut bed, 5);
    // Math trace exists and stays bounded.
    let len = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "sum")
        .map(|(_, t)| t.live.len())
        .unwrap_or(0);
    assert!(len > 0 && len <= 2_000);
}
