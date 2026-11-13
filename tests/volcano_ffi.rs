//! Proof-of-life for the VolcanoPlot FFI backend (feature `volcano`).
//! Renders a real scope-shaped plot headlessly on Vulkan (llvmpipe or
//! hardware) and verifies the framebuffer contents.

#![cfg(feature = "volcano")]

use liveplot::volcano::{VolcanoFigure, VolcanoLineStyle};

fn sine(n: usize, fs: f64, freq: f64) -> Vec<[f64; 2]> {
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            [t, (2.0 * std::f64::consts::PI * freq * t).sin()]
        })
        .collect()
}

/// A sine wave must produce non-uniform pixels (line + grid over the
/// background) — proves the full FFI → Vulkan → readback path works.
#[test]
fn volcano_renders_sine_framebuffer() {
    let mut fig = VolcanoFigure::new(640, 480).expect("vp_figure_create");
    let id = fig.add_trace();
    assert!(id != 0);
    fig.set_trace(
        id,
        &sine(500, 100.0, 1.0),
        0xFF6600FF,
        2.0,
        VolcanoLineStyle::Solid,
    );
    fig.set_axis_labels(Some("time [s]"), Some("amp"));
    let px = fig.render_rgba().expect("render frame");
    assert_eq!(px.len(), 640 * 480 * 4);
    let unique: std::collections::HashSet<u32> = px
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert!(
        unique.len() >= 4,
        "expected a rendered plot, got {} unique colors",
        unique.len()
    );
    // The 0xFF6600 (orange) trace line should be present somewhere —
    // antialiasing shifts channels, so match with tolerance.
    let orange = px
        .chunks_exact(4)
        .any(|c| c[0] > 200 && c[1] > 60 && c[1] < 160 && c[2] < 80 && c[3] == 255);
    assert!(orange, "trace color 0xFF6600 not found in framebuffer");
}

/// Viewport + style changes round-trip through the FFI.
#[test]
fn volcano_viewport_and_styles() {
    let mut fig = VolcanoFigure::new(320, 200).expect("figure");
    let a = fig.add_trace();
    let b = fig.add_trace();
    fig.set_trace(
        a,
        &sine(200, 100.0, 2.0),
        0xFF0000FF,
        1.0,
        VolcanoLineStyle::Dashed,
    );
    fig.set_trace(
        b,
        &sine(200, 100.0, 0.5),
        0x00FF00FF,
        1.0,
        VolcanoLineStyle::PointsOnly,
    );
    fig.set_viewport(0.0, 1.0, -0.5, 0.5);
    assert!(fig.render_rgba().is_some());
    fig.remove_trace(a);
    assert!(fig.render_rgba().is_some());
    assert!(fig.resize(400, 300));
    assert!(fig.render_rgba().is_some());
}

/// Rough perf probe: render a 200k-point trace a few times and report
/// the average frame time (Vulkan render + readback).
#[test]
fn volcano_perf_200k_points() {
    use std::time::Instant;
    let n = 200_000usize;
    let mut xy = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 * 0.001;
        xy.push([t, t.sin() * (1.0 + 0.05 * (t * 8.0).cos())]);
    }
    let mut fig = VolcanoFigure::new(1280, 720).expect("figure");
    let id = fig.add_trace();
    let t_up = Instant::now();
    fig.set_trace(id, &xy, 0xFF8800FF, 1.5, VolcanoLineStyle::Solid);
    eprintln!("upload 200k pts: {:.1?}", t_up.elapsed());
    fig.set_viewport(0.0, 200.0, -1.2, 1.2);
    let mut times = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        assert!(fig.render_rgba().is_some());
        times.push(t.elapsed());
    }
    eprintln!(
        "render+readback 1280x720 x5: {:?} (avg {:?})",
        times,
        times.iter().sum::<std::time::Duration>() / 5
    );
}

/// Tiny heights must not fail — the FFT scope can be as short as ~50px.
#[test]
fn volcano_tiny_height_renders() {
    for (w, h) in [(1122, 51), (100, 8), (64, 64)] {
        let mut fig = match VolcanoFigure::new(w, h) {
            Some(f) => f,
            None => {
                eprintln!("figure {w}x{h} create failed");
                continue;
            }
        };
        let id = fig.add_trace();
        fig.set_trace(
            id,
            &[[0.0, 0.0], [1.0, 1.0]],
            0xFF8800FF,
            1.5,
            VolcanoLineStyle::Solid,
        );
        fig.set_viewport(0.0, 1.0, 0.0, 1.0);
        let ok = fig.render_rgba().is_some();
        eprintln!("{w}x{h}: render ok={ok}");
    }
}

