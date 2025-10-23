//! Example: Set custom colors for traces via `TracesController`
//!
//! What it demonstrates
//! - Using `TracesController` to change trace colors programmatically after traces are registered.
//!
//! How to run
//! ```bash
//! cargo run --example custom_colors
//! ```
//! The example streams two signals and sets custom RGB colors for the `sine` and `cosine` traces.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, TracesController, PlotPoint};
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
    cfg.title = Some("LivePlot (custom colors)".into());
    cfg.traces_controller = Some(traces_ctrl);
    run_liveplot(rx, cfg)
}
