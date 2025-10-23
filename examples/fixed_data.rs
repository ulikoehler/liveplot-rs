//! Example: Display fixed (precomputed) trace data
//!
//! What it demonstrates
//! - Loading and showing static time-series data using `ScopeAppMulti::set_trace_data`.
//! - Pausing the UI and auto-fitting to the provided dataset.
//!
//! How to run
//! ```bash
//! cargo run --example fixed_data
//! ```
//! This example preloads 10 periods of a 1 Hz sine and cosine, then displays them as paused traces.

use eframe::{egui, NativeOptions};
use liveplot::{channel_plot, ScopeAppMulti};

fn make_fixed_waves() -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    // 10 periods of a 1 Hz sine and cosine wave.
    let f_hz = 1.0f64;
    let periods = 10.0f64;
    let duration = periods / f_hz; // seconds
    let n = 2000usize; // number of samples per trace

    // Anchor the data to end "now" so X axis shows current wall time
    let t_end = chrono::Utc::now().timestamp_micros() as f64 * 1e-6;
    let t_start = t_end - duration;

    let dt = duration / (n.saturating_sub(1) as f64);

    let mut sine: Vec<[f64; 2]> = Vec::with_capacity(n);
    let mut cosine: Vec<[f64; 2]> = Vec::with_capacity(n);

    for i in 0..n {
        let t = t_start + (i as f64) * dt;
        let phase = 2.0 * std::f64::consts::PI * f_hz * (t - t_start);
        sine.push([t, phase.sin()]);
        cosine.push([t, phase.cos()]);
    }

    (sine, cosine)
}

struct FixedDataApp {
    plot: ScopeAppMulti,
}

impl FixedDataApp {
    fn new() -> Self {
        // Create a plot app with an unused channel (no live data needed)
    let (_sink, rx) = channel_plot();
        let mut plot = ScopeAppMulti::new(rx);
        plot.time_window = 10.0;
        plot.max_points = 10_000;
        plot.show_legend = true;

        // Prepare and set fixed data
        let (sine, cosine) = make_fixed_waves();
        plot.set_trace_data("sine", sine);
        plot.set_trace_data("cosine", cosine);
        // Optional Y unit
        // plot.y_unit = Some("V".into());

        Self { plot }
    }
}

impl eframe::App for FixedDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.heading("Fixed data demo (10 periods of 1 Hz sine + cosine)");
            ui.label("Data is preloaded via set_trace_data() and view is paused.");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.plot.ui_embed(ui);
        });
    }
}

fn main() -> eframe::Result<()> {
    let app = FixedDataApp::new();
    eframe::run_native(
        "LivePlot fixed data demo",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
