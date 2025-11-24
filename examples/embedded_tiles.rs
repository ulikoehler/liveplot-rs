//! Example: Embedding multiple LivePlot instances in a single egui window
//!
//! What it demonstrates
//! - Arranging four independent `LivePlotApp` instances in a 2 x 2 layout inside one `eframe` window.
//! - Feeding each plot with its own waveform from the surrounding application via `PlotSink`/`Trace` handles.
//! - Starting the host `eframe` window maximized to showcase a dashboard-like layout.
//!
//! How to run
//! ```bash
//! cargo run --example embedded_tiles --features tiles
//! ```

use std::time::Duration;

use eframe::{egui, NativeOptions};
use egui_tiles::Tree;
use liveplot::tiles::{build_grid_tree, render_tile_grid, LivePlotPaneRef, LivePlotTile};
use liveplot::{channel_plot, LivePlotApp, PlotPoint, PlotSink, Trace, TracesController};

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
    ) -> (Self, LivePlotTile) {
        let (sink, rx) = channel_plot();
        let trace = sink.create_trace(label, None);
        let mut plot = LivePlotApp::new(rx);
        plot.time_window = 8.0;
        plot.max_points = 5_000;
        plot.y_min = -1.25;
        plot.y_max = 1.25;
        plot.auto_zoom_y = true;
        plot.pending_auto_y = true;
        // Attach a per-plot traces controller so we can request a custom color
        let ctrl = TracesController::new();
        plot.traces_controller = Some(ctrl.clone());

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
        let tile = LivePlotTile::new(label, plot);
        (panel, tile)
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
    panels: Vec<PlotPanel>,
    tiles: Vec<LivePlotTile>,
    tree: Tree<LivePlotPaneRef>,
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
        let mut tiles = Vec::new();
        for (i, (label, wave, freq, phase)) in configs.into_iter().enumerate() {
            // Per-panel color hints (u8 RGB)
            let color = match i {
                0 => Some([102, 204, 255]), // light blue
                1 => Some([255, 120, 120]), // light red
                2 => Some([200, 255, 170]), // light green
                3 => Some([255, 210, 120]), // gold / orange
                _ => None,
            };
            let (panel, tile) = PlotPanel::new(label, wave, freq, phase, color);
            panels.push(panel);
            tiles.push(tile);
        }

        // Fix the Y range for the "Sine + Cosine" panel (index 2) to [-2, 2]
        if let Some(tile) = tiles.get_mut(2) {
            let plot = tile.plot_mut();
            plot.y_min = -2.0;
            plot.y_max = 2.0;
            // Use the fixed range; disable auto-fitting for this panel
            plot.auto_zoom_y = false;
            plot.pending_auto_y = false;
        }

        let tree = build_grid_tree("embedded_dashboard_tiles", panels.len(), 2);
        Self {
            panels,
            tiles,
            tree,
        }
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        render_tile_grid(
            ui,
            &mut self.tree,
            &mut self.tiles,
            "embedded_dashboard_plot",
        );
    }
}

impl eframe::App for DashboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now_us = chrono::Utc::now().timestamp_micros();
        let t = (now_us as f64) * 1e-6;
        for panel in &self.panels {
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
