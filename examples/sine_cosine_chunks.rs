//! Example: Sending sample chunks for efficiency
//!
//! What it demonstrates
//! - Producing and sending chunks of `PlotPoint` entries via `send_points` for better throughput.
//! - How to timestamp samples within a chunk so x-values remain monotonic.
//!
//! How to run
//! ```bash
//! cargo run --example sine_cosine_chunks
//! ```
//! The UI renders two traces (`sine` and `cosine`) where each update sends a 200-sample chunk.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace plot channel
    let (sink, rx) = channel_plot();
    let tr_sine = sink.create_trace("sine", None);
    let tr_cos = sink.create_trace("cosine", None);

    // Producer: generate chunks of 200 samples at 1 kHz for a 3 Hz sine/cosine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const F_HZ: f64 = 3.0; // 3 Hz
        const CHUNK: usize = 200;
        let dt = Duration::from_millis((CHUNK as f64 / FS_HZ * 1000.0) as u64);
        let mut n: u64 = 0;
        loop {
            // Base timestamp for this chunk (seconds since epoch)
            let base_t = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Build two vectors of PlotPoint for the chunk
            let mut pts_sine: Vec<PlotPoint> = Vec::with_capacity(CHUNK);
            let mut pts_cos: Vec<PlotPoint> = Vec::with_capacity(CHUNK);

            for i in 0..CHUNK {
                let t = (n as f64 + i as f64) / FS_HZ; // relative time in seconds
                let s_val = (2.0 * std::f64::consts::PI * F_HZ * t).sin();
                let c_val = (2.0 * std::f64::consts::PI * F_HZ * t).cos();
                // Use the base timestamp plus the per-sample offset to produce realistic x values
                let t_s = base_t + (i as f64) / FS_HZ;
                pts_sine.push(PlotPoint { x: t_s, y: s_val });
                pts_cos.push(PlotPoint { x: t_s, y: c_val });
            }

            // Ignore errors if the UI closed (receiver dropped)
            let _ = sink.send_points(&tr_sine, pts_sine);
            let _ = sink.send_points(&tr_cos, pts_cos);

            n = n.wrapping_add(CHUNK as u64);
            std::thread::sleep(dt);
        }
    });

    // Run the UI until closed. Uses the unified multi-trace engine.
    run_liveplot(rx, LivePlotConfig::default())
}
