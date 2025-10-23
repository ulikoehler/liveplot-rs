use liveplot::{channel_plot, run_liveplot, LivePlotConfig, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create multi-trace plot channel (single trace labeled "signal")
    let (sink, rx) = channel_plot();
    let trace = sink.create_trace("signal", Some("Sine with decaying history"));

    // Producer: 500 Hz sample rate, 2 Hz sine
    let sink_clone = sink.clone();
    let trace_clone = trace.clone();
    std::thread::spawn(move || {
        const FS_HZ: f64 = 500.0;
        const F_HZ: f64 = 2.0;
        let dt = Duration::from_millis((1000.0 / FS_HZ) as u64);
        let mut n: u64 = 0;
        loop {
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let phase_t = n as f64 / FS_HZ;
            let val = (2.0 * std::f64::consts::PI * F_HZ * phase_t).sin();
            let _ = sink_clone.send_point(&trace_clone, PlotPoint { x: t_s, y: val });
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    // Periodically shrink the amplitude of older parts of the trace.
    // Every 300ms, apply a 0.9x multiplier to all samples older than 3 seconds.
    let sink_decay = sink.clone();
    let trace_decay = trace.clone();
    std::thread::spawn(move || {
        let mut last_tick = SystemTime::now();
        loop {
            std::thread::sleep(Duration::from_millis(300));
            let now_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
                // Define the range: everything strictly older than 3 seconds
            let x_max = now_s - 3.0;
            if x_max.is_finite() {
                // Apply multiplicative decay to older samples; repeated application makes older segments fade
                // Use NaN as the lower bound to mean "start of data" (see docs)
                let _ = sink_decay.apply_y_fn_in_x_range(&trace_decay, f64::NAN, x_max, Box::new(|y| y * 0.9));
            }
            // Prevent too frequent updates if system clock jumps
            let _ = &mut last_tick;
        }
    });

    // Run the UI until closed
    run_liveplot(rx, LivePlotConfig::default())
}
