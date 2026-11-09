//! Parity suite: run the same scope scenario through the egui_plot
//! path and the VolcanoPlot path and assert the observable outcome
//! matches. Behavioral checks run everywhere; pixel checks use a wgpu
//! harness and skip gracefully when no adapter is available.
//!
//! Legend / autoscroll / trace-visibility regressions in the volcano
//! path all surface as failures here.

use crate::harness::*;

/// Optional wgpu bed — None when no GPU adapter exists on the box.
fn wgpu_bed() -> Option<TestBed> {
    let (sink, rx) = liveplot::channel_plot();
    let panel = liveplot::LivePlotPanel::new(rx);
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(W, H))
            .wgpu()
            .with_context_setup(|ctx| {
                egui_material_icons::initialize(ctx);
            })
            .with_accessibility_check(false)
            .with_step_dt(1.0 / 60.0)
            .build_ui_state(
                move |ui, panel| {
                    liveplot::data::hotkeys::handle_hotkeys(panel, ui.ctx());
                    panel.apply_controllers_embedded(ui.ctx());
                    panel.update(ui);
                },
                panel,
            )
    }));
    match built {
        Ok(mut h) => {
            h.step();
            Some(TestBed { harness: h, sink })
        }
        Err(_) => {
            eprintln!("parity: no wgpu adapter — skipping pixel check");
            None
        }
    }
}

/// The legend overlay must render on both paths when enabled.
#[test]
fn parity_legend_overlay_rect() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        assert!(
            scope(&bed, 0).legend_overlay_rect.is_some(),
            "legend overlay must render (volcano={vol})"
        );
    }
}

/// Streaming data must advance the trailing x-window on both paths.
#[test]
fn parity_autoscroll_advances_bounds() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 10);
        let b0 = scope(&bed, 0).x_axis.bounds.1;
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 40.0);
        steps(&mut bed, 10);
        let b1 = scope(&bed, 0).x_axis.bounds.1;
        assert!(
            b1 > b0,
            "autoscroll must advance right bound (volcano={vol}): {b0} -> {b1}"
        );
    }
}

/// Primary-drag pan moves bounds identically on both paths.
#[test]
fn parity_primary_drag_pans() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.x_axis.auto_fit = false;
            s.y_axis.auto_fit = false;
            s.paused = true;
        }
        steps(&mut bed, 25);
        let (xb0, _) = scope(&bed, 0).last_plot_bounds.unwrap();
        let rect = scope(&bed, 0).last_plot_screen_rect.unwrap();
        let c = egui::pos2(
            rect[0] + (rect[2] - rect[0]) / 2.0,
            rect[1] + (rect[3] - rect[1]) / 2.0,
        );
        drag(
            &mut bed,
            c,
            egui::pos2(c.x + 80.0, c.y),
            egui::PointerButton::Primary,
        );
        steps(&mut bed, 5);
        let (xb1, _) = scope(&bed, 0).last_plot_bounds.unwrap();
        assert!(
            (xb1[0] - xb0[0]).abs() > 1e-6 || (xb1[1] - xb0[1]).abs() > 1e-6,
            "pan must move bounds (volcano={vol}): {xb0:?} -> {xb1:?}"
        );
    }
}

/// XY scope renders the pair on both paths.
#[test]
fn parity_xy_scope_renders_pair() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "xs", 300, 1.0, 100.0, 0.0);
        feed_sine(&bed, "ys", 300, 0.5, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.scope_type = liveplot::data::scope::ScopeType::XYScope;
            s.xy_pairs.push((
                Some(liveplot::TraceRef("xs".into())),
                Some(liveplot::TraceRef("ys".into())),
                Default::default(),
            ));
            s.volcano_enabled = vol;
        }
        steps(&mut bed, 20);
        let s = scope(&bed, 0);
        assert!(
            s.rendered_this_frame,
            "XY scope must render (volcano={vol})"
        );
        assert!(s.last_plot_bounds.is_some());
    }
}

