//! CAT-XY — XY-scope extras: newest-point highlight, pair hide, auto-fit.

use liveplot::data::scope::ScopeType;
use liveplot::panels::scope_legend::xy_pair_item_id;
use liveplot::TraceRef;

use crate::harness::*;
use crate::signals::SignalSpec;

fn setup_xy(bed: &mut TestBed) {
    feed_spec(
        bed,
        "xs",
        &SignalSpec::Custom(std::sync::Arc::new(|i, _| i as f64)),
        200,
        100.0,
        0.0,
    );
    feed_spec(bed, "ys", &SignalSpec::sine(1.0), 200, 100.0, 0.0);
    {
        let s = scope_mut(bed, 0);
        s.scope_type = ScopeType::XYScope;
        s.xy_pairs.push((
            Some(TraceRef("xs".into())),
            Some(TraceRef("ys".into())),
            Default::default(),
        ));
    }
    steps(bed, 4);
}

/// XY-05: auto-fit bounds cover both axes' data ranges.
#[test]
fn xy_bounds_cover_both_series() {
    let mut bed = make_test_bed();
    setup_xy(&mut bed);
    {
        let s = scope_mut(&mut bed, 0);
        s.x_axis.auto_fit = true;
        s.y_axis.auto_fit = true;
    }
    steps(&mut bed, 3);
    let (xb, yb) = scope(&bed, 0).last_plot_bounds.unwrap();
    assert!(xb[1] >= 100.0, "x bounds should cover xs range {xb:?}");
    assert!(
        yb[0] <= -0.5 && yb[1] >= 0.5,
        "y bounds should cover sine range {yb:?}"
    );
}

/// XY-04: hiding the pair via PlotMemory syncs the pair look.
#[test]
fn xy_pair_hide_via_legend_memory() {
    let mut bed = make_test_bed();
    setup_xy(&mut bed);
    let pair_id = xy_pair_item_id(&TraceRef("xs".into()), &TraceRef("ys".into()));
    // Hide via the scope's stored plot memory (as a legend click would).
    {
        let s = scope_mut(&mut bed, 0);
        let plot_id = s.plot_id.expect("plot_id stored after render");
        let ctx = bed.harness.ctx.clone();
        if let Some(mut mem) = egui_plot::PlotMemory::load(&ctx, plot_id) {
            mem.hidden_items.insert(pair_id);
            mem.store(&ctx, plot_id);
        }
    }
    steps(&mut bed, 3);
    let pair_visible = scope(&bed, 0).xy_pairs[0].2.visible;
    assert!(!pair_visible, "hidden pair id should mark pair invisible");
}

/// XY-03: newest-point highlight adds a marker entry without breaking.
#[test]
fn xy_highlight_newest_point() {
    let mut bed = make_test_bed();
    setup_xy(&mut bed);
    {
        let s = scope_mut(&mut bed, 0);
        s.xy_pairs[0].2.highlight_newest_point = true;
    }
    steps(&mut bed, 3);
    // Legend should include the "(last)" style entry — rect exists.
    assert!(bed.harness.state().liveplot_panel.scope_panels()[0]
        .legend_overlay_rect()
        .is_some());
}
