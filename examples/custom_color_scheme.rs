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

use eframe::egui::{Color32, Visuals};
use liveplot::config::CustomColorScheme;
use liveplot::{channel_plot, run_liveplot, ColorScheme, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Define a custom palette (8 colors)
    let palette = vec![
        Color32::from_rgb(255, 66, 0),    // orange
        Color32::from_rgb(0, 153, 255),   // blue
        Color32::from_rgb(0, 200, 70),    // green
        Color32::from_rgb(255, 200, 0),   // yellow
        Color32::from_rgb(200, 0, 200),   // purple
        Color32::from_rgb(255, 0, 120),   // pink
        Color32::from_rgb(120, 120, 120), // gray
        Color32::from_rgb(0, 0, 0),       // black
    ];

    // Optionally customize visuals (here: dark background, white text)
    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(20, 20, 20);
    visuals.override_text_color = Some(Color32::WHITE);

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
    cfg.title = "Custom Color Scheme Demo".to_string();
    cfg.headline = Some("Custom Color Scheme Demo".to_string());
    cfg.subheadline = Some("This uses a user-defined palette and visuals".to_string());

    run_liveplot(rx, cfg)
}