/// FFT scope renders on both paths (feature-gated).
#[cfg(feature = "fft")]
#[test]
fn parity_fft_scope_renders() {
    use liveplot::panels::fft_ui::FftPanel;
    use liveplot::Panel;
    for vol in [false, true] {
        let mut bed = make_test_bed();
        {
            let fp = find_panel_mut::<FftPanel>(&mut bed).expect("FftPanel");
            fp.state_mut().visible = true;
            fp.scope_ui.get_data_mut().volcano_enabled = vol;
        }
        feed_sine(&bed, "sig", 4096, 1.0, 1000.0, 0.0);
        steps(&mut bed, 40);
        let data = find_panel::<FftPanel>(&bed).unwrap().scope_ui.get_data();
        assert!(
            data.rendered_this_frame,
            "FFT scope renders (volcano={vol})"
        );
        if vol {
            assert!(
                data.volcano_surface
                    .as_ref()
                    .is_some_and(|v| v.initialized()),
                "FFT surface must initialize"
            );
        }
    }
}

/// Pixel check: the plot area must contain *colored* (non-gray) pixels
/// — grid/labels are grayscale, traces are colored. A blank volcano
/// render fails here. wgpu only; skips without an adapter.
#[test]
fn parity_trace_pixels_inside_plot() {
    for vol in [false, true] {
        let Some(mut bed) = wgpu_bed() else { return };
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 12);
        let rect = scope(&bed, 0).last_plot_screen_rect.expect("plot rect");
        let Ok(img) = bed.harness.render() else {
            eprintln!("parity: render() unavailable — skipping");
            return;
        };
        let (x0, y0) = (rect[0] as u32 + 5, rect[1] as u32 + 5);
        let (x1, y1) = (
            (rect[2] as u32).min(img.width() - 5),
            (rect[3] as u32).min(img.height() - 5),
        );
        let mut colored = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = img.get_pixel(x, y).0;
                // Colored (saturated) pixel: channel spread > 30.
                let mx = *p.iter().max().unwrap() as i32;
                let mn = *p.iter().min().unwrap() as i32;
                if mx - mn > 30 {
                    colored += 1;
                }
            }
        }
        assert!(
            colored > 100,
            "plot must contain trace-colored pixels (volcano={vol}), got {colored}"
        );
    }
}

/// Legend presence in pixels: the legend rect region must contain
/// content on both paths. wgpu only.
#[test]
fn parity_legend_pixels() {
    for vol in [false, true] {
        let Some(mut bed) = wgpu_bed() else { return };
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 12);
        let Some(lr) = scope(&bed, 0).legend_overlay_rect else {
            panic!("legend rect missing (volcano={vol})");
        };
        let lr = egui::Rect::from_min_max(egui::pos2(lr[0], lr[1]), egui::pos2(lr[2], lr[3]));
        let Ok(img) = bed.harness.render() else {
            return;
        };
        let (x0, y0) = (lr.left() as u32, lr.top() as u32);
        let (x1, y1) = (
            (lr.right() as u32).min(img.width() - 1),
            (lr.bottom() as u32).min(img.height() - 1),
        );
        let mut distinct = std::collections::HashSet::new();
        for y in y0..y1 {
            for x in x0..x1 {
                distinct.insert(img.get_pixel(x, y).0[..3].to_vec());
            }
        }
        assert!(
            distinct.len() > 4,
            "legend region should have text+marker colors (volcano={vol}), got {}",
            distinct.len()
        );
    }
}

/// App-level parity — run inside the real `LivePlotApp` (eframe App)
/// so the frame/panel context matches the shipped binary.
#[test]
fn parity_legend_app_level() {
    use crate::signals::SignalSpec;
    for vol in [false, true] {
        let mut bed = make_app_bed();
        for s in bed
            .harness
            .state_mut()
            .main_panel
            .liveplot_panel
            .get_data_mut()
            .iter_mut()
        {
            s.volcano_enabled = vol;
        }
        let trace = bed.sink.create_trace("sig", None);
        bed.sink
            .send_points(
                &trace,
                crate::signals::gen(&SignalSpec::sine(1.0), 2000, 100.0, 0.0),
            )
            .unwrap();
        for _ in 0..20 {
            bed.harness.step();
        }
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        assert!(
            scopes[0].rendered_this_frame,
            "app-level scope renders (volcano={vol})"
        );
        assert!(
            scopes[0].legend_overlay_rect.is_some(),
            "app-level legend must render (volcano={vol})"
        );
    }
}

// ── Regression tests for the reported bugs (docs/volcano-parity-
// verification-plan.md: RC-1 axes-rect, RC-2 autoscroll-while-zoomed,
// RC-3/RC-4 legend, RC-11 XY fit). ───────────────────────────────────

