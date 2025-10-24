//! Example: use SetData to toggle between sine and cosine every 2 seconds.
//!
//! This example demonstrates the `SetData` command which replaces the entire
//! data vector for a trace atomically. The UI must run on the main thread on
//! Linux (winit requirement), so we run the UI on the main thread and spawn a
//! worker thread that toggles between sine and cosine every two seconds.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example set_toggle
//! ```

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liveplot::{run_liveplot, LivePlotConfig};
use liveplot::PlotPoint;
use liveplot::PlotCommand;

fn make_wave(is_sine: bool, t0: f64, n: usize, dt: f64) -> Vec<PlotPoint> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let t = t0 + (i as f64) * dt;
        let phase = 2.0 * std::f64::consts::PI * 1.0 * t; // 1 Hz
        let y = if is_sine { phase.sin() } else { phase.cos() };
        v.push(PlotPoint { x: t, y });
    }
    v
}

fn main() {
    // channel for PlotCommand messages
    let (tx, rx) = mpsc::channel::<PlotCommand>();

    // Producer thread: send commands while UI runs on the main thread
    let producer = thread::spawn(move || {
        // register trace id 1
        let _ = tx.send(PlotCommand::RegisterTrace {
            id: 1,
            name: "toggle".into(),
            info: Some("sine/cosine toggle".into()),
        });

        let _start = Instant::now();
        let mut t0 = 0.0_f64;
        // pre-generate one period of samples at 100 Hz
        let n = 200usize;
        let dt = 0.01_f64;
        let mut is_sine = true;

        loop {
            // Build waveform and use SetData to overwrite the trace
            let pts = make_wave(is_sine, t0, n, dt);
            if mpsc::Sender::send(&tx, PlotCommand::SetData { trace_id: 1, points: pts }).is_err() {
                break; // UI likely closed
            }
            // toggle every 2 seconds
            thread::sleep(Duration::from_secs(2));
            t0 += 2.0;
            is_sine = !is_sine;
        }
    });

    // Run UI on the main thread (required by winit/eframe)
    if let Err(e) = run_liveplot(rx, LivePlotConfig::default()) {
        eprintln!("UI error: {e}");
    }

    // Ensure producer exits
    let _ = producer.join();
}
