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

/// T-RND-epoch: epoch-scale timestamps (t≈1.79e9) render and bound
/// correctly — historical f32-precision regression zone
/// (`shift`/`shift_ref` logic on the volcano path).
#[test]
fn parity_epoch_timestamps() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        // feed_sine's t0 becomes the epoch base.
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 1_790_000_000.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let (xb, _) = view_bounds(&bed, 0);
        // Trailing window at now≈1.79e9+20 — bounds must be at epoch
        // scale, not collapsed to zero or f32-quantized.
        assert!(
            xb[0] > 1_789_999_990.0 && xb[1] > 1_790_000_010.0,
            "epoch bounds (vol={vol}): {xb:?}"
        );
        assert!(
            scope(&bed, 0).rendered_this_frame,
            "frame renders at epoch scale (vol={vol})"
        );
        // The trace must be visible — count drawn pixels via the
        // colored-pixel probe used by the other pixel tests? The
        // behavioral check: legend + bounds sane is enough here.
    }
}

/// T-LGD-005: alt-click on a legend row isolates the item — only it
/// stays visible (shared PlotMemory path on both arms).
#[test]
fn parity_legend_alt_click_isolates() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig_a", 2000, 1.0, 100.0, 0.0);
        feed_sine(&bed, "sig_b", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 25);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let lr = scope(&bed, 0).legend_overlay_rect.expect("legend");
        let row = egui::pos2(lr[0] + 24.0, lr[1] + 12.0);
        // alt-click isolates the FIRST entry (highest in the list).
        bed.harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(row));
        bed.harness.step();
        for pressed in [true, false] {
            bed.harness
                .input_mut()
                .events
                .push(egui::Event::PointerButton {
                    pos: row,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::ALT,
                });
            bed.harness.step();
        }
        steps(&mut bed, 5);
        // Exactly one of the two trace ids must be hidden afterwards.
        let id_a =
            liveplot::panels::scope_legend::trace_item_id(&liveplot::TraceRef("sig_a".into()));
        let id_b =
            liveplot::panels::scope_legend::trace_item_id(&liveplot::TraceRef("sig_b".into()));
        let hidden = &scope(&bed, 0).legend_hidden_items;
        let (ha, hb) = (hidden.contains(&id_a), hidden.contains(&id_b));
        assert_ne!(
            ha, hb,
            "alt-click isolates exactly one (vol={vol}): a={ha} b={hb}"
        );
    }
}

/// T-LGD-018: wheel over the legend scrolls it (or is ignored) instead
/// of zooming the plot — bounds unchanged on both paths.
#[test]
fn parity_wheel_over_legend_no_zoom() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let lr = scope(&bed, 0).legend_overlay_rect.expect("legend");
        let inside_legend = egui::pos2((lr[0] + lr[2]) / 2.0, (lr[1] + lr[3]) / 2.0);
        let (xb0, yb0) = view_bounds(&bed, 0);
        scroll_at(&mut bed, inside_legend, 0.0, 150.0, 4);
        steps(&mut bed, 4);
        let (xb1, yb1) = view_bounds(&bed, 0);
        let tol = (xb0[1] - xb0[0]).abs().max(1e-9) * 1e-6;
        assert!(
            (xb1[0] - xb0[0]).abs() < tol
                && (xb1[1] - xb0[1]).abs() < tol
                && (yb1[0] - yb0[0]).abs() < tol
                && (yb1[1] - yb0[1]).abs() < tol,
            "wheel over legend must not zoom (vol={vol}): {xb0:?}->{xb1:?}"
        );
    }
}