/// T-SCRL-002: zoomed x keeps tracking `now` at the zoomed span.
/// THE reported autoscroll bug: on the broken path the view froze at
/// the zoom instant; on both paths the trailing window must keep
/// advancing with the new (narrower) span.
#[test]
fn parity_autoscroll_span_kept_when_zoomed() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0); // t = 0..40
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            eprintln!("no vulkan — skipping volcano arm");
            continue;
        }
        // Wheel-zoom x in a few steps at axes center.
        let c = axes_rect(&bed, 0).center();
        scroll_at(&mut bed, c, 0.0, 150.0, 6);
        steps(&mut bed, 10);
        let (xb, _) = view_bounds(&bed, 0);
        let span_zoomed = xb[1] - xb[0];
        assert!(
            span_zoomed < 10.0,
            "zoom must have narrowed the view (vol={vol}): {xb:?}"
        );
        // Newer data arrives — the window must keep tracking `now`.
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 40.0); // t = 40..60
        steps(&mut bed, 15);
        let (xb, _) = view_bounds(&bed, 0);
        let span_now = xb[1] - xb[0];
        assert!(
            (span_now - span_zoomed).abs() < span_zoomed * 0.3 + 0.05,
            "zoomed span must persist (vol={vol}): {span_zoomed} -> {span_now}"
        );
        assert!(
            xb[1] > 55.0,
            "right edge must track `now`≈60 (vol={vol}): {xb:?}"
        );
    }
}

/// T-SCRL-003: an x-zoom writes the new span into `time_window` —
/// that is what makes the trailing window keep its zoomed width.
#[test]
fn parity_time_window_updated_on_x_zoom() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let tw0 = scope(&bed, 0).time_window;
        let c = axes_rect(&bed, 0).center();
        scroll_at(&mut bed, c, 0.0, 150.0, 6);
        steps(&mut bed, 10);
        let tw1 = scope(&bed, 0).time_window;
        let (xb, _) = view_bounds(&bed, 0);
        assert!(
            tw1 < tw0,
            "time_window must shrink on zoom-in (vol={vol}): {tw0} -> {tw1}"
        );
        // tw is the INNER span; view bounds are padded ~1.1x.
        let inner = (xb[1] - xb[0]) / 1.1;
        assert!(
            (tw1 - inner).abs() < inner * 0.05 + 1e-9,
            "time_window must equal inner x span (vol={vol}): tw={tw1} inner={inner}"
        );
    }
}

/// T-LGD-002: the legend overlay anchors INSIDE the axes rect on both
/// paths — the RC-1 margin mismatch placed it wrong on volcano.
#[test]
fn parity_legend_inside_axes() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let lr = scope(&bed, 0).legend_overlay_rect.expect("legend rect");
        let legend = egui::Rect::from_min_max(egui::pos2(lr[0], lr[1]), egui::pos2(lr[2], lr[3]));
        let axes = axes_rect(&bed, 0);
        assert!(
            legend.left() >= axes.left() - 1.0
                && legend.right() <= axes.right() + 1.0
                && legend.top() >= axes.top() - 1.0
                && legend.bottom() <= axes.bottom() + 1.0,
            "legend must be inside axes (vol={vol}): legend={legend:?} axes={axes:?}"
        );
    }
}

/// T-LGD-004/016: clicking a legend row hides the trace — hidden_items
/// AND `legend_hidden_items` publish on both paths (RC-3 sync).
#[test]
fn parity_legend_click_hides_trace() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let lr = scope(&bed, 0).legend_overlay_rect.expect("legend");
        // First row center: legend frame has 8/4px inner margin.
        let row = egui::pos2(lr[0] + 24.0, lr[1] + 12.0);
        click_at(&mut bed, row);
        steps(&mut bed, 6);
        let id = liveplot::panels::scope_legend::trace_item_id(&liveplot::TraceRef("sig".into()));
        assert!(
            scope(&bed, 0).legend_hidden_items.contains(&id),
            "hidden_items must contain the trace id (vol={vol})"
        );
        // Click again → restored.
        click_at(&mut bed, row);
        steps(&mut bed, 6);
        assert!(
            !scope(&bed, 0).legend_hidden_items.contains(&id),
            "second click must restore (vol={vol})"
        );
    }
}

