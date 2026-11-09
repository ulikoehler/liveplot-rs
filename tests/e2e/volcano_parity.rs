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
        drag(&mut bed, c, egui::pos2(c.x + 80.0, c.y), egui::PointerButton::Primary);
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
        assert!(s.rendered_this_frame, "XY scope must render (volcano={vol})");
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
        let data = find_panel::<FftPanel>(&bed)
            .unwrap()
            .scope_ui
            .get_data();
        assert!(data.rendered_this_frame, "FFT scope renders (volcano={vol})");
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
        let (x1, y1) = ((rect[2] as u32).min(img.width() - 5), (rect[3] as u32).min(img.height() - 5));
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
        let lr = egui::Rect::from_min_max(
            egui::pos2(lr[0], lr[1]),
            egui::pos2(lr[2], lr[3]),
        );
        let Ok(img) = bed.harness.render() else { return };
        let (x0, y0) = (lr.left() as u32, lr.top() as u32);
        let (x1, y1) = ((lr.right() as u32).min(img.width() - 1), (lr.bottom() as u32).min(img.height() - 1));
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
            .send_points(&trace, crate::signals::gen(&SignalSpec::sine(1.0), 2000, 100.0, 0.0))
            .unwrap();
        for _ in 0..20 {
            bed.harness.step();
        }
        let scopes = bed
            .harness
            .state()
            .main_panel
            .liveplot_panel
            .get_data();
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
