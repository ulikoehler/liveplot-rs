//! Example: Embedding multiple LivePlot instances in a single egui window
//!
//! What it demonstrates
//! - Arranging four independent embedded `LivePlotPanel` instances in a 2 x 2 layout inside one `eframe` window.
//! - Feeding each plot with its own waveform from the surrounding application via `PlotSink`/`Trace` handles.
//! - Starting the host `eframe` window maximized to showcase a dashboard-like layout.
//!
//! How to run
//! ```bash
//! cargo run --example embedded_tiles
//! ```

use std::time::Duration;

use eframe::{egui, NativeOptions};
use liveplot::{channel_plot, LivePlotPanel, PlotPoint, PlotSink, Trace, TracesController};

#[derive(Clone, Copy)]
enum PanelWave {
    Sine,
    Cosine,
    Sum { secondary_freq_hz: f64 },
    AmMod { modulation_hz: f64 },
}

struct PlotPanel {
    wave: PanelWave,
    freq_hz: f64,
    phase_cycles: f64,
    sink: PlotSink,
    trace: Trace,
}

impl PlotPanel {
    fn new(
        label: &'static str,
        wave: PanelWave,
        freq_hz: f64,
        phase_cycles: f64,
        color_rgb: Option<[u8; 3]>,
    ) -> (Self, LivePlotPanel) {
        let (sink, rx) = channel_plot();
        let trace = sink.create_trace(label, None);
        let mut plot = LivePlotPanel::new(rx);
        for scope in plot.liveplot_panel.get_data_mut() {
            scope.time_window = 8.0;
        }
        plot.traces_data.max_points = 5_000;
        plot.liveplot_panel.update_data(&plot.traces_data);
        // Attach a per-plot traces controller so we can request a custom color
        let ctrl = TracesController::new();
        plot.set_controllers(None, None, Some(ctrl.clone()), None, None, None, None);

        // If a color hint is supplied, request it via the traces controller (applied during tick)
        if let Some(rgb) = color_rgb {
            ctrl.request_set_color(label, rgb);
        }

        let panel = Self {
            wave,
            freq_hz,
            phase_cycles,
            sink,
            trace,
        };
        (panel, plot)
    }

    fn feed(&self, t: f64) {
        const TAU: f64 = std::f64::consts::PI * 2.0;
        let base_phase = (t * self.freq_hz + self.phase_cycles) * TAU;
        let value = match self.wave {
            PanelWave::Sine => base_phase.sin(),
            PanelWave::Cosine => base_phase.cos(),
            PanelWave::Sum { secondary_freq_hz } => {
                base_phase.sin() + (t * secondary_freq_hz * TAU).cos()
            }
            PanelWave::AmMod { modulation_hz } => {
                let envelope = 1.0 + 0.5 * (t * modulation_hz * TAU).sin();
                base_phase.sin() * envelope
            }
        };
        let _ = self
            .sink
            .send_point(&self.trace, PlotPoint { x: t, y: value });
    }
}

struct DashboardApp {
    panels: Vec<(PlotPanel, LivePlotPanel)>,
}

impl DashboardApp {
    fn new() -> Self {
        let configs = [
            ("Sine 1 Hz", PanelWave::Sine, 1.0, 0.0),
            ("Cosine 0.5 Hz", PanelWave::Cosine, 0.5, 0.25),
            (
                "Sine + Cosine",
                PanelWave::Sum {
                    secondary_freq_hz: 2.0,
                },
                1.5,
                0.0,
            ),
            (
                "AM Sine",
                PanelWave::AmMod { modulation_hz: 0.2 },
                0.75,
                0.5,
            ),
        ];
        let mut panels = Vec::new();
        for (i, (label, wave, freq, phase)) in configs.into_iter().enumerate() {
            // Per-panel color hints (u8 RGB)
            let color = match i {
                0 => Some([102, 204, 255]), // light blue
                1 => Some([255, 120, 120]), // light red
                2 => Some([200, 255, 170]), // light green
                3 => Some([255, 210, 120]), // gold / orange
                _ => None,
            };
            let (panel, plot) = PlotPanel::new(label, wave, freq, phase, color);
            panels.push((panel, plot));
        }

        // Fix the Y range for the "Sine + Cosine" panel (index 2) to [-2, 2]
        if let Some((_panel, plot)) = panels.get_mut(2) {
            if let Some(scope) = plot.liveplot_panel.get_data_mut().first_mut() {
                scope.y_axis.bounds = (-2.0, 2.0);
                scope.y_axis.auto_fit = false;
            }
        }

        Self { panels }
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        let cols = 2;
        let rows = (self.panels.len() + cols - 1) / cols;
        let avail = ui.available_size();
        let cell_w = avail.x / cols as f32;
        let cell_h = avail.y / rows as f32;

        egui::Grid::new("embedded_dashboard_grid")
            .num_columns(cols)
            .spacing([0.0, 0.0])
            .show(ui, |ui| {
                for (idx, (_panel, plot)) in self.panels.iter_mut().enumerate() {
                    let (_, rect) = ui.allocate_space(egui::vec2(cell_w, cell_h));
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    plot.update_embedded(&mut child_ui);

                    if idx % cols == cols - 1 {
                        ui.end_row();
                    }
                }
            });
    }
}

impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now_us = chrono::Utc::now().timestamp_micros();
        let t = (now_us as f64) * 1e-6;
        for (panel, _) in &self.panels {
            panel.feed(t);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Embedded LivePlot dashboard");
            ui.label("Four independent LivePlot instances embedded in a resizable 2 x 2 grid.");
            ui.add_space(8.0);
            self.render_dashboard(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let app = DashboardApp::new();
    eframe::run_native(
        "LivePlot embedded dashboard demo",
        NativeOptions {
            viewport: egui::ViewportBuilder::default().with_maximized(true),
            ..NativeOptions::default()
        },
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
