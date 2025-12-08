//! Example: Embedding LivePlot into your own egui application window
//!
//! What it demonstrates
//! - How to embed the `LivePlotApp` UI inside an existing `eframe`/`egui` application window.
//! - Feeding data from the main app into the embedded plot via `PlotSink` and `Trace` handles.
//!
//! How to run
//! ```bash
//! cargo run --example embedded_window
//! ```
//! Click "Open Plot Window" in the demo UI to show the embedded LivePlot view.

use std::time::Duration;

use eframe::{egui, NativeOptions};
use liveplot::{channel_plot, LivePlotApp, PlotPoint, PlotSink, Trace};

#[derive(Clone, Copy, PartialEq)]
enum WaveKind {
    Sine,
    Cosine,
}

struct DemoApp {
    kind: WaveKind,
    // data feed
    sink: PlotSink,
    trace_sine: Trace,
    trace_cos: Trace,
    // embedded plot app
    plot: LivePlotApp,
    // show window flag
    show_plot_window: bool,
}

impl DemoApp {
    fn new() -> Self {
        let (sink, rx) = channel_plot();
        let trace_sine = sink.create_trace("sine", None);
        let trace_cos = sink.create_trace("cosine", None);
        let mut plot = LivePlotApp::new(rx);
        plot.time_window = 10.0;
        plot.max_points = 10_000;
        Self {
            kind: WaveKind::Sine,
            sink,
            trace_sine,
            trace_cos,
            plot,
            show_plot_window: false,
        }
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Embedding LivePlot in egui::Window");
            ui.horizontal(|ui| {
                ui.label("Select wave:");
                egui::ComboBox::from_id_salt("wave_select")
                    .selected_text(match self.kind {
                        WaveKind::Sine => "Sine",
                        WaveKind::Cosine => "Cosine",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.kind, WaveKind::Sine, "Sine");
                        ui.selectable_value(&mut self.kind, WaveKind::Cosine, "Cosine");
                    });
                if ui.button("Open Plot Window").clicked() {
                    self.show_plot_window = true;
                }
            });
            ui.separator();
            ui.label("Pick a wave and click the button to open the embedded LivePlot window.");
        });

        // Show the embedded plot in its own egui::Window when requested
        if self.show_plot_window {
            let mut open = true;
            egui::Window::new("LivePlot Window")
                .open(&mut open)
                .show(ctx, |ui| {
                    // Optional minimal size for nicer layout
                    ui.set_min_size(egui::vec2(600.0, 300.0));
                    self.plot.ui_embed(ui);
                });
            if !open {
                self.show_plot_window = false;
            }
        }

        // Feed the chosen wave
        let now_us = chrono::Utc::now().timestamp_micros();
        let t = (now_us as f64) * 1e-6;
        let phase = t * 2.0 * std::f64::consts::PI;
        let val = match self.kind {
            WaveKind::Sine => phase.sin(),
            WaveKind::Cosine => phase.cos(),
        };
        let tr = match self.kind {
            WaveKind::Sine => &self.trace_sine,
            WaveKind::Cosine => &self.trace_cos,
        };
        let _ = self.sink.send_point(tr, PlotPoint { x: t, y: val });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let app = DemoApp::new();
    eframe::run_native(
        "LivePlot embedded window demo",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