/// T-FFT-001: the FFT panel's embedded scope renders through the
/// volcano path too — spectrum bounds + render flag on both arms.
#[cfg(feature = "fft")]
#[test]
fn parity_fft_embedded_scope() {
    use liveplot::panels::fft_ui::FftPanel;
    use liveplot::panels::Panel;
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "tone", 8192, 1.0, 1000.0, 0.0);
        {
            let fp = find_panel_mut::<FftPanel>(&mut bed).expect("FftPanel");
            fp.state_mut().visible = true;
            fp.scope_ui.get_data_mut().volcano_enabled = vol;
        }
        steps(&mut bed, 30);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let data = find_panel::<FftPanel>(&bed).unwrap().scope_ui.get_data();
        // Embedded scope must render and produce sane bounds — the
        // volcano path uses the same ScopePanel code.
        assert!(data.rendered_this_frame, "fft scope renders (vol={vol})");
        let b = data.last_plot_bounds.expect("fft plot bounds");
        assert!(
            b.0[1] > b.0[0] && b.1[1] > b.1[0],
            "sane fft bounds (vol={vol}): {b:?}"
        );
    }
}

/// App-level (real LivePlotApp) legend + screenshot — merged from the
/// retired zz_snap.rs. Both traces render in the volcano path inside
/// the real app; `harness.render()` must produce a frame.
#[test]
fn parity_app_level_legend_and_screenshot() {
    let mut bed = make_app_bed();
    for s in bed
        .harness
        .state_mut()
        .main_panel
        .liveplot_panel
        .get_data_mut()
        .iter_mut()
    {
        s.volcano_enabled = true;
    }
    let two = bed.sink.create_trace("two_tone", None);
    bed.sink
        .send_points(
            &two,
            crate::signals::gen(&crate::signals::SignalSpec::sine(5.0), 4096, 1000.0, 0.0),
        )
        .unwrap();
    let sw = bed.sink.create_trace("swept_freq", None);
    bed.sink
        .send_points(
            &sw,
            crate::signals::gen(&crate::signals::SignalSpec::sine(50.0), 4096, 1000.0, 0.0),
        )
        .unwrap();
    for _ in 0..40 {
        bed.harness.step();
    }
    let (lr, rd) = {
        let scopes = bed.harness.state().main_panel.liveplot_panel.get_data();
        (scopes[0].legend_overlay_rect, scopes[0].rendered_this_frame)
    };
    assert!(lr.is_some(), "legend must render in the real app");
    assert!(rd, "scope renders in the real app");
    // wgpu render path (software rasterizer) — proves the volcano
    // texture composits into a real frame.
    assert!(bed.harness.render().is_ok(), "screenshot frame renders");
}

/// T-TRC-004: `show_points` + marker style changes re-upload and keep
/// rendering on both paths (W-14 sig coverage — marker/size folded
/// into the per-trace signature).
#[test]
fn parity_show_points_marker() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            s.paused = true;
        }
        steps(&mut bed, 20);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        // Toggle show_points + change marker — must not break rendering.
        for (sp, marker) in [
            (true, egui_plot::MarkerShape::Diamond),
            (true, egui_plot::MarkerShape::Square),
            (false, egui_plot::MarkerShape::Circle),
        ] {
            {
                let name = liveplot::TraceRef("sig".into());
                if let Some(tr) = bed.harness.state_mut().traces_data.get_trace_mut(&name) {
                    tr.look.show_points = sp;
                    tr.look.marker = marker;
                }
            }
            steps(&mut bed, 6);
            assert!(
                scope(&bed, 0).rendered_this_frame,
                "renders with marker (vol={vol}, sp={sp}, {marker:?})"
            );
        }
    }
}

/// T-RND-bg: dark/light visuals keep the frame healthy on both paths
/// (the surface picks up `ui.visuals().dark_mode` + `panel_fill`).
#[test]
fn parity_dark_light_background() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        for v in [egui::Visuals::dark(), egui::Visuals::light()] {
            bed.harness.ctx.set_visuals(v.clone());
            steps(&mut bed, 6);
            if vol && !vol_available(&mut bed, 0) {
                break;
            }
            assert!(
                scope(&bed, 0).rendered_this_frame,
                "renders in {:?} (vol={vol})",
                if v.dark_mode { "dark" } else { "light" }
            );
        }
    }
}

