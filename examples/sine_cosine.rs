use liveplot::{channel_multi, run_liveplot, LivePlotConfig};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace plot channel
    let (sink, rx) = channel_multi();

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
            let now_us = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0);
            // Ignore error if the UI closed (receiver dropped)
            let _ = sink.send_value(n, s_val, now_us, "sine");
            let _ = sink.send_value(n, c_val, now_us, "cosine");
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Run the UI until closed (default: FFT hidden). Uses the unified multi-trace engine.
    run_liveplot(rx, LivePlotConfig::default())
}
