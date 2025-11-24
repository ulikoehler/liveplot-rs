//! Example: Embedding multiple LivePlot instances in a single egui window
//!
//! What it demonstrates
//! - Arranging four independent `LivePlotApp` instances in a 2 x 2 layout inside one `eframe` window.
//! - Feeding each plot with its own waveform from the surrounding application via `PlotSink`/`Trace` handles.
//! - Starting the host `eframe` window maximized to showcase a dashboard-like layout.
//!
//! How to run
//! ```bash
//! cargo run --example embedded_dashboard
//! ```

use std::time::Duration;

use eframe::{egui, NativeOptions};
use egui_tiles::{Behavior, Container, ContainerKind, TileId, Tiles, Tree, UiResponse};
use liveplot::{channel_plot, LivePlotApp, PlotPoint, PlotSink, Trace, TracesController};

#[derive(Clone, Copy)]
enum PanelWave {
    Sine,
    Cosine,
    Sum { secondary_freq_hz: f64 },
    AmMod { modulation_hz: f64 },
}

struct PlotPanel {
    label: &'static str,
    wave: PanelWave,
    freq_hz: f64,
    phase_cycles: f64,
    sink: PlotSink,
    trace: Trace,
    plot: LivePlotApp,
}

impl PlotPanel {
    fn new(
        label: &'static str,
        wave: PanelWave,
        freq_hz: f64,
        phase_cycles: f64,
        color_rgb: Option<[u8; 3]>,
    ) -> Self {
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

        Self {
            label,
            wave,
            freq_hz,
            phase_cycles,
            sink,
            trace,
            plot,
        }
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

    fn ui(&mut self, ui: &mut egui::Ui, plot_id: egui::Id) {
        let available = ui.available_size();
        let margin = egui::Margin::symmetric(8, 6);
        let margin_width = (margin.left + margin.right) as f32;
        let margin_height = (margin.top + margin.bottom) as f32;
        egui::Frame::group(ui.style())
            .inner_margin(margin)
            .show(ui, |ui| {
                let inner_min = egui::vec2(
                    (available.x - margin_width).max(0.0),
                    (available.y - margin_height).max(0.0),
                );
                ui.set_min_size(inner_min);
                ui.horizontal(|ui| {
                    ui.strong(self.label);
                });
                ui.add_space(4.0);
                let plot_area = ui.available_size();
                ui.allocate_ui(plot_area, |plot_ui| {
                    self.plot.ui_embed_with_id(plot_ui, plot_id);
                });
            });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DashboardPane {
    panel_index: usize,
}

struct DashboardBehavior<'a> {
    panels: &'a mut [PlotPanel],
}

impl<'a> Behavior<DashboardPane> for DashboardBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &DashboardPane) -> egui::WidgetText {
        if let Some(panel) = self.panels.get(pane.panel_index) {
            panel.label.into()
        } else {
            format!("Panel {}", pane.panel_index + 1).into()
        }
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &mut DashboardPane,
    ) -> UiResponse {
        if let Some(panel) = self.panels.get_mut(pane.panel_index) {
            let plot_id = ui.id().with(("embedded_dashboard_plot", tile_id));
            panel.ui(ui, plot_id);
        } else {
            ui.colored_label(egui::Color32::LIGHT_RED, "Missing panel");
        }
        UiResponse::None
    }
}

struct DashboardApp {
    panels: Vec<PlotPanel>,
    tree: Tree<DashboardPane>,
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
        let mut panels: Vec<PlotPanel> = configs
            .into_iter()
            .enumerate()
            .map(|(i, (label, wave, freq, phase))| {
                // Per-panel color hints (u8 RGB)
                let color = match i {
                    0 => Some([102, 204, 255]), // light blue
                    1 => Some([255, 120, 120]), // light red
                    2 => Some([200, 255, 170]), // light green
                    3 => Some([255, 210, 120]), // gold / orange
                    _ => None,
                };
                PlotPanel::new(label, wave, freq, phase, color)
            })
            .collect();

        // Fix the Y range for the "Sine + Cosine" panel (index 2) to [-2, 2]
        if let Some(panel) = panels.get_mut(2) {
            panel.plot.y_min = -2.0;
            panel.plot.y_max = 2.0;
            // Use the fixed range; disable auto-fitting for this panel
            panel.plot.auto_zoom_y = false;
            panel.plot.pending_auto_y = false;
        }

        let tree = Self::build_default_tree(panels.len());
        Self { panels, tree }
    }

    fn build_default_tree(panel_count: usize) -> Tree<DashboardPane> {
        let tree_id = "embedded_dashboard_tiles";
        if panel_count == 0 {
            return Tree::empty(tree_id);
        }

        let mut tiles: Tiles<DashboardPane> = Tiles::default();
        let pane_ids: Vec<_> = (0..panel_count)
            .map(|panel_index| tiles.insert_pane(DashboardPane { panel_index }))
            .collect();

        let mut rows = Vec::new();
        for chunk in pane_ids.chunks(2) {
            rows.push(
                tiles.insert_container(Container::new(ContainerKind::Horizontal, chunk.to_vec())),
            );
        }

        let root = if rows.len() == 1 {
            rows[0]
        } else {
            tiles.insert_container(Container::new(ContainerKind::Vertical, rows))
        };

        Tree::new(tree_id, root, tiles)
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        let desired = ui.available_size();
        if desired.min_elem() <= 0.0 {
            ui.label("Expand the window to see the plots.");
            return;
        }

        ui.allocate_ui(desired, |dashboard_ui| {
            dashboard_ui.set_min_size(desired);
            dashboard_ui.set_clip_rect(dashboard_ui.max_rect());
            self.tree.set_width(desired.x);
            self.tree.set_height(desired.y);
            let mut behavior = DashboardBehavior {
                panels: &mut self.panels,
            };
            self.tree.ui(&mut behavior, dashboard_ui);
        });
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
