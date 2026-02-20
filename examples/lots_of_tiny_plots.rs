//! Example: Lots of tiny plots (20×15 grid)
//!
//! What it demonstrates
//! - Embedding many `MainPanel` instances in a grid
//! - Each plot shows the same sine waveform but shifted in phase
//! - Each trace receives a unique color (HSV wheel)
//!
//! How to run
//! ```bash
//! cargo run --example lots_of_tiny_plots
//! ```

use eframe::{egui, NativeOptions};
use liveplot::{channel_plot, MainPanel, PlotPoint, PlotSink, Trace, TracesController};
use std::time::Duration;

const COLS: usize = 20;
const ROWS: usize = 15;
const TOTAL: usize = COLS * ROWS;

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
}

impl LotsOfTinyPlotsApp {
    fn new() -> Self {
        let mut plots = Vec::with_capacity(TOTAL);
        for i in 0..TOTAL {
            let phase = (i as f64) / (TOTAL as f64); // cycles [0..1)
            let hue = (i as f64) / (TOTAL as f64);
            let col = hsv_to_rgb(hue, 0.85, 0.9);
            let label = format!("sine_{:03}", i);
            let (p, mp) = TinyPlot::new(&label, phase, col);
            plots.push((p, mp));
        }
        Self { plots }
    }

    fn render_grid(&mut self, ui: &mut egui::Ui) {
        let spacing = ui.spacing().item_spacing;
        let avail_w = ui.available_width();
        let avail_h = ui.available_height();
        // Divide available space evenly, accounting for inter-cell gaps.
        let cell_w = ((avail_w - spacing.x * (COLS as f32 - 1.0)) / COLS as f32).max(1.0);
        let cell_h = ((avail_h - spacing.y * (ROWS as f32 - 1.0)) / ROWS as f32).max(1.0);

        for row in 0..ROWS {
            ui.horizontal(|ui| {
                for col in 0..COLS {
                    let idx = row * COLS + col;
                    let (_p, panel) = &mut self.plots[idx];
                    ui.push_id(idx, |ui| {
                        ui.allocate_ui(egui::vec2(cell_w, cell_h), |ui| {
                            panel.update_embedded(ui);
                        });
                    });
                }
            });
        }
    }
}

impl eframe::App for LotsOfTinyPlotsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // use wall-clock time as sample x
        let now_us = chrono::Utc::now().timestamp_micros();
        let t = (now_us as f64) * 1e-6;
        // feed all plots (same freq, different phase)
        let freq = 1.5;
        for (p, _) in &self.plots {
            p.feed(t, freq);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Lots of tiny sine plots — 20 × 15");
            ui.label("Each plot shows the same sine wave shifted by phase; every trace has its own color.");
            ui.add_space(6.0);
            self.render_grid(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let app = LotsOfTinyPlotsApp::new();
    eframe::run_native(
        "Lots of tiny plots",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
