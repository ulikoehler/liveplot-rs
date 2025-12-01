// Example: sine_cosine_delayed_snapshot
//
// This example streams two traces (`sine` and `cosine`) at 1 kHz and opens
// the multi-trace UI. After 5 seconds it will:
//  1) Pause the UI (take a time-domain snapshot)
//  2) Save a PNG screenshot to `./snapshot.png`
//  3) Save the raw time-domain data for all traces to `./snapshot.parquet`
//     (note: Parquet currently falls back to CSV; filename is used to choose format)
//  4) Resume the UI after a short delay
//
// Files written to the current directory by default:
//  - snapshot.png        (viewport screenshot)
//  - snapshot.parquet    (raw time-domain points for all traces; CSV fallback)
//
// Run with:
//
//   cargo run --example sine_cosine_delayed_snapshot
//
use liveplot::{channel_multi, run_liveplot, LivePlotConfig, RawExportFormat, UiActionController};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace channel
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
            let _ = sink.send_value(n, s_val, now_us, "sine");
            let _ = sink.send_value(n, c_val, now_us, "cosine");
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Create UI action controller
    let ui_ctrl = UiActionController::new();

    // After 5 seconds: pause, save screenshot and parquet (both to current directory), then resume.
    {
        let ui_ctrl = ui_ctrl.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            ui_ctrl.pause();
            // Save a PNG screenshot to CWD
            ui_ctrl.request_save_png_to_path("./snapshot.png");
            // Save raw data as parquet to CWD
            ui_ctrl.request_save_raw_to_path(RawExportFormat::Parquet, "./snapshot.parquet");
            // Resume after a short delay
            std::thread::sleep(Duration::from_secs(1));
            ui_ctrl.resume();
        });
    }

    let mut cfg = LivePlotConfig::default();
    cfg.title = Some("LivePlot (delayed snapshot)".into());
    cfg.native_options = Some(eframe::NativeOptions::default());
    cfg.ui_action_controller = Some(ui_ctrl);
    run_liveplot(rx, cfg)
}
