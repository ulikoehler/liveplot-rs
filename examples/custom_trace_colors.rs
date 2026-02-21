//! Example: Set custom colors for traces via `TracesController`
//!
//! What it demonstrates
//! - Using `TracesController` to change trace colors programmatically after traces are registered.
//!
//! How to run
//! ```bash
//! cargo run --example custom_trace_colors
//! ```
//! The example streams two signals and sets custom RGB colors for the `sine` and `cosine` traces.

use eframe::egui::{Color32, Pos2};
use liveplot::{
    channel_plot, run_liveplot, ColorScheme, LivePlotConfig, PlotPoint, TracesController,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace plot channel
    let (sink, rx) = channel_plot();
    let tr_sine = sink.create_trace("sine", None);
    let tr_cos = sink.create_trace("cosine", None);

    // Producer: 1 kHz sample rate, 3 Hz sine and cosine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const F_HZ: f64 = 3.0; // 3 Hz
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

    // Traces controller to set custom colors once traces appear
    let traces_ctrl = TracesController::new();
    {
        // In a background thread, subscribe to trace info and set colors when seen
        let ctrl = traces_ctrl.clone();
        let rx_info = ctrl.subscribe();
        std::thread::spawn(move || {
            while let Ok(info) = rx_info.recv() {
                for tr in info.traces {
                    match tr.name.as_str() {
                        "sine" => ctrl.request_set_color("sine", [0xFF, 0x66, 0x00]), // orange
                        "cosine" => ctrl.request_set_color("cosine", [0x00, 0x99, 0xFF]), // blue
                        _ => {}
                    }
                }
            }
        });
    }

    // Run the UI with the controller attached via config
    let mut cfg = LivePlotConfig::default();
    cfg.title = "LivePlot (custom colors)".into();
    cfg.controllers.traces = Some(traces_ctrl);

    // make a yellow background and rainbow palette for traces
    let rainbow = vec![
        Color32::from_rgb(255, 0, 0),   // red
        Color32::from_rgb(255, 127, 0), // orange
        Color32::from_rgb(255, 255, 0), // yellow
        Color32::from_rgb(0, 255, 0),   // green
        Color32::from_rgb(0, 0, 255),   // blue
        Color32::from_rgb(75, 0, 130),  // indigo
        Color32::from_rgb(148, 0, 211), // violet
    ];
    let mut visuals = eframe::egui::Visuals::dark();
    visuals.panel_fill = Color32::YELLOW; // bright yellow background
    cfg.color_scheme = ColorScheme::Custom(liveplot::config::CustomColorScheme {
        visuals: Some(visuals),
        palette: rainbow.clone(),
        label: Some("Rainbow Theme".to_string()),
    });

    // overlay closure draws rainbow grid lines over plots
    cfg.overlays = Some(Box::new(move |plot_ui, _scope, _traces| {
        let rect = plot_ui.response().rect;
        let n = rainbow.len();
        for i in 0..n {
            let color = rainbow[i];
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

    run_liveplot(rx, cfg)
}
