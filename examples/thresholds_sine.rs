use liveplot::{channel_multi, run_liveplot, ThresholdController, ThresholdDef, ThresholdKind, TraceRef, LivePlotConfig};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    let (sink, rx) = channel_multi();

    // Producer: 1 kHz sample rate, 3 Hz sine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0; // 1 kHz sampling rate
        const F_HZ: f64 = 3.0; // 3 Hz sine wave
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
    thr_ctrl.add_threshold(ThresholdDef {
        name: "gt_0_8".into(),
        target: TraceRef("signal".into()),
        kind: ThresholdKind::GreaterThan { value: 0.8 },
        min_duration_s: 0.002,
        max_events: 100,
    });

    // Run the UI with the controller attached via config
    let mut cfg = LivePlotConfig::default();
    cfg.title = Some("LivePlot (thresholds)".into());
    cfg.native_options = Some(eframe::NativeOptions::default());
    cfg.threshold_controller = Some(thr_ctrl);
    run_liveplot(rx, cfg)
}
