//! Example: Density splatting renderer with noisy signals
//!
//! What it demonstrates
//! - Two noisy sine waves streamed at high rate.
//! - Switch to "Density Splatting" render mode via the trace look editor
//!   (right-click a trace in the legend) to see the density heat map.
//!
//! How to run
//! ```bash
//! cargo run --example density_demo
//! ```
//! You should see two traces with noisy signals. Right-click a trace in
//! the legend and select "Density Splatting" to see the density rendering.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_plot();

    let trace1 = sink.create_trace("noisy_sine", Some("Noisy 3Hz"));
    let trace2 = sink.create_trace("noisy_cos", Some("Noisy 1Hz +offset"));

    // Producer: 2 kHz sample rate with heavy noise
    std::thread::spawn(move || {
        const FS_HZ: f64 = 2000.0;
        const F1_HZ: f64 = 3.0;
        const F2_HZ: f64 = 1.0;
        let dt = Duration::from_micros(500);
        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Pseudo-random noise (deterministic, no rand dependency)
            let noise1 = ((n.wrapping_mul(2654435761) as f64 / u32::MAX as f64) - 0.5) * 2.0;
            let noise2 = ((n.wrapping_mul(40503) as f64 / u32::MAX as f64) - 0.5) * 1.5;

            let val1 = (2.0 * std::f64::consts::PI * F1_HZ * t).sin() + noise1;
            let val2 = (2.0 * std::f64::consts::PI * F2_HZ * t).cos() + noise2 + 5.0;

            let _ = sink.send_point(&trace1, PlotPoint { x: t_s, y: val1 });
            let _ = sink.send_point(&trace2, PlotPoint { x: t_s, y: val2 });

            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    run_liveplot(rx, LivePlotConfig::default())
}