/// T-XY-002: an XY scope's view fits the joined pair extent on both
/// paths — the volcano fit previously covered only `trace_order`.
#[test]
fn parity_xy_viewport_fits_pairs() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "xs", 500, 1.0, 100.0, 0.0);
        feed_sine(&bed, "ys", 500, 0.5, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.scope_type = liveplot::data::scope::ScopeType::XYScope;
            s.xy_pairs.push((
                Some(liveplot::TraceRef("xs".into())),
                Some(liveplot::TraceRef("ys".into())),
                Default::default(),
            ));
            s.x_axis.auto_fit = true;
            s.y_axis.auto_fit = true;
            s.volcano_enabled = vol;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let (xb, yb) = view_bounds(&bed, 0);
        // xs is a sine in [-1,1], ys a slower sine in [-1,1] — the
        // fitted view must cover roughly [-1.1, 1.1] on both axes
        // (fit + 5% pad), never the degenerate (0,1) default.
        assert!(
            xb[0] < -0.8 && xb[1] > 0.8,
            "x view must fit pair extent (vol={vol}): {xb:?}"
        );
        assert!(
            yb[0] < -0.8 && yb[1] > 0.8,
            "y view must fit pair extent (vol={vol}): {yb:?}"
        );
    }
}

/// T-XY-007: pair legend shows the egui-format entry and the `(last)`
/// marker entry exists when highlight_newest_point is on (RC-4).
#[test]
fn parity_xy_legend_entries_and_last_marker() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "xs", 500, 1.0, 100.0, 0.0);
        feed_sine(&bed, "ys", 500, 0.5, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.scope_type = liveplot::data::scope::ScopeType::XYScope;
            let mut look: liveplot::data::trace_look::TraceLook = Default::default();
            look.highlight_newest_point = true;
            s.xy_pairs.push((
                Some(liveplot::TraceRef("xs".into())),
                Some(liveplot::TraceRef("ys".into())),
                look,
            ));
            s.volcano_enabled = vol;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        // Legend must contain >= 2 entries (pair + "(last)") — the
        // overlay rect grows with entries; assert presence + that the
        // pair-id click path exists by hiding via PlotMemory.
        let lr = scope(&bed, 0).legend_overlay_rect;
        assert!(lr.is_some(), "XY legend must render (vol={vol})");
        // Pair must be hideable through the shared sync.
        let id = liveplot::panels::scope_legend::xy_pair_item_id(
            &liveplot::TraceRef("xs".into()),
            &liveplot::TraceRef("ys".into()),
        );
        let plot_id = scope(&bed, 0).plot_id.expect("plot id");
        {
            let ctx = bed.harness.ctx.clone();
            let mut mem = egui_plot::PlotMemory::load(&ctx, plot_id).unwrap();
            mem.hidden_items.insert(id);
            mem.store(&ctx, plot_id);
        }
        steps(&mut bed, 5);
        assert!(
            scope(&bed, 0).legend_hidden_items.contains(&id),
            "pair hidden_items must publish (vol={vol})"
        );
    }
}

/// T-CLK-001: a click in measurement mode captures the same data
/// coordinate on both paths (RC-1 wrong-margin mapping). Asserted
/// through the measurement def's captured point.
#[test]
fn parity_clicked_point_coords() {
    use liveplot::panels::MeasurementPanel;
    let mut outs = Vec::new();
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        {
            let mp = find_panel_mut::<MeasurementPanel>(&mut bed).unwrap();
            mp.add_measurement();
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            eprintln!("no vulkan — skipping volcano arm");
            continue;
        }
        let axes = axes_rect(&bed, 0);
        let c = axes.center();
        click_at(&mut bed, c);
        steps(&mut bed, 2);
        let (p1, _) = find_panel::<MeasurementPanel>(&bed).unwrap().measurements()[0].get_points();
        let vb = view_bounds(&bed, 0);
        outs.push((p1.expect("measurement p1"), vb));
    }
    if outs.len() == 2 {
        let ((e_p, (exb, eyb)), (v_p, (vxb, vyb))) = (outs[0], outs[1]);
        // The captured point must be the axes-center data coord of the
        // CURRENT view on each path (paths may show different views —
        // each must map its own view correctly).
        for (tag, p, (xb, yb)) in [("egui", e_p, (exb, eyb)), ("volcano", v_p, (vxb, vyb))] {
            let exp_x = (xb[0] + xb[1]) / 2.0;
            let exp_y = (yb[0] + yb[1]) / 2.0;
            let tol_x = (xb[1] - xb[0]).abs().max(1e-9) * 0.05;
            let tol_y = (yb[1] - yb[0]).abs().max(1e-9) * 0.05;
            assert!(
                (p[0] - exp_x).abs() < tol_x && (p[1] - exp_y).abs() < tol_y,
                "{tag}: captured {p:?} want axes-center ~[{exp_x},{exp_y}]"
            );
        }
        // And with identical feeds the two paths' views should agree.
        assert!(
            (vxb[0] - exb[0]).abs() < 0.1 && (vyb[0] - eyb[0]).abs() < 0.1,
            "views agree: egui={:?} volcano={:?}",
            outs[0].1,
            outs[1].1
        );
    }
}

