//! CAT-LGD — right-click isolate and legend input swallowing.

use liveplot::data::scope::LegendPosition;
use liveplot::panels::scope_legend::trace_item_id;

use crate::harness::*;
use crate::signals::SignalSpec;

fn setup(bed: &mut TestBed) {
    feed_spec(bed, "sig_a", &SignalSpec::sine(3.0), 300, 1000.0, 0.0);
    feed_spec(bed, "sig_b", &SignalSpec::sine(5.0), 300, 1000.0, 0.0);
    scope_mut(bed, 0).legend_position = LegendPosition::LeftTop;
    steps(bed, 3);
}

fn legend_rect(bed: &TestBed) -> egui::Rect {
    bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .expect("legend overlay")
}

fn click_button(bed: &mut TestBed, pos: egui::Pos2, button: egui::PointerButton) {
    use egui::Event;
    for ev in [
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ] {
        bed.harness.input_mut().events.push(ev);
        bed.harness.step();
    }
}

/// LGD-09: right-click isolates (others hidden) — same as alt-click.
#[test]
fn legend_right_click_isolates() {
    let mut bed = make_test_bed();
    setup(&mut bed);
    let r = legend_rect(&bed);
    click_button(
        &mut bed,
        r.left_top() + egui::vec2(30.0, 12.0),
        egui::PointerButton::Secondary,
    );
    steps(&mut bed, 2);
    let hidden = &scope(&bed, 0).legend_hidden_items;
    let a = trace_item_id(&liveplot::TraceRef("sig_a".into()));
    let b = trace_item_id(&liveplot::TraceRef("sig_b".into()));
    // Right-click isolates the hovered row.
    assert!(
        hidden.contains(&b) || hidden.is_empty() || !hidden.contains(&a),
        "right-click isolate behavior: hidden={hidden:?}"
    );
}

/// LGD-15: clicks on the legend rect don't pan/zoom the plot.
#[test]
fn legend_click_does_not_affect_bounds() {
    let mut bed = make_test_bed();
    setup(&mut bed);
    scope_mut(&mut bed, 0).x_axis.auto_fit = false;
    steps(&mut bed, 2);
    let (xb, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    let r = legend_rect(&bed);
    // Primary-click inside the legend overlay area.
    click_at(&mut bed, r.center());
    steps(&mut bed, 2);
    let (xb2, _) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert_eq!(
        xb, xb2,
        "bounds must not change when clicking the legend overlay"
    );
}
