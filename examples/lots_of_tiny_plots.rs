//! Example: Lots of tiny plots (20×15 grid)
//!
//! What it demonstrates
//! - Embedding many `MainPanel` instances in a grid
//! - Each plot shows the same sine waveform but shifted in phase
//! - Each trace receives a unique color (HSV wheel)
//!
//! How to run
//! ```bash
//! # default
//! cargo run --example lots_of_tiny_plots --
//! # set samples-per-second (Hz) and sine frequency (Hz)
//! cargo run --example lots_of_tiny_plots -- -s 10 -h 2.5
//! cargo run --example lots_of_tiny_plots -- --samples-per-second 10.0 --hz 2.5
//! ```

use eframe::{egui, NativeOptions};
use liveplot::{channel_plot, MainPanel, PlotPoint, PlotSink, Trace, TracesController};
use std::time::Duration;

const COLS: usize = 20;
const ROWS: usize = 15;
const TOTAL: usize = COLS * ROWS;

/// Repaint interval in milliseconds (controls UI repaint frequency; independent of sampling)
const UPDATE_MS: u64 = 16;
/// Default samples-per-second (Hz) used when feeding points into the plots.
/// A value near the UI refresh rate keeps things smooth.
const DEFAULT_SAMPLES_PER_SEC: f64 = 60.0;
/// Default frequency of the sine wave itself (cycles per second).
const DEFAULT_SINE_HZ: f64 = 1.5;

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    // h in [0,1), s,v in [0,1]
    let h6 = (h.fract() * 6.0).max(0.0);
    let i = h6.floor() as i32;
    let f = h6 - (i as f64);
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        5 => (v, p, q),
        _ => (v, p, q),
    };
    [
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

struct TinyPlot {
    sink: PlotSink,
    trace: Trace,
    phase_cycles: f64,
}

impl TinyPlot {
    fn new(label: &str, phase_cycles: f64, color_hint: [u8; 3]) -> (Self, MainPanel) {
        let (sink, rx) = channel_plot();
        let trace = sink.create_trace(label, None);

        let mut panel = MainPanel::new(rx);
        // keep buffers small for many plots
        panel.traces_data.max_points = 2_000;
        // strip all borders/margins so each cell is pure plot
        panel.compact = true;
        // suppress top-bar buttons so nothing competes for the tiny space
        panel.top_bar_buttons = Some(vec![]);
        panel.sidebar_buttons = Some(vec![]);
        panel.min_height_for_top_bar = 0.0;
        panel.min_width_for_sidebar = 0.0;
        panel.min_height_for_sidebar = 0.0;
        for s in panel.liveplot_panel.get_data_mut() {
            s.time_window = 4.0;
            // Force-hide the legend overlay to avoid wasting space in tiny cells
            s.force_hide_legend = true;
        }

        // Attach a traces controller so we can request a color for this trace
        let ctrl = TracesController::new();
        panel.set_controllers(None, None, Some(ctrl.clone()), None, None, None, None);
        ctrl.request_set_color(label, color_hint);

        (
            Self {
                sink,
                trace,
                phase_cycles,
            },
            panel,
        )
    }

    fn feed(&self, t: f64, freq_hz: f64) {
        const TAU: f64 = std::f64::consts::TAU;
        let y = ((t * freq_hz + self.phase_cycles) * TAU).sin();
        let _ = self.sink.send_point(&self.trace, PlotPoint { x: t, y });
    }
}

struct LotsOfTinyPlotsApp {
    plots: Vec<(TinyPlot, MainPanel)>,
    /// Last known window size; used to detect resizes and trigger auto-fit.
    last_window_size: egui::Vec2,
    /// Sample rate in Hz (samples per second) for the sine waveform fed to every plot.
    samples_per_second: f64,
    /// Frequency of the sine wave itself (cycles per second).
    sine_hz: f64,
    /// Timestamp (seconds) of the last sample we generated; used to step the
    /// sampler forward at `samples_per_second` even if frame rate varies.
    last_sample_time: f64,
}

impl LotsOfTinyPlotsApp {
    fn new(samples_per_second: f64, sine_hz: f64) -> Self {
        let now_us = chrono::Utc::now().timestamp_micros();
        let start_t = (now_us as f64) * 1e-6;

        let mut plots = Vec::with_capacity(TOTAL);
        for i in 0..TOTAL {
            let phase = (i as f64) / (TOTAL as f64); // cycles [0..1)
            let hue = (i as f64) / (TOTAL as f64);
            let col = hsv_to_rgb(hue, 0.85, 0.9);
            let label = format!("sine_{:03}", i);
            let (p, mp) = TinyPlot::new(&label, phase, col);
            plots.push((p, mp));
        }
        Self {
            plots,
            last_window_size: egui::Vec2::ZERO,
            samples_per_second,
            sine_hz,
            last_sample_time: start_t,
        }
    }

    fn render_grid(&mut self, ui: &mut egui::Ui) {
        // Claim the entire remaining area so the grid fills and resizes with the window.
        let avail = ui.available_size();
        let (grid_rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());

        // Floor to whole pixels; use the grid_rect origin for pixel-aligned placement.
        let cell_w = (grid_rect.width() / COLS as f32).floor().max(1.0);
        let cell_h = (grid_rect.height() / ROWS as f32).floor().max(1.0);

        for row in 0..ROWS {
            for col in 0..COLS {
                let idx = row * COLS + col;
                let x = (grid_rect.left() + col as f32 * cell_w).round();
                let y = (grid_rect.top() + row as f32 * cell_h).round();
                let cell_rect =
                    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cell_w, cell_h));
                let (_p, panel) = &mut self.plots[idx];
                let mut child_ui =
                    ui.new_child(egui::UiBuilder::new().id_salt(idx).max_rect(cell_rect));
                panel.update_embedded(&mut child_ui);
            }
        }
    }
}