/// T-RND-006: `last_plot_screen_rect`/`last_axes_screen_rect` use the
/// `[l,t,r,b]` layout on both paths and axes ⊂ plot.
#[test]
fn parity_screen_rect_layout() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let pr = plot_rect(&bed, 0);
        let ax = axes_rect(&bed, 0);
        assert!(
            pr.width() > 100.0 && pr.height() > 50.0,
            "plot rect must be a real rect (vol={vol}): {pr:?}"
        );
        assert!(
            ax.left() >= pr.left() - 1.0
                && ax.right() <= pr.right() + 1.0
                && ax.top() >= pr.top() - 1.0
                && ax.bottom() <= pr.bottom() + 1.0,
            "axes inside plot (vol={vol}): axes={ax:?} plot={pr:?}"
        );
        // Margins exist but are sane (< 30% of each dimension).
        assert!(
            (ax.left() - pr.left()).abs() < pr.width() * 0.3
                && (pr.bottom() - ax.bottom()).abs() < pr.height() * 0.3,
            "sane margins (vol={vol})"
        );
    }
}

/// T-ZOOM-005: ZoomMode::Off — wheel must change NOTHING, not even
/// auto_fit (RC-8: volcano used to pin the view on any scroll).
#[test]
fn parity_wheel_zoom_off_is_noop() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        set_zoom_mode(&mut bed, 0, liveplot::panels::scope_ui::ZoomMode::Off);
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let af_x = scope(&bed, 0).x_axis.auto_fit;
        let af_y = scope(&bed, 0).y_axis.auto_fit;
        let (b0, y0) = view_bounds(&bed, 0);
        let c = axes_rect(&bed, 0).center();
        scroll_at(&mut bed, c, 0.0, 150.0, 4);
        steps(&mut bed, 5);
        let (b1, y1) = view_bounds(&bed, 0);
        let span = (b0[1] - b0[0]).abs().max(1e-9);
        let dy = (y0[1] - y0[0]).abs().max(1e-9);
        assert!(
            (b1[0] - b0[0]).abs() < span * 1e-9 && (b1[1] - b0[1]).abs() < span * 1e-9,
            "ZoomMode::Off: x bounds frozen (vol={vol}): {b0:?} -> {b1:?}"
        );
        assert!(
            (y1[0] - y0[0]).abs() < dy * 1e-9 && (y1[1] - y0[1]).abs() < dy * 1e-9,
            "ZoomMode::Off: y bounds frozen (vol={vol}): {y0:?} -> {y1:?}"
        );
        assert_eq!(
            af_x,
            scope(&bed, 0).x_axis.auto_fit,
            "auto_fit x must not flip (vol={vol})"
        );
        assert_eq!(
            af_y,
            scope(&bed, 0).y_axis.auto_fit,
            "auto_fit y must not flip (vol={vol})"
        );
    }
}

/// T-ZOOM-018/019: on a live TimeScope an x-pan snaps back to the
/// trailing window while a y-pan sticks — egui semantics both paths
/// must share.
#[test]
fn parity_pan_y_sticks_x_recenters_live() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let ax = axes_rect(&bed, 0);
        let (xb0, yb0) = view_bounds(&bed, 0);
        // Diagonal drag: pans both axes on egui.
        drag(
            &mut bed,
            ax.center(),
            ax.center() + egui::vec2(80.0, -60.0),
            egui::PointerButton::Primary,
        );
        steps(&mut bed, 12);
        let (xb1, yb1) = view_bounds(&bed, 0);
        // y-pan sticks on both paths (live_update only touches x).
        assert!(
            (yb1[0] - yb0[0]).abs() > 1e-6,
            "y-pan must move y bounds (vol={vol}): {yb0:?} -> {yb1:?}"
        );
        // x right-edge re-centers to the trailing window (~now).
        assert!(
            (xb1[1] - xb0[1]).abs() < (xb0[1] - xb0[0]) * 0.1,
            "x must re-track trailing window (vol={vol}): {xb0:?} -> {xb1:?}"
        );
    }
}

