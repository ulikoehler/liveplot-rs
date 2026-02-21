//! Example: Display fixed (precomputed) trace data
//!
//! What it demonstrates
//! - Loading and showing static time-series data using `PlotSink::set_data`.
//! - Pausing the UI and auto-fitting to the provided dataset.
//!
//! How to run
//! ```bash
//! cargo run --example fixed_data
//! ```
//! This example preloads 10 periods of a 1 Hz sine and cosine, then displays them as paused traces.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};

fn make_fixed_waves() -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    // 10 periods of a 1 Hz sine and cosine wave.
    let f_hz = 1.0f64;
    let periods = 10.0f64;
    let duration = periods / f_hz; // seconds
    let n = 2000usize; // number of samples per trace

    // Anchor the data to end "now" so X axis shows current wall time
    let t_end = chrono::Utc::now().timestamp_micros() as f64 * 1e-6;
    let t_start = t_end - duration;

    let dt = duration / (n.saturating_sub(1) as f64);

    let mut sine: Vec<[f64; 2]> = Vec::with_capacity(n);
    let mut cosine: Vec<[f64; 2]> = Vec::with_capacity(n);

    for i in 0..n {
        let t = t_start + (i as f64) * dt;
        let phase = 2.0 * std::f64::consts::PI * f_hz * (t - t_start);
        sine.push([t, phase.sin()]);
        cosine.push([t, phase.cos()]);
    }

    (sine, cosine)
}

fn main() -> eframe::Result<()> {
    // Create the plot channel: we will create traces and push their data via `PlotSink`.
    let (sink, rx) = channel_plot();

    // Prepare fixed waves and convert to PlotPoint vectors
    let (sine_v, cosine_v) = make_fixed_waves();
    let sine_points: Vec<PlotPoint> = sine_v
        .into_iter()
        .map(|p| PlotPoint { x: p[0], y: p[1] })
        .collect();
    let cosine_points: Vec<PlotPoint> = cosine_v
        .into_iter()
        .map(|p| PlotPoint { x: p[0], y: p[1] })
        .collect();

    // Register traces and set full data via sink
    let sine_t = sink.create_trace("sine", Some("Sine"));
    let cos_t = sink.create_trace("cosine", Some("Cosine"));
    let _ = sink.set_data(&sine_t, sine_points);
    let _ = sink.set_data(&cos_t, cosine_points);

    // Build configuration via LivePlotConfig instead of mutating the internal app fields
    let mut cfg = LivePlotConfig::default();
    cfg.time_window_secs = 10.0;
    cfg.max_points = 10_000;
    cfg.features.legend = true;

    // Show the UI using the channel receiver and the config
    run_liveplot(rx, cfg)
}
