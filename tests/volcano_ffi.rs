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
    fig.set_trace(id, &sine(500, 100.0, 1.0), 0xFF6600FF, 2.0, VolcanoLineStyle::Solid);
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
    let orange = px.chunks_exact(4).any(|c| {
        c[0] > 200 && c[1] > 60 && c[1] < 160 && c[2] < 80 && c[3] == 255
    });
    assert!(orange, "trace color 0xFF6600 not found in framebuffer");
}

/// Viewport + style changes round-trip through the FFI.
#[test]
fn volcano_viewport_and_styles() {
    let mut fig = VolcanoFigure::new(320, 200).expect("figure");
    let a = fig.add_trace();
    let b = fig.add_trace();
    fig.set_trace(a, &sine(200, 100.0, 2.0), 0xFF0000FF, 1.0, VolcanoLineStyle::Dashed);
    fig.set_trace(b, &sine(200, 100.0, 0.5), 0x00FF00FF, 1.0, VolcanoLineStyle::PointsOnly);
    fig.set_viewport(0.0, 1.0, -0.5, 0.5);
    assert!(fig.render_rgba().is_some());
    fig.remove_trace(a);
    assert!(fig.render_rgba().is_some());
    assert!(fig.resize(400, 300));
    assert!(fig.render_rgba().is_some());
}