/// T-PIX-001: the trace's colored-pixel CENTROID inside the axes rect
/// must match between arms — proves the curve lands at the same
/// screen position (viewport + margin alignment), not just "some
/// pixels drawn". Counts needn't match — the two renderers
/// anti-alias/width differently.
#[test]
fn parity_trace_pixel_centroid() {
    let mut outs = Vec::new();
    for vol in [false, true] {
        let Some(mut bed) = wgpu_bed() else { return };
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        {
            let s = scope_mut(&mut bed, 0);
            s.volcano_enabled = vol;
            // Pause → deterministic snapshot; the async volcano worker
            // otherwise sees a different sine phase than egui's
            // synchronous render (legitimate lag, not a bug).
            s.paused = true;
        }
        steps(&mut bed, 12);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        let ax = axes_rect(&bed, 0);
        let Ok(img) = bed.harness.render() else {
            eprintln!("no wgpu frame — skipping pixel test");
            return;
        };
        let (x0, y0) = (ax.left().max(0.0) as u32 + 2, ax.top().max(0.0) as u32 + 2);
        let (x1, y1) = (
            (ax.right() as u32).min(img.width().saturating_sub(2)),
            (ax.bottom() as u32).min(img.height().saturating_sub(2)),
        );
        let (mut n, mut sx, mut sy) = (0u64, 0u64, 0u64);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = img.get_pixel(x, y).0;
                let mx = *p.iter().max().unwrap() as i32;
                let mn = *p.iter().min().unwrap() as i32;
                if mx - mn > 30 {
                    n += 1;
                    sx += x as u64;
                    sy += y as u64;
                }
            }
        }
        assert!(n > 100, "trace pixels present (vol={vol}), got {n}");
        outs.push((n, sx as f64 / n as f64, sy as f64 / n as f64));
    }
    if outs.len() == 2 {
        let ((n0, cx0, cy0), (n1, cx1, cy1)) = (outs[0], outs[1]);
        // x centroid must align tightly — margins are symmetric L/R
        // on both paths. y carries the KNOWN margin-model difference:
        // egui maps data to the full widget rect (labels inside),
        // volcano insets it by the mpl margins (W-21 tracks exact
        // edge-to-edge parity) — allow the margin-order tolerance.
        assert!(
            (cx1 - cx0).abs() < 12.0,
            "x centroid aligns: egui={cx0:.1} vol={cx1:.1}"
        );
        assert!(
            (cy1 - cy0).abs() < 40.0,
            "y centroid within margin model: egui={cy0:.1} vol={cy1:.1}"
        );
        assert!(
            (n1 as f64 - n0 as f64).abs() < n0 as f64 * 0.6,
            "pixel counts similar: {n0} vs {n1}"
        );
    }
}

/// T-PAUSE-005: pausing must freeze the RENDERED data (snapshot), not
/// just the autoscroll — new points arriving while paused must not
/// shift what the volcano path uploads (reported bug: "stop only
/// stops autoscroll, doesn't keep the data").
#[test]
fn parity_pause_keeps_rendered_snapshot() {
    for vol in [false, true] {
        let mut bed = make_test_bed();
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 0.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 15);
        if vol && !vol_available(&mut bed, 0) {
            continue;
        }
        scope_mut(&mut bed, 0).paused = true;
        steps(&mut bed, 10);
        let (xb0, yb0) = view_bounds(&bed, 0);
        // Feed MORE data while paused — the drawn data must not move.
        feed_sine(&bed, "sig", 2000, 1.0, 100.0, 100.0);
        steps(&mut bed, 10);
        let (xb1, yb1) = view_bounds(&bed, 0);
        let tol = (xb0[1] - xb0[0]).abs().max(1e-9) * 0.05;
        assert!(
            (xb1[0] - xb0[0]).abs() < tol
                && (xb1[1] - xb0[1]).abs() < tol
                && (yb1[0] - yb0[0]).abs() < tol
                && (yb1[1] - yb0[1]).abs() < tol,
            "paused view frozen despite new data (vol={vol}): {xb0:?}->{xb1:?}"
        );
        // And the scope is still rendering the snapshot, not blank.
        assert!(scope(&bed, 0).rendered_this_frame, "renders (vol={vol})");
    }
}

