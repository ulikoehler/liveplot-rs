//! Example: Continuous sine wave producer
//!
//! What it demonstrates
//! - Streaming samples into the multi-trace UI using `channel_plot()` and `PlotSink`.
//! - Creating a trace with `create_trace` and sending points at a fixed sample rate.
//!
//! How to run
//! ```bash
//! cargo run --example sine
//! ```
//! You should see a single trace named "signal" rendering a live sine wave.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace plot channel (we use a single trace labeled "signal")
    let (sink, rx) = channel_plot();
    let trace = sink.create_trace("signal", Some("Test Sine"));

    // Producer: 1 kHz sample rate, 3 Hz sine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const F_HZ: f64 = 3.0; // 3 Hz sine wave
        let dt = Duration::from_millis(1);
        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;
            let val = (2.0 * std::f64::consts::PI * F_HZ * t).sin();
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            // Ignore error if the UI closed (receiver dropped)
            let _ = sink.send_point(&trace, PlotPoint { x: t_s, y: val });
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Run the UI until closed
    run_liveplot(rx, LivePlotConfig::default())
}
