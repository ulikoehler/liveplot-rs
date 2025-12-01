use liveplot::{channel_multi, run_liveplot, LivePlotConfig, WindowController};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_multi();

    // Simple sine producer (1 kHz sample rate, 3 Hz sine)
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0;
        const F_HZ: f64 = 3.0;
        let dt = Duration::from_millis(1);
        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;
            let val = (2.0 * std::f64::consts::PI * F_HZ * t).sin();
            let now_us = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0);
            let _ = sink.send_value(n, val, now_us, "signal");
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Create a window controller to observe and control the window
    let window_ctrl = WindowController::new();
    let updates = window_ctrl.subscribe();

    // On startup: try to set the window to half the primary monitor size, positioned at top-right.
    // We use the `display_info` crate to query current monitor size.
    #[cfg(feature = "window_control_display_info")]
    {
        if let Ok(di) = display_info::DisplayInfo::all() {
            if let Some(primary) = di.into_iter().find(|d| d.is_primary) {
                let w = primary.width as f32;
                let h = primary.height as f32;
                let half = [w * 0.5, h * 0.5];
                // Top-right outer position: (right - half_width, 0)
                let pos = [w - half[0], 0.0];
                window_ctrl.request_set_size(half);
                window_ctrl.request_set_pos(pos);
            }
        }
    }

    // Log window updates in a background thread
    std::thread::spawn(move || {
        let mut last_size: Option<[f32; 2]> = None;
        let mut last_pos: Option<[f32; 2]> = None;
        while let Ok(info) = updates.recv() {
            let changed = info.current_size != last_size || info.current_pos != last_pos;
            if changed {
                eprintln!(
                    "[window_control] Window size={:?} pos={:?} (requested size={:?} pos={:?})",
                    info.current_size, info.current_pos, info.requested_size, info.requested_pos
                );
                last_size = info.current_size;
                last_pos = info.current_pos;
            }
        }
    });

    // Run with default options but attach our window controller
    let mut cfg = LivePlotConfig::default();
    cfg.title = Some("LivePlot (window control demo)".into());
    cfg.native_options = Some(eframe::NativeOptions::default());
    cfg.window_controller = Some(window_ctrl);
    run_liveplot(rx, cfg)
}
