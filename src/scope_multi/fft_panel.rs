#![cfg(feature = "fft")]

use eframe::egui;
use egui_plot::{Line, Legend, Plot, PlotPoints};

use crate::controllers::FFTPanelInfo;
use crate::fft;

use super::app::{FFTWindow, LivePlotApp};
use super::panel::{DockPanel, DockState};

#[derive(Debug, Clone)]
pub struct FFTPanel {
    pub dock: DockState,
}

impl Default for FFTPanel {
    fn default() -> Self {
        Self { dock: DockState::new("📊 FFT") }
    }
}

impl DockPanel for FFTPanel {
    fn dock_mut(&mut self) -> &mut DockState { &mut self.dock }

    fn panel_contents(&mut self, app: &mut LivePlotApp, ui: &mut egui::Ui) {
        // Publish current size to controller (physical px)
        if let Some(ctrl) = &app.fft_controller {
            let size_pts = ui.available_size();
            let ppp = ui.ctx().pixels_per_point();
            let size_px = [size_pts.x * ppp, size_pts.y * ppp];
            let mut inner = ctrl.inner.lock().unwrap();
            inner.current_size = Some(size_px);
            let info = FFTPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        egui::CollapsingHeader::new("FFT Settings").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("FFT size:");
                let mut size_log2 = (app.fft_size as f32).log2() as u32;
                let mut changed = false;
                let resp = egui::Slider::new(&mut size_log2, 8..=15).text("2^N");
                if ui.add(resp).changed() { changed = true; }
                if changed { app.fft_size = 1usize << size_log2; }
                ui.separator();
                ui.label("Window:");
                egui::ComboBox::from_id_salt("fft_window_multi")
                    .selected_text(app.fft_window.label())
                    .show_ui(ui, |ui| { for w in FFTWindow::ALL { ui.selectable_value(&mut app.fft_window, *w, w.label()); } });
                ui.separator();
                if ui.button(if app.fft_db { "Linear" } else { "dB" }).on_hover_text("Toggle FFT magnitude scale").clicked() { app.fft_db = !app.fft_db; }
                ui.separator();
                if ui.button("Fit into view").on_hover_text("Auto scale FFT axes").clicked() { app.fft_fit_view = true; }
            });
        });

        // Compute all FFTs (throttled)
        if app.fft_last_compute.elapsed() > std::time::Duration::from_millis(100) {
            for name in app.trace_order.clone().into_iter() {
                if let Some(tr) = app.traces.get_mut(&name) {
                    tr.last_fft = fft::compute_fft(
                        &tr.live,
                        app.paused,
                        &tr.snap,
                        app.fft_size,
                        app.fft_window,
                    );
                }
            }
            app.fft_last_compute = std::time::Instant::now();
        }

        // Determine overall bounds for optional fit
        let mut any_spec = false;
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for name in app.trace_order.clone().into_iter() {
            if let Some(tr) = app.traces.get(&name) {
                if let Some(spec) = &tr.last_fft {
                    any_spec = true;
                    if app.fft_db {
                        for p in spec.iter() {
                            let y = 20.0 * p[1].max(1e-12).log10();
                            if p[0] < min_x { min_x = p[0]; }
                            if p[0] > max_x { max_x = p[0]; }
                            if y < min_y { min_y = y; }
                            if y > max_y { max_y = y; }
                        }
                    } else {
                        for p in spec.iter() {
                            if p[0] < min_x { min_x = p[0]; }
                            if p[0] > max_x { max_x = p[0]; }
                            if p[1] < min_y { min_y = p[1]; }
                            if p[1] > max_y { max_y = p[1]; }
                        }
                    }
                }
            }
        }

        // Build plot and optionally include bounds
        let mut plot = Plot::new("fft_plot_multi")
            .legend(Legend::default())
            .allow_zoom(true)
            .allow_scroll(false)
            .allow_boxed_zoom(true)
            .y_axis_label(if app.fft_db { "Magnitude (dB)" } else { "Magnitude" })
            .x_axis_label("Hz");
        if app.fft_fit_view {
            if min_x.is_finite() { plot = plot.include_x(min_x).include_x(max_x); }
            if min_y.is_finite() { plot = plot.include_y(min_y).include_y(max_y); }
            app.fft_fit_view = false; // consume request
        }

        let _ = plot.show(ui, |plot_ui| {
            for name in app.trace_order.clone().into_iter() {
                if let Some(tr) = app.traces.get(&name) {
                    if let Some(spec) = &tr.last_fft {
                        let pts: PlotPoints = if app.fft_db {
                            spec.iter().map(|p| { let mag = p[1].max(1e-12); let y = 20.0 * mag.log10(); [p[0], y] }).collect()
                        } else {
                            spec.iter().map(|p| [p[0], p[1]]).collect()
                        };
                        let line = Line::new(&tr.name, pts).color(tr.look.color);
                        plot_ui.line(line);
                    }
                }
            }
        });
        if !any_spec { ui.label("FFT: not enough data yet"); }
    }
}