// ---------- performance comparison (opt-in, Linux /proc) ----------

fn proc_cpu_ms() -> u64 {
    // utime+stime in clock ticks (fields 14,15); HZ=100 on Linux.
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let f: Vec<&str> = s.split(' ').collect();
    let u: u64 = f.get(13).and_then(|x| x.parse().ok()).unwrap_or(0);
    let st: u64 = f.get(14).and_then(|x| x.parse().ok()).unwrap_or(0);
    (u + st) * 10
}

/// CPU of the calling thread only (render workers excluded) — splits
/// UI-side work from render-worker work in the volcano path.
/// `/proc/thread-self` resolves to the calling thread's task dir.
fn main_thread_cpu_ms() -> u64 {
    let s = std::fs::read_to_string("/proc/thread-self/stat").unwrap_or_default();
    let f: Vec<&str> = s.split(' ').collect();
    let u: u64 = f.get(13).and_then(|x| x.parse().ok()).unwrap_or(0);
    let st: u64 = f.get(14).and_then(|x| x.parse().ok()).unwrap_or(0);
    (u + st) * 10
}

fn proc_rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

struct BenchRow {
    label: String,
    frames: u64,
    wall_ms: u128,
    cpu_ms: u64,
    rss_delta_kb: i64,
    rendered: u64,
    dropped: u64,
}

/// CPU/RSS/frame-time comparison of egui_plot vs VolcanoPlot.
/// Three workloads: BASE (4x20k pts) PAUSED (static view — isolates
/// render cost) + LIVE (data churn — worst case), and HEAVY LIVE
/// (8x200k pts — where egui CPU tessellation dominates and the GPU
/// path is supposed to win).
/// Run: cargo test --features volcano --test e2e -- --ignored bench_render_perf
#[test]
#[ignore]
fn bench_render_perf() {
    const FRAMES: usize = 120;
    let mut rows = Vec::new();

    for (traces, points, paused) in [
        (4usize, 20_000usize, true),
        (4, 20_000, false),
        (8, 200_000, true),
        (8, 200_000, false),
    ] {
        for vol in [false, true] {
            let mut bed = make_test_bed();
            for i in 0..traces {
                feed_sine(&bed, &format!("sig{i}"), points, 1.0 + i as f64, 100.0, 0.0);
            }
            {
                let s = scope_mut(&mut bed, 0);
                s.volcano_enabled = vol;
                s.paused = paused;
            }
            steps(&mut bed, 15); // warm-up: init, JIT, allocators
            let rss0 = proc_rss_kb();
            let cpu0 = proc_cpu_ms();
            let mcpu0 = main_thread_cpu_ms();
            let t0 = std::time::Instant::now();
            let mut rss_peak = rss0;
            for f in 0..FRAMES {
                if !paused && f % 4 == 0 {
                    feed_sine(&bed, "sig0", 800, 1.0, 100.0, 100.0 + f as f64 * 8.0);
                }
                bed.harness.step();
                // Pace frames ~1ms so the async volcano worker can
                // actually complete renders — a bare loop starves it
                // (frames_rendered would read ~1).
                std::thread::sleep(std::time::Duration::from_millis(1));
                rss_peak = rss_peak.max(proc_rss_kb());
            }
            let wall = t0.elapsed();
            let cpu = proc_cpu_ms() - cpu0;
            let mcpu = main_thread_cpu_ms() - mcpu0;
            let s = scope(&bed, 0);
            let (rd, dp) = if let Some(surf) = s.volcano_surface.as_ref() {
                eprintln!(
                    "    [{}] main_cpu={}ms worker_cpu={}ms dispatches={} sig_frames={} busy_skips={} uploads={} slides={}",
                    if vol { "volcano" } else { "egui" },
                    mcpu,
                    cpu as i64 - mcpu as i64,
                    surf.dbg_dispatches,
                    surf.dbg_sig_frames,
                    surf.dbg_busy_skips,
                    surf.dbg_uploads,
                    surf.dbg_slides,
                );
                (surf.frames_rendered, surf.dropped_frames as u64)
            } else {
                (0, 0)
            };
            rows.push(BenchRow {
                label: format!(
                    "{} {} {}x{}k",
                    if vol { "volcano" } else { "egui   " },
                    if paused { "paused" } else { "live  " },
                    traces,
                    points / 1000,
                ),
                frames: FRAMES as u64,
                wall_ms: wall.as_millis(),
                cpu_ms: cpu,
                rss_delta_kb: rss_peak as i64 - rss0 as i64,
                rendered: rd,
                dropped: dp,
            });
        }
    }

    eprintln!("\n========= render-path perf ({} frames) =========", FRAMES);
    eprintln!(
        "{:<22} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "path", "wall", "ms/frame", "cpu", "ms/frame", "RSS Δ", "rendered"
    );
    eprintln!(
        "{:<22} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "", "(ms)", "wall", "(ms)", "cpu", "(KB)", "dropped"
    );
    for r in &rows {
        eprintln!(
            "{:<22} {:>9} {:>9.2} {:>9} {:>9.2} {:>9} {:>3}/{}",
            r.label,
            r.wall_ms,
            r.wall_ms as f64 / r.frames as f64,
            r.cpu_ms,
            r.cpu_ms as f64 / r.frames as f64,
            r.rss_delta_kb,
            r.rendered,
            r.dropped
        );
    }
    eprintln!("=================================================================\n");
}

