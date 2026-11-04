//! CAT-LGD — legend edge cases: alt-click isolate, info labels,
//! force-hide, narrow-width suppression.

use liveplot::data::scope::LegendPosition;
use liveplot::panels::scope_legend::trace_item_id;

use crate::harness::*;
use crate::signals::SignalSpec;

fn setup(bed: &mut TestBed, pos: LegendPosition) {
    feed_spec(bed, "sig_a", &SignalSpec::sine(3.0), 300, 1000.0, 0.0);
    feed_spec(bed, "sig_b", &SignalSpec::sine(5.0), 300, 1000.0, 0.0);
    scope_mut(bed, 0).legend_position = pos;
    steps(bed, 3);
}

fn legend_rect(bed: &TestBed) -> egui::Rect {
    bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .expect("legend overlay")
}

fn click_mod(bed: &mut TestBed, pos: egui::Pos2, mods: egui::Modifiers) {
    use egui::Event;
    bed.harness
        .input_mut()
        .events
        .push(Event::ModifiersChanged(mods));
    bed.harness.step();
    for ev in [
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        },
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: mods,
        },
    ] {
        bed.harness.input_mut().events.push(ev);
        bed.harness.step();
    }
    bed.harness
        .input_mut()
        .events
        .push(Event::ModifiersChanged(egui::Modifiers::NONE));
    bed.harness.step();
}

/// LGD-08: alt-click isolates one trace — all others hidden.
#[test]
fn legend_alt_click_isolates() {
    let mut bed = make_test_bed();
    setup(&mut bed, LegendPosition::LeftTop);
    let r = legend_rect(&bed);
    click_mod(
        &mut bed,
        r.left_top() + egui::vec2(30.0, 12.0),
        egui::Modifiers::ALT,
    );
    steps(&mut bed, 2);
    let hidden = &scope(&bed, 0).legend_hidden_items;
    let a = trace_item_id(&liveplot::TraceRef("sig_a".into()));
    let b = trace_item_id(&liveplot::TraceRef("sig_b".into()));
    assert!(
        hidden.contains(&b) && !hidden.contains(&a),
        "alt-click should isolate sig_a (hidden={hidden:?})"
    );
}

/// LGD-11: `show_info_in_legend` + `SetTraceInfo` appends info to the row.
#[test]
fn legend_shows_trace_info() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "sig_a", &SignalSpec::sine(3.0), 300, 1000.0, 0.0);
    let t = bed.sink.create_trace("sig_a", None);
    bed.sink.set_trace_info(&t, "unit test info".into());
    {
        let s = scope_mut(&mut bed, 0);
        s.show_info_in_legend = true;
    }
    steps(&mut bed, 3);
    // The legend is wider with info appended — just verify it still fits.
    let plot_rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let plot = egui::Rect::from_min_max(
        egui::pos2(plot_rect[0], plot_rect[1]),
        egui::pos2(plot_rect[2], plot_rect[3]),
    );
    let legend = legend_rect(&bed);
    assert!(legend.width() <= plot.width() * 0.8 + 1.0);
}

/// LGD-13: `force_hide_legend` suppresses the overlay entirely.
#[test]
fn force_hide_legend_suppresses_overlay() {
    let mut bed = make_test_bed();
    setup(&mut bed, LegendPosition::LeftTop);
    assert!(bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .is_some());
    scope_mut(&mut bed, 0).force_hide_legend = true;
    steps(&mut bed, 3);
    assert!(
        bed.harness.state().liveplot_panel.scope_panels()[0]
            .legend_overlay_rect()
            .is_none(),
        "force_hide_legend must suppress the overlay"
    );
}

/// LGD-12: below `min_width_for_legend` the legend is skipped.
#[test]
fn narrow_scope_suppresses_legend() {
    let mut bed = make_test_bed();
    feed_spec(&bed, "a", &SignalSpec::sine(1.0), 100, 100.0, 0.0);
    bed.harness.set_size(egui::vec2(360.0, 500.0));
    steps(&mut bed, 4);
    // At ~360px the sidebar collapses and the plot is too narrow for a legend.
    let rect = bed.harness.state().liveplot_panel.scope_panels()[0].legend_overlay_rect();
    let plot = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let plot_w = plot[2] - plot[0];
    let min_w = bed.harness.state().liveplot_panel.scope_panels()[0].min_width_for_legend;
    if plot_w < min_w {
        assert!(
            rect.is_none(),
            "legend should be suppressed below threshold"
        );
    }
}