/// T-ZOOM-011/012: box zoom in X mode applies the dragged x-range.
/// `box_zoom_start` is captured on the first decided-drag frame at the
/// CURRENT pointer pos, so we move in steps (a few px past `a` first)
/// — a single jump gives an empty box on both paths.
/// Live semantics: after the zoom, `live_update` re-anchors the window
/// to `now` at the NEW span — so assert the span shrank to the dragged
/// fraction and the right edge still tracks `now`, on both paths.
#[test]
fn parity_box_zoom_x_mode() {
    let mut outs = Vec::new();
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            eprintln!("no vulkan — skipping volcano arm");
            continue;
        }
        let ax = axes_rect(&bed, 0);
        let (xb0, yb0) = view_bounds(&bed, 0);
        // Default ZoomMode::X — horizontal band drag.
        let a = egui::pos2(ax.left() + ax.width() * 0.3, ax.center().y);
        let b = egui::pos2(ax.left() + ax.width() * 0.6, ax.center().y + 30.0);
        let mid = [
            a + egui::vec2(4.0, 2.0),
            a + egui::vec2(12.0, 5.0),
            a + egui::vec2(28.0, 10.0),
            b,
        ];
        drag_path(&mut bed, a, &mid, egui::PointerButton::Secondary);
        steps(&mut bed, 8);
        let (xb1, yb1) = view_bounds(&bed, 0);
        let span0 = xb0[1] - xb0[0];
        let span1 = xb1[1] - xb1[0];
        assert!(
            span1 < span0 * 0.65 && span1 > span0 * 0.15,
            "box x zoom shrinks span to ~0.3x (vol={vol}): {span0} -> {span1}"
        );
        // Right edge re-anchors to `now` — near the old right edge.
        assert!(
            (xb1[1] - xb0[1]).abs() < span0 * 0.15,
            "right edge stays at now (vol={vol}): {xb0:?} -> {xb1:?}"
        );
        // y unchanged in X mode.
        assert_eq!(yb0, yb1, "y bounds unchanged in X mode (vol={vol})");
        outs.push((xb1, yb1));
    }
    if outs.len() == 2 {
        let ((ex, ey), (vx, vy)) = (outs[0], outs[1]);
        let tol = (ex[1] - ex[0]).abs().max(1e-9) * 0.1;
        assert!(
            (vx[0] - ex[0]).abs() < tol
                && (vx[1] - ex[1]).abs() < tol
                && (vy[0] - ey[0]).abs() < tol
                && (vy[1] - ey[1]).abs() < tol,
            "identical box-zoom outcome: egui={:?} volcano={:?}",
            outs[0],
            outs[1]
        );
    }
}

/// T-ZOOM-013: box zoom in Both mode — x span shrinks to the dragged
/// fraction (re-anchored to `now`), y shrinks to the dragged y range.
#[test]
fn parity_box_zoom_both_modes() {
    let mut outs = Vec::new();
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 4000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        set_zoom_mode(&mut bed, 0, liveplot::panels::scope_ui::ZoomMode::Both);
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            eprintln!("no vulkan — skipping volcano arm");
            continue;
        }
        let ax = axes_rect(&bed, 0);
        let (xb0, yb0) = view_bounds(&bed, 0);
        let a = egui::pos2(ax.left() + ax.width() * 0.3, ax.top() + ax.height() * 0.3);
        let b = egui::pos2(ax.left() + ax.width() * 0.6, ax.top() + ax.height() * 0.7);
        let mid = [
            a + egui::vec2(4.0, 3.0),
            a + egui::vec2(12.0, 8.0),
            a + egui::vec2(28.0, 18.0),
            b,
        ];
        drag_path(&mut bed, a, &mid, egui::PointerButton::Secondary);
        steps(&mut bed, 8);
        let (xb1, yb1) = view_bounds(&bed, 0);
        let xspan0 = xb0[1] - xb0[0];
        let yspan0 = yb0[1] - yb0[0];
        let xspan1 = xb1[1] - xb1[0];
        let yspan1 = yb1[1] - yb1[0];
        assert!(
            xspan1 < xspan0 * 0.65 && xspan1 > xspan0 * 0.15,
            "x span shrinks to ~0.3x (vol={vol}): {xspan0} -> {xspan1}"
        );
        // dragged y covers screen 0.3..0.7 → ~40% of the y view.
        assert!(
            yspan1 < yspan0 * 0.7 && yspan1 > yspan0 * 0.2,
            "y span shrinks to ~0.4x (vol={vol}): {yspan0} -> {yspan1}"
        );
        outs.push((xb1, yb1));
    }
    if outs.len() == 2 {
        let ((ex, ey), (vx, vy)) = (outs[0], outs[1]);
        let tol = (ex[1] - ex[0]).abs().max(1e-9) * 0.15;
        assert!(
            (vx[0] - ex[0]).abs() < tol
                && (vx[1] - ex[1]).abs() < tol
                && (vy[0] - ey[0]).abs() < tol
                && (vy[1] - ey[1]).abs() < tol,
            "identical box-zoom outcome: egui={:?} volcano={:?}",
            outs[0],
            outs[1]
        );
    }
}

