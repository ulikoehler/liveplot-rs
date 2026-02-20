//! Example: Simple event listener (click events)
//!
//! What it demonstrates
//! - Creating an [`EventController`] and subscribing to click events.
//! - Attaching the controller to [`LivePlotConfig`] so the UI emits events.
//! - Receiving and printing events on a background thread.
//!
//! How to run
//! ```bash
//! cargo run --example events_simple
//! ```
//! Click inside the plot area to see click events printed in the terminal.

use liveplot::{
    channel_plot, run_liveplot, EventController, EventFilter, EventKind, LivePlotConfig, PlotPoint,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Create the event controller and subscribe to click-related events.
    let event_ctrl = EventController::new();
    let rx = event_ctrl.subscribe(EventFilter::only(
        EventKind::CLICK | EventKind::DOUBLE_CLICK,
    ));

    // Print received events on a background thread.
    std::thread::spawn(move || {
        while let Ok(evt) = rx.recv() {
            println!("[event] kind={:?}", evt.kinds);
            if let Some(click) = &evt.click {
                if let Some(pp) = &click.plot_pos {
                    println!("  plot position: ({:.4}, {:.4})", pp.x, pp.y);
                }
                if let Some(sp) = &click.screen_pos {
                    println!("  screen position: ({:.1}, {:.1})", sp.x, sp.y);
                }
            }
        }
        println!("[event] channel closed");
    });

    // Set up a simple sine trace so there is something to click on.
    let (sink, data_rx) = channel_plot();
    let trace = sink.create_trace("sine", Some("Sine Wave"));

    std::thread::spawn(move || {
        let dt = Duration::from_millis(1);
        loop {
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let val = (2.0 * std::f64::consts::PI * 2.0 * t_s).sin();
            let _ = sink.send_point(&trace, PlotPoint { x: t_s, y: val });
            std::thread::sleep(dt);
        }
    });

    let mut cfg = LivePlotConfig::default();
    cfg.event_controller = Some(event_ctrl);

    run_liveplot(data_rx, cfg)
}