/// Screenshot probe: epoch-timestamped trace through both renderers.
#[test]
fn probe_epoch_screenshots() {
    for vol in [false, true] {
        let Some(mut bed) = wgpu_bed() else { return };
        feed_sine(&bed, "sig", 20000, 8.0, 1000.0, 1_790_000_000.0);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        steps(&mut bed, 30);
        let Ok(img) = bed.harness.render() else { return };
        let path = format!("/tmp/epoch_vol{vol}.png");
        img.save(&path).unwrap();
        eprintln!("saved {path}");
    }
}


/// Screenshot probe: live-streamed epoch data (fft_demo-like), both
/// renderers, captures at matched feed positions.
#[test]
fn probe_live_epoch_screenshots() {
    use crate::signals::{gen, SignalSpec};
    for vol in [false, true] {
        let Some(mut bed) = wgpu_bed() else { return };
        let t_two = bed.sink.create_trace("two_tone", None);
        let t_swe = bed.sink.create_trace("swept", None);
        scope_mut(&mut bed, 0).volcano_enabled = vol;
        let mut shot = 0;
        let spec_two = SignalSpec::SumOfSines(vec![(5.0, 1.0), (50.0, 0.5)]);
        let spec_swe = SignalSpec::sine(25.0);
        // 70 chunks x 200 pts @ 1kHz = 14 s of stream — past the 10k
        // store cap, so ring slides + the GPU envelope path both engage.
        // Screenshots at ~6 s intervals.
        for chunk in 0..70 {
            let t0 = 1_790_000_000.0 + chunk as f64 * 0.2;
            bed.sink
                .send_points(&t_two, gen(&spec_two, 200, 1000.0, t0))
                .unwrap();
            bed.sink
                .send_points(&t_swe, gen(&spec_swe, 200, 1000.0, t0))
                .unwrap();
            steps(&mut bed, 3);
            if (chunk + 1) % 22 == 0 {
                let Ok(img) = bed.harness.render() else { return };
                shot += 1;
                let path = format!("/tmp/live_vol{vol}_s{shot}.png");
                img.save(&path).unwrap();
                eprintln!("saved {path}");
            }
        }
    }
}
