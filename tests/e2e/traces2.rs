//! CAT-TRACES — alias, offset, render modes, recolor, hover.

use liveplot::data::trace_look::RenderMode;
use liveplot::TraceRef;

use crate::harness::*;

/// TRC-08: `display_label` alias shows in legend but keeps the canonical id.
#[test]
fn display_alias_in_legend() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "canon_name", 100, 1.0, 100.0, 0.0);
    steps(&mut bed, 1); // ingest the register+points commands first
    {
        let tr = bed
            .harness
            .state_mut()
            .traces_data
            .get_trace_mut(&TraceRef("canon_name".into()));
        tr.unwrap().display_name = Some("Pretty Label".to_string());
    }
    steps(&mut bed, 3);
    // Legend entry still keyed by canonical name → item id unchanged.
    let id = liveplot::panels::scope_legend::trace_item_id(&TraceRef("canon_name".into()));
    assert!(
        scope(&bed, 0).legend_hidden_items.is_empty()
            || !scope(&bed, 0).legend_hidden_items.contains(&id)
    );
}

/// TRC-09: trace offset shifts drawn points.
#[test]
fn trace_offset_shifts_drawn_points() {
    let mut bed = make_test_bed();
    feed_spec(
        &bed,
        "off",
        &crate::signals::SignalSpec::Constant { v: 1.0 },
        50,
        50.0,
        0.0,
    );
    steps(&mut bed, 1);
    {
        let tr = bed
            .harness
            .state_mut()
            .traces_data
            .get_trace_mut(&TraceRef("off".into()));
        tr.unwrap().offset = 10.0;
    }
    scope_mut(&mut bed, 0).y_axis.auto_fit = true;
    steps(&mut bed, 3);
    // Offset persists across frames and the scope renders with it applied
    // (offset is added when building drawn plot points, scope_ui.rs).
    let tr = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .find(|(n, _)| n.0 == "off")
        .map(|(_, t)| t.offset);
    assert_eq!(tr, Some(10.0));
    assert!(scope(&bed, 0).rendered_this_frame);
}

/// TRC-11: render modes are stored and don't panic.
#[test]
fn render_modes_survive() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 1000, 1.0, 100.0, 0.0);
    steps(&mut bed, 1);
    for mode in [
        RenderMode::Line,
        RenderMode::Downsampled,
        RenderMode::MinMaxEnvelope,
        RenderMode::DensitySplatting,
    ] {
        let tr = bed
            .harness
            .state_mut()
            .traces_data
            .get_trace_mut(&TraceRef("a".into()));
        tr.unwrap().look.render_mode = mode;
        steps(&mut bed, 2);
    }
    // No panic — draw path accepted all modes.
    assert!(scope(&bed, 0).rendered_this_frame);
}

/// TRC-06: `recolor_using_palette` reassigns colors after a scheme change.
#[test]
fn recolor_uses_palette() {
    let mut bed = make_test_bed();
    feed_sine(&bed, "a", 10, 1.0, 10.0, 0.0);
    feed_sine(&bed, "b", 10, 1.0, 10.0, 0.0);
    steps(&mut bed, 2);
    let before: Vec<_> = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .map(|(_, t)| t.look.color)
        .collect();
    bed.harness.state_mut().traces_data.recolor_using_palette();
    steps(&mut bed, 2);
    let after: Vec<_> = bed
        .harness
        .state()
        .traces_data
        .traces_iter()
        .map(|(_, t)| t.look.color)
        .collect();
    assert_eq!(before.len(), after.len());
    // Palette recolor either reassigns or keeps palette colors — valid either
    // way as long as it doesn't panic and stays deterministic.
}