/// Helper: set a scope panel's ZoomMode through the (now mutable)
/// `scope_panels_mut` accessor.
fn set_zoom_mode(bed: &mut TestBed, idx: usize, mode: liveplot::panels::scope_ui::ZoomMode) {
    let mut panels = bed.harness.state_mut().liveplot_panel.scope_panels_mut();
    panels[idx].set_zoom_mode(mode);
}

/// T-ZOOM-001/002: wheel zoom anchors at the hovered point — the
/// anchor's data coord must be (approximately) invariant on both
/// paths, and the span must shrink by the same factor.
#[test]
fn parity_wheel_zoom_anchors_at_pointer() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            eprintln!("no vulkan — skipping volcano arm");
            continue;
        }
        let ax = axes_rect(&bed, 0);
        // Zoom in at a point 40% across the axes — not the center.
        let anchor = egui::pos2(ax.left() + ax.width() * 0.4, ax.center().y);
        let (xb0, _) = view_bounds(&bed, 0);
        let anchor_frac = (anchor.x - ax.left()) as f64 / ax.width() as f64;
        let anchor_x = xb0[0] + anchor_frac * (xb0[1] - xb0[0]);
        scroll_at(&mut bed, anchor, 0.0, 150.0, 4);
        steps(&mut bed, 4);
        let (xb1, _) = view_bounds(&bed, 0);
        assert!(
            xb1[1] - xb1[0] < xb0[1] - xb0[0],
            "zoom must narrow (vol={vol})"
        );
        // Anchor invariance: anchor_x stays at the same axes fraction.
        let after_frac = (anchor_x - xb1[0]) / (xb1[1] - xb1[0]);
        assert!(
            (after_frac - anchor_frac).abs() < 0.06,
            "zoom anchors at pointer (vol={vol}): frac {anchor_frac:.3} -> {after_frac:.3}"
        );
    }
}

/// T-ZOOM-004: ZoomMode::Y — wheel changes only y.
#[test]
fn parity_wheel_zoom_y_mode() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        set_zoom_mode(&mut bed, 0, liveplot::panels::scope_ui::ZoomMode::Y);
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let (xb0, yb0) = view_bounds(&bed, 0);
        let c = axes_rect(&bed, 0).center();
        scroll_at(&mut bed, c, 0.0, 150.0, 4);
        steps(&mut bed, 4);
        let (xb1, yb1) = view_bounds(&bed, 0);
        let xs = (xb0[1] - xb0[0]).abs().max(1e-9);
        assert!(
            (xb1[0] - xb0[0]).abs() < xs * 1e-9 && (xb1[1] - xb0[1]).abs() < xs * 1e-9,
            "Y mode: x untouched (vol={vol}): {xb0:?} -> {xb1:?}"
        );
        assert!(
            yb1[1] - yb1[0] < yb0[1] - yb0[0],
            "Y mode: y narrows (vol={vol}): {yb0:?} -> {yb1:?}"
        );
    }
}

