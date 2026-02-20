//! Example: Advanced event listener (all event types)
//!
//! What it demonstrates
//! - Subscribing to every event kind with [`EventFilter::all()`].
//! - Printing detailed metadata for each event category.
//! - Handling click, double-click, measurement, zoom, pan, fit-to-view,
//!   resize, key-press, data-update, pause/resume, scope add/remove,
//!   trace show/hide, colour change, math trace, threshold, export,
//!   and screenshot events.
//!
//! How to run
//! ```bash
//! cargo run --example events_advanced
//! ```
//! Interact with the plot (click, scroll, press keys, add measurements, etc.)
//! and watch the events stream in the terminal.

use liveplot::{
    channel_plot, run_liveplot, EventController, EventFilter, EventKind, LivePlotConfig, PlotPoint,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Subscribe to ALL events.
    let event_ctrl = EventController::new();
    let rx = event_ctrl.subscribe(EventFilter::all());

    // Background thread: pretty-print every event.
    std::thread::spawn(move || {
        while let Ok(evt) = rx.recv() {
            let k = evt.kind;

            // ── Click / Double-click ──────────────────────────────────────
            if k.contains(EventKind::CLICK) || k.contains(EventKind::DOUBLE_CLICK) {
                let label = if k.contains(EventKind::DOUBLE_CLICK) {
                    "DOUBLE_CLICK"
                } else {
                    "CLICK"
                };
                if let Some(c) = &evt.click {
                    println!(
                        "[{label}] plot=({:.4},{:.4})  screen=({:.1},{:.1})  scope={}",
                        c.plot_pos.x,
                        c.plot_pos.y,
                        c.screen_pos.x,
                        c.screen_pos.y,
                        c.scope_id.map_or("?".into(), |id| id.to_string()),
                    );
                } else {
                    println!("[{label}]");
                }
            }

            // ── Pause / Resume ────────────────────────────────────────────
            if k.contains(EventKind::PAUSE) || k.contains(EventKind::RESUME) {
                let label = if k.contains(EventKind::RESUME) {
                    "RESUME"
                } else {
                    "PAUSE"
                };
                if let Some(p) = &evt.pause {
                    println!("[{label}] scope={}", p.scope_id.unwrap_or(0));
                } else {
                    println!("[{label}]");
                }
            }

            // ── Zoom / Pan / Fit-to-View ──────────────────────────────────
            if k.contains(EventKind::ZOOM)
                || k.contains(EventKind::PAN)
                || k.contains(EventKind::FIT_TO_VIEW)
            {
                let label = if k.contains(EventKind::FIT_TO_VIEW) {
                    "FIT_TO_VIEW"
                } else if k.contains(EventKind::ZOOM) {
                    "ZOOM"
                } else {
                    "PAN"
                };
                if let Some(v) = &evt.view_change {
                    println!(
                        "[{label}] x={:?}  y={:?}  scope={}",
                        v.x_range,
                        v.y_range,
                        v.scope_id.map_or("?".into(), |id| id.to_string()),
                    );
                } else {
                    println!("[{label}]");
                }
            }

            // ── Measurement ───────────────────────────────────────────────
            if k.contains(EventKind::MEASUREMENT_POINT) {
                let complete = k.contains(EventKind::MEASUREMENT_COMPLETE);
                if let Some(m) = &evt.measurement {
                    println!(
                        "[MEASUREMENT{}] name={:?} point=({:.4},{:.4}) p1={:?} p2={:?} slope={:?} dist={:?}",
                        if complete { " COMPLETE" } else { "" },
                        m.measurement_name,
                        m.point[0], m.point[1],
                        m.p1, m.p2, m.slope, m.distance,
                    );
                }
            }
            if k.contains(EventKind::MEASUREMENT_CLEARED) {
                println!("[MEASUREMENT_CLEARED]");
            }

            // ── Resize ────────────────────────────────────────────────────
            if k.contains(EventKind::RESIZE) {
                if let Some(r) = &evt.resize {
                    println!("[RESIZE] {}×{}", r.width as u32, r.height as u32);
                }
            }

            // ── Key press ─────────────────────────────────────────────────
            if k.contains(EventKind::KEY_PRESSED) {
                if let Some(kp) = &evt.key_press {
                    println!(
                        "[KEY] {:?}  ctrl={} alt={} shift={} cmd={}",
                        kp.key,
                        kp.modifiers.ctrl,
                        kp.modifiers.alt,
                        kp.modifiers.shift,
                        kp.modifiers.command,
                    );
                }
            }

            // ── Data update ───────────────────────────────────────────────
            if k.contains(EventKind::DATA_UPDATED) {
                if let Some(d) = &evt.data_update {
                    println!("[DATA_UPDATED] traces={:?}", d.traces);
                }
            }

            // ── Trace visibility / colour / offset ────────────────────────
            if k.contains(EventKind::TRACE_SHOWN) || k.contains(EventKind::TRACE_HIDDEN) {
                if let Some(t) = &evt.trace {
                    println!(
                        "[TRACE_{}] {:?} visible={:?}",
                        if k.contains(EventKind::TRACE_SHOWN) {
                            "SHOWN"
                        } else {
                            "HIDDEN"
                        },
                        t.trace.0,
                        t.visible,
                    );
                }
            }
            if k.contains(EventKind::TRACE_COLOR_CHANGED) {
                if let Some(t) = &evt.trace {
                    println!("[TRACE_COLOR] {:?} rgb={:?}", t.trace.0, t.color_rgb);
                }
            }
            if k.contains(EventKind::TRACE_OFFSET_CHANGED) {
                if let Some(t) = &evt.trace {
                    println!("[TRACE_OFFSET] {:?} offset={:?}", t.trace.0, t.offset);
                }
            }

            // ── Math trace ────────────────────────────────────────────────
            if k.contains(EventKind::MATH_TRACE_ADDED) {
                if let Some(m) = &evt.math_trace {
                    println!("[MATH_TRACE_ADDED] {:?} formula={:?}", m.name, m.formula);
                }
            }
            if k.contains(EventKind::MATH_TRACE_REMOVED) {
                if let Some(m) = &evt.math_trace {
                    println!("[MATH_TRACE_REMOVED] {:?}", m.name);
                }
            }

            // ── Threshold ─────────────────────────────────────────────────
            if k.contains(EventKind::THRESHOLD_EXCEEDED) {
                if let Some(t) = &evt.threshold {
                    println!(
                        "[THRESHOLD_EXCEEDED] {:?} trace={:?} area={:?}",
                        t.threshold_name, t.trace, t.area,
                    );
                }
            }
            if k.contains(EventKind::THRESHOLD_REMOVED) {
                if let Some(t) = &evt.threshold {
                    println!("[THRESHOLD_REMOVED] {:?}", t.threshold_name);
                }
            }

            // ── Export / Screenshot ───────────────────────────────────────
            if k.contains(EventKind::EXPORT) || k.contains(EventKind::SCREENSHOT) {
                let label = if k.contains(EventKind::SCREENSHOT) {
                    "SCREENSHOT"
                } else {
                    "EXPORT"
                };
                if let Some(e) = &evt.export {
                    println!("[{label}] format={:?} path={:?}", e.format, e.path);
                }
            }

            // ── Scope management ──────────────────────────────────────────
            if k.contains(EventKind::SCOPE_ADDED) {
                if let Some(s) = &evt.scope_manage {
                    println!("[SCOPE_ADDED] id={}", s.scope_id);
                }
            }
            if k.contains(EventKind::SCOPE_REMOVED) {
                if let Some(s) = &evt.scope_manage {
                    println!("[SCOPE_REMOVED] id={}", s.scope_id);
                }
            }
        }
        println!("[event] channel closed");
    });

    // Set up a sine + cosine trace so there is data to interact with.
    let (sink, data_rx) = channel_plot();
    let t_sin = sink.create_trace("sin", Some("Sine"));
    let t_cos = sink.create_trace("cos", Some("Cosine"));

    std::thread::spawn(move || {
        let dt = Duration::from_millis(1);
        loop {
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let _ = sink.send_point(
                &t_sin,
                PlotPoint {
                    x: t_s,
                    y: (2.0 * std::f64::consts::PI * 2.0 * t_s).sin(),
                },
            );
            let _ = sink.send_point(
                &t_cos,
                PlotPoint {
                    x: t_s,
                    y: (2.0 * std::f64::consts::PI * 2.0 * t_s).cos(),
                },
            );
            std::thread::sleep(dt);
        }
    });

    let mut cfg = LivePlotConfig::default();
    cfg.event_controller = Some(event_ctrl);

    run_liveplot(data_rx, cfg)
}
