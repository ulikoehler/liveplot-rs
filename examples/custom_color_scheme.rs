//! Example: Custom user-defined color scheme
//!
//! What it demonstrates
//! - Creating a custom color scheme with a user palette and visuals.
//! - Passing a custom scheme to LivePlotConfig.
//!
//! How to run
//! ```bash
//! cargo run --example custom_color_scheme
//! ```

use eframe::egui::{Color32, Pos2, Visuals};
use liveplot::config::CustomColorScheme;
use liveplot::{channel_plot, run_liveplot, ColorScheme, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Define a custom palette (8 rainbow colors)
    let palette = vec![
        Color32::from_rgb(255, 0, 0),     // red
        Color32::from_rgb(255, 127, 0),   // orange
        Color32::from_rgb(255, 255, 0),   // yellow
        Color32::from_rgb(0, 255, 0),     // green
        Color32::from_rgb(0, 0, 255),     // blue
        Color32::from_rgb(75, 0, 130),    // indigo
        Color32::from_rgb(148, 0, 211),   // violet
        Color32::from_rgb(255, 192, 203), // pink for extra flair
    ];

    // Optionally customize visuals (here: yellow background, black text)
    let mut visuals = Visuals::dark();
    // start from dark to avoid bright widgets by default
    visuals.panel_fill = Color32::YELLOW;
    visuals.window_fill = Color32::YELLOW;
    visuals.extreme_bg_color = Color32::YELLOW;
    visuals.faint_bg_color = Color32::YELLOW;
    visuals.override_text_color = Some(Color32::BLACK);

    let custom_scheme = CustomColorScheme {
        visuals: Some(visuals),
        palette,
        label: Some("My Custom Scheme".to_string()),
    };

    let scheme = ColorScheme::Custom(custom_scheme);

    let (sink, rx) = channel_plot();
    let tr_sine = sink.create_trace("sine", None);
    let tr_cos = sink.create_trace("cosine", None);

    // Producer: 1 kHz, 3 Hz sine + cosine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0;
        const F_HZ: f64 = 3.0;
        let dt = Duration::from_millis(1);
        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;
            let s_val = (2.0 * std::f64::consts::PI * F_HZ * t).sin();
            let c_val = (2.0 * std::f64::consts::PI * F_HZ * t).cos();
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let _ = sink.send_point(&tr_sine, PlotPoint { x: t_s, y: s_val });
            let _ = sink.send_point(&tr_cos, PlotPoint { x: t_s, y: c_val });
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    let mut cfg = LivePlotConfig::default();
    cfg.color_scheme = scheme;
    // rainbow overlay for grid
    let palette = vec![
        Color32::from_rgb(255, 0, 0),
        Color32::from_rgb(255, 127, 0),
        Color32::from_rgb(255, 255, 0),
        Color32::from_rgb(0, 255, 0),
        Color32::from_rgb(0, 0, 255),
        Color32::from_rgb(75, 0, 130),
        Color32::from_rgb(148, 0, 211),
    ];
    cfg.overlays = Some(Box::new(move |plot_ui, _scope, _traces| {
        let rect = plot_ui.response().rect;
        let n = palette.len();
        for i in 0..n {
            let color = palette[i];
            let x = rect.left() + rect.width() * (i as f32) / (n as f32);
            plot_ui.ctx().debug_painter().line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                (1.0, color),
            );
            let y = rect.top() + rect.height() * (i as f32) / (n as f32);
            plot_ui.ctx().debug_painter().line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                (1.0, color),
            );
        }
    }));

    cfg.title = "Custom Color Scheme Demo".to_string();
    cfg.headline = Some("Custom Color Scheme Demo".to_string());
    cfg.subheadline = Some("This uses a user-defined palette and visuals".to_string());

    run_liveplot(rx, cfg)
}
