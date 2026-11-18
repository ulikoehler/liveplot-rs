//! Example: XY-trace demo — modified version of `fft_demo`
//!
//! What it demonstrates
//! - The same two signals as `fft_demo` (`two_tone`: 5 Hz + 50 Hz sum,
//!   `swept_freq`: slow FM around 25 Hz), but plotted *against each
//!   other* instead of against time: the X coordinate comes from the
//!   `two_tone` trace and the Y coordinate from `swept_freq`.
//! - The resulting phase-space trajectory is a dense parametric curve
//!   that covers a wide area — a good stress test for XY rendering:
//!   the stroke must stay one continuous, filled-looking curve and
//!   must not break into separated segments when zooming in.
//!
//! How to run
//! ```bash
//! cargo run --example xy_demo --features fft
//! # Vulkan backend:
//! LIVEPLOT_VOLCANO=1 cargo run --example xy_demo --features fft,volcano
//! ```
//! Then open the scope settings (gear icon) and select **XY-Scope**.
//! Consecutive traces are paired automatically, so `x_sig` (X) pairs
//! with `y_sig` (Y). Compare against the default egui backend by
//! running without `LIVEPLOT_VOLCANO`.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_plot();
    // Consecutive traces pair as (X, Y) in XY-Scope mode.
    let tr_x = sink.create_trace("x_sig", None);
    let tr_y = sink.create_trace("y_sig", None);

    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const DT: Duration = Duration::from_millis(1);

        // Two-tone signal parameters (same as fft_demo)
        const F1_HZ: f64 = 5.0;
        const F2_HZ: f64 = 50.0;

        // Swept-frequency signal parameters (same as fft_demo)
        const F_CENTER_HZ: f64 = 25.0; // centre frequency
        const F_MOD_HZ: f64 = 0.1; // modulation rate
        const F_DEV_HZ: f64 = 15.0; // frequency deviation (±)

        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;

            // X coordinate: sum of two sines
            let x_val = (2.0 * std::f64::consts::PI * F1_HZ * t).sin()
                + (2.0 * std::f64::consts::PI * F2_HZ * t).sin();

            // Y coordinate: swept-frequency sine
            let phase = 2.0 * std::f64::consts::PI * F_CENTER_HZ * t
                + F_DEV_HZ / F_MOD_HZ * (2.0 * std::f64::consts::PI * F_MOD_HZ * t).sin();
            let y_val = phase.sin();

            // Both traces share the same timestamp so the XY join
            // pairs every sample 1:1.
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            let _ = sink.send_point(&tr_x, PlotPoint { x: t_s, y: x_val });
            let _ = sink.send_point(&tr_y, PlotPoint { x: t_s, y: y_val });

            n = n.wrapping_add(1);
            std::thread::sleep(DT);
        }
    });

    let cfg = LivePlotConfig {
        headline: Some("XY Demo".to_string()),
        subheadline: Some(
            "x_sig vs y_sig phase trajectory — enable 'XY-Scope' in scope settings".to_string(),
        ),
        // Cap the periodic repaint rate — 25 fps is plenty for a streaming
        // scope display and roughly halves render/CPU cost vs 60 fps.
        max_fps: Some(25.0),
        ..Default::default()
    };
    run_liveplot(rx, cfg)
}