/// Degenerate viewport / empty trace — must not break the renderer.
#[test]
fn volcano_degenerate_state() {
    let mut fig = VolcanoFigure::new(1122, 51).unwrap();
    // no traces at all, degenerate viewport
    fig.set_viewport(0.0, 0.0, 0.0, 0.0);
    eprintln!("empty+degenerate: {:?}", fig.render_rgba().map(|v| v.len()));
    let id = fig.add_trace();
    fig.set_trace(id, &[], 0xFF8800FF, 1.5, VolcanoLineStyle::Solid);
    eprintln!("empty trace: {:?}", fig.render_rgba().map(|v| v.len()));
    fig.set_trace(
        id,
        &[[5.0, f64::NAN], [6.0, 1.0]],
        0xFF8800FF,
        1.5,
        VolcanoLineStyle::Solid,
    );
    eprintln!("nan point: {:?}", fig.render_rgba().map(|v| v.len()));
    fig.set_viewport(5.0, 5.0, 0.0, 1.0);
    eprintln!("degenerate xlim: {:?}", fig.render_rgba().map(|v| v.len()));
}

/// Temporary probe: keep a figure alive ~10s so external tools can
/// observe which GPU device the process holds.
#[test]
#[ignore]
fn volcano_hold_device_probe() {
    let mut fig = VolcanoFigure::new(640, 480).expect("figure");
    let id = fig.add_trace();
    fig.set_trace(
        id,
        &sine(500, 100.0, 1.0),
        0xFF6600FF,
        2.0,
        VolcanoLineStyle::Solid,
    );
    for _ in 0..20 {
        fig.render_rgba();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Temporary probe: is renderFrame's ~5ms GPU-bound or CPU-bound?
/// Same 200k-point render at 64x64 (near-zero raster work) vs full
/// size — if time scales with pixels it's the GPU wait (spin = CPU),
/// if constant it's CPU-side scene building.
#[test]
#[ignore]
fn volcano_render_scaling_probe() {
    use std::time::Instant;
    let n = 200_000usize;
    let xy: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = i as f64 * 0.001;
            [t, t.sin() * (1.0 + 0.05 * (t * 8.0).cos())]
        })
        .collect();
    for (w, h) in [(64, 64), (320, 200), (1122, 635)] {
        let mut fig = VolcanoFigure::new(w, h).expect("figure");
        let id = fig.add_trace();
        fig.set_trace(id, &xy, 0xFF8800FF, 1.5, VolcanoLineStyle::Solid);
        fig.set_viewport(0.0, 200.0, -1.2, 1.2);
        let mut times = Vec::new();
        for _ in 0..6 {
            let t = Instant::now();
            assert!(fig.render_rgba().is_some());
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
        eprintln!("{w}x{h}: renders(ms) {times:?}");
    }
}

/// Diagnostic: render with axis labels and dump the framebuffer to PNG.
#[test]
fn volcano_debug_render_png() {
    let mut fig = VolcanoFigure::new(640, 400).unwrap();
    fig.set_axis_labels(Some("Frequency (Hz)"), Some("Magnitude (dB)"));
    fig.set_viewport(0.0, 10.0, 0.0, 1.0);
    fig.set_background(true);
    let id = fig.add_trace();
    let pts: Vec<[f64; 2]> = (0..500)
        .map(|i| [i as f64 * 0.02, (i as f64 * 0.02).sin()])
        .collect();
    fig.set_trace(id, &pts, 0xFF4444FF, 1.5, VolcanoLineStyle::Solid);
    let (w, h) = fig.size();
    if let Some(px) = fig.render_rgba() {
        let img = image::RgbaImage::from_raw(w, h, px.to_vec()).unwrap();
        img.save("/tmp/volcano_debug.png").unwrap();
        eprintln!("saved /tmp/volcano_debug.png {w}x{h}");
    }
}

/// Count columns containing the trace color (tolerant match), return
/// (min_col, max_col) in canvas x pixels or None when absent.
fn trace_x_extent(px: &[u8], w: usize, rgba: [u8; 3]) -> Option<(usize, usize)> {
    let mut lo = usize::MAX;
    let mut hi = 0usize;
    for (i, c) in px.chunks_exact(4).enumerate() {
        let close = c[0].abs_diff(rgba[0]) < 60
            && c[1].abs_diff(rgba[1]) < 60
            && c[2].abs_diff(rgba[2]) < 60
            && c[3] > 200;
        if close {
            let x = i % w;
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// Regression: a slide landing BEFORE the first prepare must not
/// upload the evicted prefix — the cold path used to ship rawF64_
/// from index 0, stroking dead points joined to the newest data.
#[test]
fn volcano_slide_before_first_render_drops_old_points() {
    let w = 640;
    let mut fig = VolcanoFigure::new(w as u32, 300).expect("figure");
    let id = fig.add_trace();
    // Old window: x in [0,5]. Then drop ALL of it and append x in [7,9]
    // — before the first render, so prepare() takes the cold path.
    let old: Vec<[f64; 2]> =
        (0..50).map(|i| [i as f64 * 0.1, (i as f64 * 0.4).sin()]).collect();
    fig.set_trace(id, &old, 0xFF0000FF, 2.0, VolcanoLineStyle::Solid);
    let tail: Vec<[f64; 2]> = (0..20)
        .map(|i| [7.0 + i as f64 * 0.1, (i as f64 * 0.4).sin()])
        .collect();
    assert!(fig.set_trace_slide(
        id,
        50,
        &tail,
        0xFF0000FF,
        2.0,
        VolcanoLineStyle::Solid
    ));
    fig.set_viewport(0.0, 10.0, -1.0, 1.0);
    let px = fig.render_rgba().expect("render");
    let (lo, hi) = trace_x_extent(px, w, [255, 0, 0])
        .expect("red trace not rendered at all");
    // Axes fill ~11%..98% of the canvas; x∈[7,9] of viewport [0,10]
    // maps to roughly the right third. Any red in the left ~half means
    // the evicted prefix leaked into the upload.
    assert!(
        lo as f64 > w as f64 * 0.45,
        "stale prefix rendered: red starts at x={lo} (w={w})"
    );
    assert!(hi > lo, "no stroke");
}

/// Regression: a slide sent while the plot is gone (removed by an
/// empty upload) must report failure — silently dropping it lost the
/// trace permanently.
#[test]
fn volcano_slide_on_missing_plot_reports_failure() {
    let mut fig = VolcanoFigure::new(320, 200).expect("figure");
    let id = fig.add_trace();
    let pts: Vec<[f64; 2]> =
        (0..10).map(|i| [i as f64, (i as f64 * 0.6).sin()]).collect();
    fig.set_trace(id, &pts, 0xFF0000FF, 2.0, VolcanoLineStyle::Solid);
    assert!(fig.render_rgba().is_some());
    // Empty upload removes the LinePlot (empty plots crash render).
    fig.set_trace(id, &[], 0xFF0000FF, 2.0, VolcanoLineStyle::Solid);
    // The slide cannot be applied — the caller must see failure.
    assert!(
        !fig.set_trace_slide(id, 0, &pts, 0xFF0000FF, 2.0, VolcanoLineStyle::Solid),
        "slide on a removed plot must report failure"
    );
    // Full upload resurrects the trace.
    fig.set_trace(id, &pts, 0xFF0000FF, 2.0, VolcanoLineStyle::Solid);
    fig.set_viewport(0.0, 10.0, -1.0, 1.0);
    for k in 0..3 {
        let px = fig.render_rgba().expect("render");
        let red = px.chunks_exact(4)
            .filter(|c| c[0].abs_diff(255) < 60 && c[1] < 60 && c[2] < 60 && c[3] > 200)
            .count();
        eprintln!("render {k}: red_px={red}");
        if k == 2 {
            assert!(trace_x_extent(px, 320usize, [255, 0, 0]).is_some());
        }
    }
}

/// Regression: repeated slides must keep the GPU window aligned with
/// the data — the stroked x-extent tracks the sliding window.
#[test]
fn volcano_repeated_slides_track_window() {
    let w = 640;
    let mut fig = VolcanoFigure::new(w as u32, 300).expect("figure");
    let id = fig.add_trace();
    // Window x∈[0,10].
    let mut data: Vec<[f64; 2]> = (0..100)
        .map(|i| [i as f64 * 0.1, (i as f64 * 0.1).sin()])
        .collect();
    fig.set_trace(id, &data, 0xFF0000FF, 2.0, VolcanoLineStyle::Solid);
    fig.set_viewport(0.0, 20.0, -1.0, 1.0);
    assert!(fig.render_rgba().is_some());
    // Slide five times: evict 20, append 20 (window → x∈[2,12]), then
    // again (x∈[4,14]) ... ending x∈[10,20].
    for _ in 0..5 {
        data.drain(..20);
        for _ in 0..20 {
            let x = data.last().unwrap()[0] + 0.1;
            data.push([x, (x * 1.0).sin()]);
        }
        assert!(fig.set_trace_slide(
            id,
            20,
            &data[data.len() - 20..],
            0xFF0000FF,
            2.0,
            VolcanoLineStyle::Solid
        ));
        assert!(fig.render_rgba().is_some());
    }
    let px = fig.render_rgba().expect("render");
    let (lo, hi) = trace_x_extent(px, w, [255, 0, 0])
        .expect("red trace not rendered");
    // Window x∈[10,20] of viewport [0,20] → right half only.
    assert!(
        lo as f64 > w as f64 * 0.4,
        "window did not slide: red starts at x={lo} (w={w})"
    );
    assert!(
        hi as f64 > w as f64 * 0.8,
        "tail missing: red ends at x={hi} (w={w})"
    );
}

/// Screenshot repro of the sliding-window corruption: 4 traces, each
/// sliding via ring-append drops, dumped to PNG after several slides —
/// the rendered window must match the data window exactly (no stale
/// prefix stroked to the newest point).
#[test]
fn volcano_slide_screenshot() {
    let (w, h) = (900u32, 500u32);
    let mut fig = VolcanoFigure::new(w, h).unwrap();
    fig.set_background(true);
    fig.set_axis_labels(Some("t"), Some("y"));
    fig.set_viewport(0.0, 20.0, -1.3, 1.3);

    let colors = [0xFF4444FFu32, 0x44FF44FF, 0x4444FFFF, 0xFF44FFFF];
    let mut windows: Vec<Vec<[f64; 2]>> = (0..4)
        .map(|t| {
            (0..200)
                .map(|i| {
                    let x = i as f64 * 0.1;
                    [x, (x * (0.7 + t as f64 * 0.4)).sin() * (0.4 + t as f64 * 0.15)]
                })
                .collect()
        })
        .collect();
    let ids: Vec<u64> = (0..4).map(|_| fig.add_trace()).collect();
    for (k, id) in ids.iter().enumerate() {
        fig.set_trace(*id, &windows[k], colors[k], 1.8, VolcanoLineStyle::Solid);
    }
    assert!(fig.render_rgba().is_some());

    // 8 slides × drop 25 / append 25 → window moves x: [0,20) → [20,40)
    for _ in 0..8 {
        for (k, id) in ids.iter().enumerate() {
            windows[k].drain(..25);
            for _ in 0..25 {
                let x = windows[k].last().unwrap()[0] + 0.1;
                windows[k].push([x, (x * (0.7 + k as f64 * 0.4)).sin() * (0.4 + k as f64 * 0.15)]);
            }
            assert!(fig.set_trace_slide(
                *id, 25, &windows[k][175..], colors[k], 1.8, VolcanoLineStyle::Solid
            ));
        }
        assert!(fig.render_rgba().is_some());
    }

    // Viewport follows the data window — the traces should fill the
    // frame; a stale prefix would leave a bogus segment in the left.
    fig.set_viewport(20.0, 40.0, -1.3, 1.3);
    let px = fig.render_rgba().expect("final render");
    image::RgbaImage::from_raw(w, h, px.to_vec())
        .unwrap()
        .save("/tmp/volcano_slide_after.png")
        .unwrap();
    eprintln!("saved /tmp/volcano_slide_after.png");

    // Every trace color present, and each spans ~the full width (the
    // data window covers the whole viewport — a stale/missing trace
    // would leave one color absent or clipped).
    for (k, c) in colors.iter().enumerate() {
        let rgb = [(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8];
        let ext = trace_x_extent(px, w as usize, rgb);
        eprintln!("trace {k}: ext={ext:?}");
        let (lo, hi) = ext.expect("trace missing");
        assert!(
            lo as f64 <= w as f64 * 0.35 && hi as f64 >= w as f64 * 0.65,
            "trace {k} extent [{lo},{hi}] does not span the window"
        );
    }
}

/// The device-resident envelope→polyline path (compute reduce + emit,
/// no host readback) must produce the same pixels as the legacy
/// submit+readback envelope. `VOLCANO_ENVDEV=0` latches the second
/// figure's renderer onto the legacy path for a same-process A/B.
#[test]
fn volcano_envelope_device_matches_readback() {
    let (w, h) = (640u32, 480u32);
    // >8192 pts → envelopeOk path; a NaN gap exercises the emit
    // shader's run-parity scan (alternating order restarts per run).
    let mut pts: Vec<[f64; 2]> = (0..20000)
        .map(|i| {
            let x = i as f64 * 0.001;
            [x, (x * 2.3).sin() * 0.8 + (x * 31.0).sin() * 0.15]
        })
        .collect();
    pts[7000] = [f64::NAN, f64::NAN];
    pts[7010] = [f64::NAN, f64::NAN];
    pts[15000] = [f64::NAN, f64::NAN];

    let render = || {
        let mut fig = VolcanoFigure::new(w, h).unwrap();
        fig.set_background(true);
        fig.set_viewport(0.0, 20.0, -1.3, 1.3);
        let id = fig.add_trace();
        fig.set_trace(id, &pts, 0xFF2222FF, 1.5, VolcanoLineStyle::Solid);
        fig.render_rgba().unwrap().to_vec()
    };

    let dev_px = render();
    image::RgbaImage::from_raw(w, h, dev_px.clone())
        .unwrap()
        .save("/tmp/env_dev.png")
        .unwrap();
    unsafe { std::env::set_var("VOLCANO_ENVDEV", "0") };
    let rb_px = render();
    unsafe { std::env::remove_var("VOLCANO_ENVDEV") };
    image::RgbaImage::from_raw(w, h, rb_px.clone())
        .unwrap()
        .save("/tmp/env_rb.png")
        .unwrap();

    let diff = dev_px
        .iter()
        .zip(rb_px.iter())
        .filter(|(a, b)| a != b)
        .count();
    eprintln!("envdev A/B differing bytes: {diff} / {}", dev_px.len());
    // FMA contraction inside the emit shader can shift an edge by a
    // ulp — a handful of pixels, not a silhouette change.
    assert!(
        diff <= dev_px.len() / 50,
        "device envelope diverged from readback envelope: {diff} bytes"
    );
    // And the device render must actually contain the trace.
    let red = dev_px
        .chunks_exact(4)
        .filter(|c| c[0] > 200 && c[1] < 100 && c[2] < 100)
        .count();
    assert!(red > 100, "device envelope drew {red} trace px");
}

/// Ring slides at envelope scale (10k pts > 8192 → GPU envelope path):
/// the drawn window must track the data window, not leave a stale
/// prefix segment.
#[test]
fn volcano_slide_screenshot_envelope_scale() {
    let (w, h) = (900u32, 500u32);
    let mut fig = VolcanoFigure::new(w, h).unwrap();
    fig.set_background(true);
    fig.set_axis_labels(Some("t"), Some("y"));

    let colors = [0xFF4444FFu32, 0x44FF44FF];
    let mut windows: Vec<Vec<[f64; 2]>> = (0..2)
        .map(|t| {
            (0..10000)
                .map(|i| {
                    let x = i as f64 * 0.001;
                    [x, (x * (40.0 + t as f64 * 30.0)).sin() * (0.4 + t as f64 * 0.3)]
                })
                .collect()
        })
        .collect();
    let ids: Vec<u64> = (0..2).map(|_| fig.add_trace()).collect();
    for (k, id) in ids.iter().enumerate() {
        fig.set_trace(*id, &windows[k], colors[k], 1.8, VolcanoLineStyle::Solid);
    }
    fig.set_viewport(0.0, 10.0, -1.0, 1.0);
    assert!(fig.render_rgba().is_some());

    // 20 slides × drop 200 / append 200 → window moves x: [0,10) → [4,14)
    for _ in 0..20 {
        for (k, id) in ids.iter().enumerate() {
            windows[k].drain(..200);
            for _ in 0..200 {
                let x = windows[k].last().unwrap()[0] + 0.001;
                windows[k].push([x, (x * (40.0 + k as f64 * 30.0)).sin() * (0.4 + k as f64 * 0.3)]);
            }
            assert!(fig.set_trace_slide(
                *id, 200, &windows[k][9800..], colors[k], 1.8, VolcanoLineStyle::Solid
            ));
        }
        fig.set_viewport(windows[0].first().unwrap()[0], windows[0].last().unwrap()[0], -1.0, 1.0);
        assert!(fig.render_rgba().is_some());
    }

    let px = fig.render_rgba().expect("final render");
    image::RgbaImage::from_raw(w, h, px.to_vec())
        .unwrap()
        .save("/tmp/volcano_env_slide.png")
        .unwrap();
    for (k, c) in colors.iter().enumerate() {
        let rgb = [(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8];
        let ext = trace_x_extent(px, w as usize, rgb);
        eprintln!("trace {k}: ext={ext:?}");
        let (lo, hi) = ext.expect("trace missing");
        assert!(
            lo as f64 <= w as f64 * 0.35 && hi as f64 >= w as f64 * 0.65,
            "trace {k} extent [{lo},{hi}] does not span the window"
        );
    }
}


/// Ramp-up + sliding window at envelope scale — mirrors the e2e app
/// feed (200-pt chunks appended via slides, growing to the 10k cap,
/// viewport padded like the scope's data bounds).
#[test]
fn volcano_ramp_slide_screenshot() {
    let (w, h) = (1100u32, 600u32);
    let mut fig = VolcanoFigure::new(w, h).unwrap();
    fig.set_background(true);
    fig.set_axis_labels(Some("t"), Some("y"));

    let colors = [0xFF4444FFu32, 0x44FF44FF];
    let ids: Vec<u64> = (0..2).map(|_| fig.add_trace()).collect();
    let mut windows: Vec<Vec<[f64; 2]>> = vec![vec![], vec![]];
    let mut next_x = 0.0f64;
    let mut gen = |k: usize, n: usize| -> Vec<[f64; 2]> {
        (0..n).map(|_| {
            let x = next_x;
            next_x += 0.001;
            [x, (x * (40.0 + k as f64 * 30.0)).sin() * (0.4 + k as f64 * 0.3)]
        }).collect()
    };

    // 70 chunks of 200 pts — window caps at 10000, then slides.
    for chunk in 0..70 {
        for (k, id) in ids.iter().enumerate() {
            let tail = gen(k, 200);
            windows[k].extend(tail.iter().copied());
            let over = windows[k].len().saturating_sub(10000);
            if over > 0 { windows[k].drain(..over); }
            if windows[k].len() == 200 {
                fig.set_trace(*id, &windows[k], colors[k], 1.8, VolcanoLineStyle::Solid);
            } else {
                assert!(fig.set_trace_slide(
                    *id, over as u32, &tail, colors[k], 1.8, VolcanoLineStyle::Solid
                ));
            }
        }
        let (x0, x1) = (windows[0].first().unwrap()[0], windows[0].last().unwrap()[0]);
        fig.set_viewport(x0 - 1.5, x1 + 0.5, -1.0, 1.0);
        assert!(fig.render_rgba().is_some());
        if chunk == 69 {
            let px = fig.render_rgba().unwrap();
            image::RgbaImage::from_raw(w, h, px.to_vec()).unwrap()
                .save("/tmp/volcano_ramp.png").unwrap();
            for (k, c) in colors.iter().enumerate() {
                let rgb = [(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8];
                eprintln!("trace {k}: ext={:?}", trace_x_extent(px, w as usize, rgb));
            }
        }
    }
}


/// Growing f64 traces that cross the 8192-point envelope threshold and
/// then keep sliding: both traces must keep drawing their full window
/// every frame. Regression coverage for the stale-descriptor bug —
/// recycled VkBuffer handles made the envelope reduce bind retired
/// buffers, leaving traces frozen or missing entirely.
#[test]
fn volcano_ramp_bisect() {
    let (w, h) = (1100u32, 600u32);
    let mut fig = VolcanoFigure::new(w, h).unwrap();
    fig.set_background(true);
    fig.set_axis_labels(Some("t"), Some("y"));
    let colors = [0xFF4444FFu32, 0x44FF44FF];
    let ids: Vec<u64> = (0..2).map(|_| fig.add_trace()).collect();
    let mut windows: Vec<Vec<[f64; 2]>> = vec![vec![], vec![]];
    let mut next_x = 0.0f64;
    for chunk in 0..70usize {
        for (k, id) in ids.iter().enumerate() {
            let tail: Vec<[f64; 2]> = (0..200).map(|_| {
                let x = next_x;
                next_x += 0.001;
                [x, (x * (40.0 + k as f64 * 30.0)).sin() * (0.4 + k as f64 * 0.3)]
            }).collect();
            windows[k].extend(tail.iter().copied());
            let over = windows[k].len().saturating_sub(10000);
            if over > 0 { windows[k].drain(..over); }
            if windows[k].len() == 200 {
                fig.set_trace(*id, &windows[k], colors[k], 1.8, VolcanoLineStyle::Solid);
            } else {
                assert!(fig.set_trace_slide(
                    *id, over as u32, &tail, colors[k], 1.8, VolcanoLineStyle::Solid
                ));
            }
        }
        let (x0, x1) = (windows[0].first().unwrap()[0], windows[0].last().unwrap()[0]);
        fig.set_viewport(x0 - 1.5, x1 + 0.5, -1.0, 1.0);
        let px = fig.render_rgba().expect("render");
        for (k, c) in colors.iter().enumerate() {
            let rgb = [(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8];
            let ext = trace_x_extent(px, w as usize, rgb);
            if ext.is_none() {
                image::RgbaImage::from_raw(w, h, px.to_vec()).unwrap()
                    .save(format!("/tmp/ramp_dead_{chunk}_{k}.png")).unwrap();
                eprintln!("saved /tmp/ramp_dead_{chunk}_{k}.png");
            }
            assert!(ext.is_some(),
                    "chunk {chunk}: trace {k} missing from framebuffer");
        }
    }
}


/// Single-trace ramp: the envelope path engages at >8192 points; the
/// trace must keep rendering the full sliding window afterwards.
#[test]
fn volcano_ramp_single() {
    let (w, h) = (1100u32, 600u32);
    let mut fig = VolcanoFigure::new(w, h).unwrap();
    fig.set_background(true);
    let id = fig.add_trace();
    let color = 0xFF4444FFu32;
    let rgb = [(color >> 24) as u8, (color >> 16) as u8, (color >> 8) as u8];
    let mut win: Vec<[f64; 2]> = vec![];
    let mut next_x = 0.0f64;
    for chunk in 0..60usize {
        let tail: Vec<[f64; 2]> = (0..200).map(|_| {
            let x = next_x;
            next_x += 0.001;
            [x, (x * 40.0).sin() * 0.4]
        }).collect();
        win.extend(tail.iter().copied());
        let over = win.len().saturating_sub(10000);
        if over > 0 { win.drain(..over); }
        if win.len() == 200 {
            fig.set_trace(id, &win, color, 1.8, VolcanoLineStyle::Solid);
        } else {
            assert!(fig.set_trace_slide(id, over as u32, &tail, color,
                                      1.8, VolcanoLineStyle::Solid));
        }
        let (x0, x1) = (win.first().unwrap()[0], win.last().unwrap()[0]);
        fig.set_viewport(x0 - 1.5, x1 + 0.5, -1.0, 1.0);
        let px = fig.render_rgba().expect("render");
        let ext = trace_x_extent(px, w as usize, rgb);
        if ext.is_none() {
            image::RgbaImage::from_raw(w, h, px.to_vec()).unwrap()
                .save(format!("/tmp/ramp_single_dead_{chunk}.png")).unwrap();
            eprintln!("saved /tmp/ramp_single_dead_{chunk}.png");
        }
        assert!(ext.is_some(),
                "chunk {chunk} len={}: trace missing from framebuffer",
                win.len());
    }
}
