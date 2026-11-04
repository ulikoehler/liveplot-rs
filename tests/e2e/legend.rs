//! CAT-LGD — legend overlay. Includes regression coverage for the 2.5.4
//! corner-anchoring bug (legend stretched across the whole scope for
//! right/bottom positions).

use liveplot::data::scope::LegendPosition;

use crate::harness::*;
use crate::signals::SignalSpec;

fn setup_legend(bed: &mut TestBed, pos: LegendPosition) {
    feed_spec(bed, "sig_a", &SignalSpec::sine(3.0), 500, 1000.0, 0.0);
    feed_spec(
        bed,
        "sig_b",
        &SignalSpec::Square {
            freq: 1.0,
            amp: 0.5,
            duty: 0.5,
        },
        500,
        1000.0,
        0.0,
    );
    {
        let s = scope_mut(bed, 0);
        s.show_legend = true;
        s.legend_position = pos;
    }
    steps(bed, 3);
}

/// The legend must be a snug overlay anchored at the requested corner,
/// not stretched across the plot.
#[test]
fn legend_anchors_to_all_four_corners() {
    for pos in [
        LegendPosition::LeftTop,
        LegendPosition::RightTop,
        LegendPosition::LeftBottom,
        LegendPosition::RightBottom,
    ] {
        let mut bed = make_test_bed();
        setup_legend(&mut bed, pos);

        let panel = &bed.harness.state().liveplot_panel;
        let plot_rect = scope(&bed, 0).last_plot_screen_rect.expect("plot rect");
        let plot = egui::Rect::from_min_max(
            egui::pos2(plot_rect[0], plot_rect[1]),
            egui::pos2(plot_rect[2], plot_rect[3]),
        );
        let legend = panel.scope_panels()[0]
            .legend_overlay_rect()
            .unwrap_or_else(|| panic!("no legend overlay for {pos:?}"));

        // Snug: two entries + margins must be far smaller than the plot.
        assert!(
            legend.width() < plot.width() * 0.5,
            "{pos:?}: legend too wide: {legend:?} vs plot {plot:?}"
        );
        assert!(
            legend.height() < plot.height() * 0.5,
            "{pos:?}: legend too tall: {legend:?} vs plot {plot:?}"
        );
        assert!(
            plot.expand(15.0).contains_rect(legend),
            "{pos:?}: legend outside plot: {legend:?} vs {plot:?}"
        );

        let tol = 30.0;
        match pos {
            LegendPosition::LeftTop => {
                assert!(
                    (legend.left() - plot.left()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
                assert!(
                    (legend.top() - plot.top()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
            }
            LegendPosition::RightTop => {
                assert!(
                    (legend.right() - plot.right()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
                assert!(
                    (legend.top() - plot.top()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
            }
            LegendPosition::LeftBottom => {
                assert!(
                    (legend.left() - plot.left()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
                assert!(
                    (legend.bottom() - plot.bottom()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
            }
            LegendPosition::RightBottom => {
                assert!(
                    (legend.right() - plot.right()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
                assert!(
                    (legend.bottom() - plot.bottom()).abs() < tol,
                    "{pos:?} {legend:?}"
                );
            }
        }
    }
}

/// LGD-06/07: clicking a legend entry hides/re-shows the trace.
#[test]
fn legend_click_toggles_trace() {
    let mut bed = make_test_bed();
    setup_legend(&mut bed, LegendPosition::LeftTop);

    let legend = bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .unwrap();
    // First row: click slightly inside the top-left of the legend.
    click_at(&mut bed, legend.left_top() + egui::vec2(30.0, 12.0));
    steps(&mut bed, 2);

    let item = liveplot::panels::scope_legend::trace_item_id(&liveplot::TraceRef("sig_a".into()));
    assert!(
        scope(&bed, 0).legend_hidden_items.contains(&item),
        "sig_a should be hidden after legend click"
    );

    // Click again → back visible.
    let legend = bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .unwrap();
    click_at(&mut bed, legend.left_top() + egui::vec2(30.0, 12.0));
    steps(&mut bed, 2);
    assert!(
        !scope(&bed, 0).legend_hidden_items.contains(&item),
        "sig_a should be visible again"
    );
}

/// LGD-10: many entries stay inside the plot (scrollable).
#[test]
fn legend_many_entries_stays_in_bounds() {
    let mut bed = make_test_bed();
    feed_n_traces(&bed, 40, 50, 100.0);
    scope_mut(&mut bed, 0).legend_position = LegendPosition::RightTop;
    steps(&mut bed, 3);

    let plot_rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
    let plot = egui::Rect::from_min_max(
        egui::pos2(plot_rect[0], plot_rect[1]),
        egui::pos2(plot_rect[2], plot_rect[3]),
    );
    let legend = bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .unwrap();
    assert!(
        legend.height() <= plot.height() + 1.0,
        "{legend:?} vs {plot:?}"
    );
    assert!(legend.width() <= plot.width() * 0.8 + 1.0);
}
