//! Example: Threshold detection on a live sine trace
//!
//! What it demonstrates
//! - Using `ThresholdController` to define thresholds (e.g. greater-than) and receiving
//!   threshold events for traces streamed into LivePlot.
//!
//! How to run
//! ```bash
//! cargo run --example thresholds_sine
//! ```
//! The example streams a sine wave and prints threshold events to stderr when the
//! configured condition is met.

use liveplot::{
    channel_plot, run_liveplot, LivePlotConfig, PlotPoint, ThresholdController, ThresholdDef,
    ThresholdKind, TraceRef,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_plot();
    let trace = sink.create_trace("signal", None);

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
            let _ = sink.send_point(&trace, PlotPoint { x: t_s, y: val });
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Build a threshold controller and pre-configure a "> 0.8" threshold on "signal".
    let thr_ctrl = ThresholdController::new();
    {
        // Subscribe to events and print them
        let rx_evt = thr_ctrl.subscribe();
        std::thread::spawn(move || {
            while let Ok(evt) = rx_evt.recv() {
                eprintln!(
                    "[threshold] {}: {} from {:.3}s for {:.3} ms, area={:.4}",
                    evt.threshold,
                    evt.trace,
                    evt.start_t,
                    evt.duration * 1000.0,
                    evt.area
                );
            }
        });
    }
    thr_ctrl.request_add_threshold(ThresholdDef {
        name: "gt_0_8".into(),
        target: TraceRef("signal".into()),
        kind: ThresholdKind::GreaterThan { value: 0.8 },
        min_duration_s: 0.002,
        max_events: 100,
        ..Default::default()
    });

    // Run the UI with the controller attached via config
    let mut cfg = LivePlotConfig::default();
    cfg.title = "LivePlot (thresholds)".into();
    cfg.native_options = Some(eframe::NativeOptions::default());
    cfg.controllers.threshold = Some(thr_ctrl);
    run_liveplot(rx, cfg)
}
