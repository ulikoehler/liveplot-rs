//! Example: FFT demo with two-tone and swept-frequency signals
//!
//! What it demonstrates
//! - Two signals ideal for visualising the FFT panel:
//!   * `two_tone` – sum of two sine waves (5 Hz + 50 Hz) producing two
//!     distinct spectral peaks.
//!   * `swept_freq` – a sine whose instantaneous frequency is slowly
//!     modulated by another sine (0.1 Hz modulation rate, 25 Hz centre,
//!     ±15 Hz deviation), producing a broadened/moving peak in the FFT.
//! - Streaming two traces concurrently at 1 kHz sample rate.
//!
//! How to run
//! ```bash
//! cargo run --example fft_demo --features fft
//! ```
//! Click "Show FFT" in the menu to open the FFT panel and view the
//! frequency spectra of both signals.

use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_plot();
    let tr_two_tone = sink.create_trace("two_tone", None);
    let tr_swept = sink.create_trace("swept_freq", None);

    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const DT: Duration = Duration::from_millis(1);

        // Two-tone signal parameters
        const F1_HZ: f64 = 5.0;
        const F2_HZ: f64 = 50.0;

        // Swept-frequency signal parameters
        const F_CENTER_HZ: f64 = 25.0; // centre frequency
        const F_MOD_HZ: f64 = 0.1; // modulation rate
        const F_DEV_HZ: f64 = 15.0; // frequency deviation (±)

        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;

            // Two-tone: sum of two sines
            let two_tone_val =
                (2.0 * std::f64::consts::PI * F1_HZ * t).sin()
                    + (2.0 * std::f64::consts::PI * F2_HZ * t).sin();

            // Swept frequency: instantaneous frequency varies sinusoidally
            // f_inst(t) = F_CENTER + F_DEV * sin(2*pi*F_MOD*t)
            // phase = 2*pi * integral(f_inst dt) = 2*pi*(F_CENTER*t + F_DEV/(2*pi*F_MOD) * sin(2*pi*F_MOD*t))
            let phase = 2.0 * std::f64::consts::PI * F_CENTER_HZ * t
                + F_DEV_HZ / F_MOD_HZ * (2.0 * std::f64::consts::PI * F_MOD_HZ * t).sin();
            let swept_val = phase.sin();

            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            let _ = sink.send_point(&tr_two_tone, PlotPoint { x: t_s, y: two_tone_val });
            let _ = sink.send_point(&tr_swept, PlotPoint { x: t_s, y: swept_val });

            n = n.wrapping_add(1);
            std::thread::sleep(DT);
        }
    });

    let mut cfg = LivePlotConfig::default();
    cfg.headline = Some("FFT Demo".to_string());
    cfg.subheadline = Some("Two-tone + swept-frequency signals — click Show FFT".to_string());
    run_liveplot(rx, cfg)
}
