//! Example: Take a delayed snapshot (screenshot + raw export)
//!
//! What it demonstrates
//! - Streaming two traces into the UI and using `UiActionController` to programmatically
//!   pause the UI, save a screenshot and raw time-domain data, and resume.
//!
//! Outputs (written to current working directory):
//! - `snapshot.png` — PNG screenshot of the viewport
//! - `snapshot.parquet` — raw exported data (parquet; may fall back to CSV)
//!
//! How to run
//! ```bash
//! cargo run --example sine_cosine_delayed_snapshot
//! ```
use liveplot::{channel_plot, run_liveplot, UiActionController, RawExportFormat, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace channel
    let (sink, rx) = channel_plot();
    let tr_sine = sink.create_trace("sine", None);
    let tr_cos = sink.create_trace("cosine", None);

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
            let t_s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            let _ = sink.send_point(&tr_sine, PlotPoint { x: t_s, y: s_val });
            let _ = sink.send_point(&tr_cos, PlotPoint { x: t_s, y: c_val });
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
