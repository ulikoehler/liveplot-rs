//! CAT-TRG — trigger definitions inside the live app.

use liveplot::data::triggers::{Trigger, TriggerSlope};
use liveplot::panels::TriggersPanel;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::SignalSpec;

#[test]
fn trigger_definition_renders_and_fires() {
    let mut bed = make_test_bed();
    // Step signal crossing the trigger level.
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
        let tp = find_panel_mut::<TriggersPanel>(&mut bed).unwrap();
        let mut trig = Trigger::default();
        trig.name = "trig1".into();
        trig.target = TraceRef("step".into());
        trig.level = 0.5;
        trig.slope = TriggerSlope::Rising;
        trig.enabled = true;
        tp.triggers.insert("trig1".into(), trig);
    }
    steps(&mut bed, 5);
    let tp = find_panel::<TriggersPanel>(&bed).unwrap();
    assert!(tp.triggers.contains_key("trig1"));
    assert!(!scope(&bed, 0).trace_order.is_empty());
}

#[test]
fn disabled_trigger_does_not_render_overlay() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "s", &SignalSpec::Constant { v: 1.0 }, 10, 10.0, 0.0);
    {
        let tp = find_panel_mut::<TriggersPanel>(&mut bed).unwrap();
        let mut trig = Trigger::default();
        trig.name = "off".into();
        trig.target = TraceRef("s".into());
        trig.level = 0.5;
        trig.enabled = false;
        tp.triggers.insert("off".into(), trig);
    }
    steps(&mut bed, 4);
    assert!(!find_panel::<TriggersPanel>(&bed).unwrap().triggers["off"].enabled);
}
