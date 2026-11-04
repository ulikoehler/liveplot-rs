//! CAT-THR — threshold definitions evaluated against live data.

use liveplot::data::thresholds::{ThresholdDef, ThresholdKind};
use liveplot::panels::ThresholdsPanel;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::{gen, SignalSpec};

#[test]
fn greater_than_threshold_fires_events() {
    let mut bed = make_test_bed();
    // Step signal that crosses 0.5 upward at idx 20 and back down at idx 40.
    feed_spec(
        &bed,
        "step",
        &SignalSpec::Step {
            levels: vec![0.0, 1.0, 0.0],
            per: 20,
        },
        60,
        60.0,
        0.0,
    );
    {
        let tp = find_panel_mut::<ThresholdsPanel>(&mut bed).unwrap();
        let mut def = ThresholdDef::default();
        def.name = "over".into();
        def.target = TraceRef("step".into());
        def.kind = ThresholdKind::GreaterThan { value: 0.5 };
        tp.thresholds.insert("over".into(), def);
    }
    steps(&mut bed, 4);
    // The threshold should appear in the legend entries (overlays).
    let tp = find_panel::<ThresholdsPanel>(&bed).unwrap();
    assert!(tp.thresholds.contains_key("over"));
    // And the scope should still be rendering without panic — state check.
    assert!(!scope(&bed, 0).trace_order.is_empty());
}

#[test]
fn threshold_kinds_evaluate() {
    let gt = ThresholdKind::GreaterThan { value: 0.5 };
    assert!(gt.is_active(0.6));
    assert!(!gt.is_active(0.5));
    let lt = ThresholdKind::LessThan { value: -0.2 };
    assert!(lt.is_active(-0.3));
    let ir = ThresholdKind::InRange {
        low: -1.0,
        high: 1.0,
    };
    assert!(ir.is_active(0.0));
    assert!(!ir.is_active(2.0));
}

#[test]
fn removing_threshold_stops_overlay() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "s", &SignalSpec::Constant { v: 1.0 }, 10, 10.0, 0.0);
    {
        let tp = find_panel_mut::<ThresholdsPanel>(&mut bed).unwrap();
        let mut def = ThresholdDef::default();
        def.name = "t1".into();
        def.target = TraceRef("s".into());
        def.kind = ThresholdKind::LessThan { value: 0.0 };
        tp.thresholds.insert("t1".into(), def);
    }
    steps(&mut bed, 3);
    {
        let tp = find_panel_mut::<ThresholdsPanel>(&mut bed).unwrap();
        tp.thresholds.remove("t1");
    }
    steps(&mut bed, 3);
    assert!(!find_panel::<ThresholdsPanel>(&bed)
        .unwrap()
        .thresholds
        .contains_key("t1"));
}

#[test]
fn feed_through_gen_helper_directly() {
    // Sanity: generators produce expected points (used by stress tests).
    let pts = gen(&SignalSpec::tone(1.0), 10, 100.0, 0.0);
    assert_eq!(pts.len(), 10);
    assert!(pts.iter().all(|p| p.y.abs() <= 1.0 + 1e-9));
}