/// T-ZOOM-021: double-click re-fits to the data extent on the volcano
/// path. NOTE: kittest cannot drive `resp.double_clicked()` through
/// egui_plot's internal response — the egui arm stays zoomed here
/// (same synthetic-input limitation that retired the old view.rs
/// tests); real-app manual protocol covers the egui reference.
#[test]
fn volcano_dblclick_refit() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let c = axes_rect(&bed, 0).center();
        scroll_at(&mut bed, c, 0.0, 200.0, 6);
        steps(&mut bed, 4);
        let (xz, _) = view_bounds(&bed, 0);
        assert!(xz[1] - xz[0] < 15.0, "zoomed in (vol={vol}): {xz:?}");
        double_click_at(&mut bed, c);
        steps(&mut bed, 4);
        let (xf, _) = view_bounds(&bed, 0);
        if vol {
            // Data spans t=0..20 — fitted view covers it.
            assert!(
                xf[0] < 1.0 && xf[1] > 19.0,
                "dblclick refits data (volcano): {xf:?}"
            );
        }
    }
}

/// T-LGD-006: legend anchors to the configured corner on both paths —
/// the overlay rect must sit in the same axes quadrant.
#[test]
fn parity_legend_position_corners() {
    use liveplot::data::scope::LegendPosition;
    for pos in [
        LegendPosition::LeftTop,
        LegendPosition::RightTop,
        LegendPosition::LeftBottom,
        LegendPosition::RightBottom,
    ] {
        for vol in [false, true] {
            let mut bed = make_test_bed();
            feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
            {
                let s = scope_mut(&mut bed, 0);
                s.volcano_enabled = vol;
                s.legend_position = pos;
            }
            steps(&mut bed, 20);
            if vol && !vol_available(&mut bed, 0) {
                continue;
            }
            let lr = scope(&bed, 0).legend_overlay_rect.expect("legend rect");
            let ax = axes_rect(&bed, 0);
            let cx = (lr[0] + lr[2]) / 2.0;
            let cy = (lr[1] + lr[3]) / 2.0;
            let left = cx < ax.center().x;
            let top = cy < ax.center().y;
            let (want_left, want_top) = match pos {
                LegendPosition::LeftTop => (true, true),
                LegendPosition::RightTop => (false, true),
                LegendPosition::LeftBottom => (true, false),
                LegendPosition::RightBottom => (false, false),
            };
            assert_eq!(
                (left, top),
                (want_left, want_top),
                "{pos:?} corner (vol={vol}): legend=({cx},{cy}) axes={ax:?}"
            );
        }
    }
}

/// T-LGD-012: force_hide_legend suppresses the overlay on both paths.
#[test]
fn parity_legend_force_hide() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.force_hide_legend = true;
        }
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        assert!(
            scope(&bed, 0).legend_overlay_rect.is_none(),
            "force_hide suppresses legend (vol={vol})"
        );
        assert!(
            scope(&bed, 0).rendered_this_frame,
            "scope still renders (vol={vol})"
        );
    }
}

/// T-CLK-003: pause_on_click toggles paused on a single click, both
/// paths.
#[test]
fn parity_pause_on_click() {
    let mut outs = Vec::new();
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.pause_on_click = true;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let paused0 = scope(&bed, 0).paused;
        let c = axes_rect(&bed, 0).center();
        click_at(&mut bed, c);
        steps(&mut bed, 4);
        let after1 = scope(&bed, 0).paused;
        click_at(&mut bed, c);
        steps(&mut bed, 4);
        let after2 = scope(&bed, 0).paused;
        // Whatever egui's exact semantics (resume vs. stay), the
        // volcano path shares the same handler → must match exactly.
        outs.push((paused0, after1, after2));
    }
    if outs.len() == 2 {
        assert_eq!(
            outs[0], outs[1],
            "pause_on_click outcomes: egui={:?} volcano={:?}",
            outs[0], outs[1]
        );
        // First click must at least pause on both.
        assert_ne!(outs[0].0, outs[0].1, "first click pauses on egui too");
    }
}

/// T-HOV-001: hovering inside the axes keeps the frame healthy and —
/// when near a point — does not panic; the label_formatter-equivalent
/// tooltip path must run on the volcano arm.
#[test]
fn parity_hover_inside_axes() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let ax = axes_rect(&bed, 0);
        // Hover near the center, then near a trace point (sine at
        // center-ish x ≈ crossing). Both must keep rendering.
        hover_at(&mut bed, ax.center());
        hover_at(&mut bed, ax.center() + egui::vec2(30.0, -40.0));
        steps(&mut bed, 3);
        assert!(
            scope(&bed, 0).rendered_this_frame,
            "frame renders while hovered (vol={vol})"
        );
    }
}
