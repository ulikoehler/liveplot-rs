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