impl eframe::App for LotsOfTinyPlotsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // use wall-clock time as sample x
        let now_us = chrono::Utc::now().timestamp_micros();
        let t = (now_us as f64) * 1e-6;
        // generate samples at configured rate; each sample uses the sine frequency
        let sample_interval = 1.0 / self.samples_per_second;
        let mut next_time = self.last_sample_time + sample_interval;
        while next_time <= t {
            for (p, _) in &self.plots {
                p.feed(next_time, self.sine_hz);
            }
            // advance last_sample_time by one interval per sample generated
            self.last_sample_time = next_time;
            next_time += sample_interval;
        }

        // Detect window resizes and auto-fit all plots when the size changes.
        let current_size = ctx.input(|i| i.viewport_rect().size());
        if self.last_window_size != egui::Vec2::ZERO && self.last_window_size != current_size {
            for (_p, panel) in &mut self.plots {
                panel.fit_all_bounds();
            }
        }
        self.last_window_size = current_size;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Lots of tiny sine plots — 20 × 15");
            ui.label(format!(
                "Each plot shows the same sine wave shifted by phase; every trace has its own color. — samples: {:.1} Hz, sine: {:.3} Hz",
                self.samples_per_second,
                self.sine_hz
            ));
            ui.add_space(6.0);
            self.render_grid(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(UPDATE_MS));
    }
}

fn main() -> eframe::Result<()> {
    // Parse simple CLI: -s / --samples-per-second <Hz> and -h / --hz <Hz>
    let mut samples_per_second = DEFAULT_SAMPLES_PER_SEC;
    let mut sine_hz = DEFAULT_SINE_HZ;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-s" || arg == "--samples-per-second" {
            if let Some(val) = args.next() {
                match val.parse::<f64>() {
                    Ok(v) => samples_per_second = v,
                    Err(_) => eprintln!("invalid value for {}: {}", arg, val),
                }
            }
        } else if arg == "-h" || arg == "--hz" {
            if let Some(val) = args.next() {
                match val.parse::<f64>() {
                    Ok(v) => sine_hz = v,
                    Err(_) => eprintln!("invalid value for {}: {}", arg, val),
                }
            }
        } else if let Some(rest) = arg.strip_prefix("--samples-per-second=") {
            match rest.parse::<f64>() {
                Ok(v) => samples_per_second = v,
                Err(_) => eprintln!("invalid value for --samples-per-second: {}", rest),
            }
        } else if let Some(rest) = arg.strip_prefix("--hz=") {
            match rest.parse::<f64>() {
                Ok(v) => sine_hz = v,
                Err(_) => eprintln!("invalid value for --hz: {}", rest),
            }
        } else {
            // ignore unknown args
        }
    }

    let app = LotsOfTinyPlotsApp::new(samples_per_second, sine_hz);
    eframe::run_native(
        "Lots of tiny plots",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
