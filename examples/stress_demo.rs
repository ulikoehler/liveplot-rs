//! Example: backend stress workload (egui_plot vs Volcano).
//!
//! What it demonstrates
//! - Many dense, jagged traces in a rolling time window — a worst-case
//!   workload for a line renderer. Intended for CPU/GPU comparison:
//!
//! ```bash
//! # egui_plot backend
//! cargo run --release --example stress_demo --features volcano
//! # Volcano (Vulkan) backend — same binary, env switch
//! LIVEPLOT_VOLCANO=1 cargo run --release --example stress_demo --features volcano
//! ```
//!
//! Workload knobs (environment variables):
//! - `STRESS_TRACES`   number of traces            (default 6)
//! - `STRESS_FS`       sample rate per trace in Hz (default 5000)
//! - `STRESS_WINDOW`   rolling window in seconds   (default 10)
//! - `STRESS_FPS`      repaint cap                 (default 60)
//!
//! With defaults the scope draws ~6 × 5 kHz × 10 s = 300 000 points per
//! frame at up to 60 fps. Set `LIVEPLOT_DEBUG_FPS=1` to print the achieved
//! frame rate to stderr every 2 s.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() -> eframe::Result<()> {
    let n_traces = env_f64("STRESS_TRACES", 6.0) as usize;
    let fs_hz = env_f64("STRESS_FS", 5000.0);
    let window_s = env_f64("STRESS_WINDOW", 10.0);
    let fps = env_f64("STRESS_FPS", 60.0);

    let (sink, rx) = channel_plot();
    let traces: Vec<_> = (0..n_traces)
        .map(|i| {
            let t = sink.create_trace(&format!("sig{i}"), None);
            t
        })
        .collect();

    std::thread::spawn(move || {
        // Send in batches: 50 batches/s keeps the producer cheap while
        // still looking like a live stream.
        const BATCHES_PER_SEC: f64 = 50.0;
        let batch = (fs_hz / BATCHES_PER_SEC).max(1.0) as usize;
        let dt = Duration::from_secs_f64(1.0 / BATCHES_PER_SEC);
        let mut n: u64 = 0;
        loop {
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            for (i, tr) in traces.iter().enumerate() {
                let mut pts = Vec::with_capacity(batch);
                for k in 0..batch {
                    let idx = n.wrapping_add(k as u64);
                    let t = idx as f64 / fs_hz;
                    // Distinct timestamp per point (backdated within the batch)
                    let x = t_s + (k as f64 - batch as f64) / fs_hz;
                    // Sines + deterministic pseudo-noise → jagged polyline
                    let noise = ((idx.wrapping_mul(2654435761 ^ (i as u64 + 1)) as f64
                        / u32::MAX as f64)
                        - 0.5)
                        * 0.6;
                    let v = (2.0 * std::f64::consts::PI * (1.0 + i as f64) * t).sin()
                        + 0.4 * (2.0 * std::f64::consts::PI * 37.0 * t).sin()
                        + noise
                        + i as f64 * 3.0;
                    pts.push(PlotPoint { x, y: v });
                }
                let _ = sink.send_points(tr, pts);
            }
            n = n.wrapping_add(batch as u64);
            std::thread::sleep(dt);
        }
    });

    let cfg = LivePlotConfig {
        headline: Some("Stress Demo".to_string()),
        subheadline: Some(format!(
            "{n_traces} traces × {fs_hz} Hz × {window_s}s window @ {fps} fps cap"
        )),
        time_window_secs: window_s,
        max_points: (fs_hz * window_s * 1.5) as usize,
        max_fps: Some(fps),
        ..Default::default()
    };
    run_liveplot(rx, cfg)
}
