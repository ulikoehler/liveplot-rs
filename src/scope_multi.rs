//! Multi-trace oscilloscope UI: plots multiple named series with shared controls.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use eframe::{self, egui};
use egui::Color32;
use egui_plot::{Line, Legend, Plot, PlotPoint, PlotPoints, Points, Text};
use image::{Rgba, RgbaImage};

use crate::controllers::{FftController, FftPanelInfo, WindowController, WindowInfo};
use crate::fft;
pub use crate::fft::FftWindow;
use crate::point_selection::PointSelection;
use crate::sink::MultiSample;
use crate::config::XDateFormat;

/// Internal per-trace state (live buffer, optional snapshot, color, cached FFT).
struct TraceState {
    name: String,
    color: Color32,
    live: VecDeque<[f64;2]>,
    snap: Option<VecDeque<[f64;2]>>,
    // Cached last computed FFT (frequency, magnitude)
    last_fft: Option<Vec<[f64;2]>>,
}

/// Egui app that displays multiple traces and supports point selection and FFT.
pub struct ScopeAppMulti {
    pub rx: Receiver<MultiSample>,
    traces: HashMap<String, TraceState>,
    pub trace_order: Vec<String>,
    pub max_points: usize,
    pub time_window: f64,
    pub last_prune: std::time::Instant,
    pub reset_view: bool,
    pub paused: bool,
    /// Optional controller to let external code get/set/listen to window info.
    pub window_controller: Option<WindowController>,
    /// Optional controller to get/set/listen to FFT panel info
    pub fft_controller: Option<FftController>,
    // FFT related
    pub show_fft: bool,
    pub fft_size: usize,
    pub fft_window: FftWindow,
    pub fft_last_compute: std::time::Instant,
    pub fft_db: bool,
    pub fft_fit_view: bool,
    pub request_window_shot: bool,
    pub last_viewport_capture: Option<Arc<egui::ColorImage>>,
    // Point & slope selection (multi-trace)
    /// Selected trace for point/slope selection. None => Free placement (no snapping).
    pub selection_trace: Option<String>,
    /// Index-based selection for the active trace (behaves like single-trace mode).
    pub point_selection: PointSelection,
    /// Formatting of X values in point labels
    pub x_date_format: XDateFormat,
}

impl ScopeAppMulti {
    pub fn new(rx: Receiver<MultiSample>) -> Self {
        Self {
            rx,
            traces: HashMap::new(),
            trace_order: Vec::new(),
            max_points: 10_000,
            time_window: 10.0,
            last_prune: std::time::Instant::now(),
            reset_view: false,
            paused: false,
            show_fft: false,
            fft_size: 1024,
            fft_window: FftWindow::Hann,
            fft_last_compute: std::time::Instant::now(),
            fft_db: false,
            fft_fit_view: false,
            window_controller: None,
            fft_controller: None,
            request_window_shot: false,
            last_viewport_capture: None,
            selection_trace: None,
            point_selection: PointSelection::default(),
            x_date_format: XDateFormat::default(),
        }
    }

    fn alloc_color(index: usize) -> Color32 {
        // Simple distinct color palette
        const PALETTE: [Color32; 10] = [
            Color32::LIGHT_BLUE,
            Color32::LIGHT_RED,
            Color32::LIGHT_GREEN,
            Color32::GOLD,
            Color32::from_rgb(0xAA, 0x55, 0xFF), // purple
            Color32::from_rgb(0xFF, 0xAA, 0x00), // orange
            Color32::from_rgb(0x00, 0xDD, 0xDD), // cyan
            Color32::from_rgb(0xDD, 0x00, 0xDD), // magenta
            Color32::from_rgb(0x66, 0xCC, 0x66), // green2
            Color32::from_rgb(0xCC, 0x66, 0x66), // red2
        ];
        PALETTE[index % PALETTE.len()]
    }
}

impl eframe::App for ScopeAppMulti {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ingest new multi samples
        while let Ok(s) = self.rx.try_recv() {
            let is_new = !self.traces.contains_key(&s.trace);
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                TraceState { name: s.trace.clone(), color: Self::alloc_color(idx), live: VecDeque::new(), snap: None, last_fft: None }
            });
            if is_new && self.selection_trace.is_none() { self.selection_trace = Some(s.trace.clone()); }
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            if entry.live.len() > self.max_points { entry.live.pop_front(); }
        }

        // Prune per-trace based on rolling time window
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            for (_k, tr) in self.traces.iter_mut() {
                if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                    let cutoff = t_latest - self.time_window * 1.15;
                    while let Some(&[t, _]) = tr.live.front() { if t < cutoff { tr.live.pop_front(); } else { break; } }
                }
            }
            self.last_prune = std::time::Instant::now();
        }

        // Controls
        egui::TopBottomPanel::top("controls_multi").show(ctx, |ui| {
            ui.heading("LivePlot (multi)");
            ui.label("Left mouse: pan  |  Right drag: zoom box");
            ui.horizontal(|ui| {
                ui.label("Time window (s):");
                ui.add(egui::Slider::new(&mut self.time_window, 1.0..=60.0));
                ui.label("Points cap:");
                ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));
                // Marker trace selection ("Free" or one trace)
                let mut new_selection = self.selection_trace.clone();
                egui::ComboBox::from_id_salt("marker_trace_select")
                    .selected_text(match &new_selection { Some(s) => format!("Trace: {}", s), None => "Trace: Free".to_owned() })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut new_selection, None, "Free");
                        for name in &self.trace_order { ui.selectable_value(&mut new_selection, Some(name.clone()), name); }
                    });
                if new_selection != self.selection_trace { self.selection_trace = new_selection; }
                if ui.button("Clear Selection").clicked() { self.point_selection.clear(); }
                if ui.button(if self.show_fft { "Hide FFT" } else { "Show FFT" }).clicked() {
                    self.show_fft = !self.show_fft;
                    if let Some(ctrl) = &self.fft_controller {
                        let mut inner = ctrl.inner.lock().unwrap();
                        inner.show = self.show_fft;
                        let info = FftPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
                        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
                    }
                }
                if ui.button(if self.paused { "Resume" } else { "Pause" }).clicked() {
                    if self.paused { // resume
                        self.paused = false;
                        for tr in self.traces.values_mut() { tr.snap = None; }
                    } else { // pause and snapshot
                        for tr in self.traces.values_mut() { tr.snap = Some(tr.live.clone()); }
                        self.paused = true;
                    }
                }
                if ui.button("Reset View").clicked() { self.reset_view = true; }
                if ui.button("Clear").clicked() { for tr in self.traces.values_mut() { tr.live.clear(); if let Some(s) = &mut tr.snap { s.clear(); } } }
                if ui.button("Save PNG").on_hover_text("Take an egui viewport screenshot").clicked() { self.request_window_shot = true; }
            });
        });

        // FFT bottom panel for multi-traces
        if self.show_fft {
            egui::TopBottomPanel::bottom("fft_panel_multi")
                .resizable(true)
                .min_height(120.0)
                .default_height(300.0)
                .show(ctx, |ui| {
                    if let Some(ctrl) = &self.fft_controller {
                        let size_pts = ui.available_size();
                        let ppp = ctx.pixels_per_point();
                        let size_px = [size_pts.x * ppp, size_pts.y * ppp];
                        let mut inner = ctrl.inner.lock().unwrap();
                        inner.current_size = Some(size_px);
                        let info = FftPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
                        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
                    }
                    egui::CollapsingHeader::new("FFT Settings").default_open(true).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("FFT size:");
                            let mut size_log2 = (self.fft_size as f32).log2() as u32;
                            let mut changed = false;
                            let resp = egui::Slider::new(&mut size_log2, 8..=15).text("2^N");
                            if ui.add(resp).changed() { changed = true; }
                            if changed { self.fft_size = 1usize << size_log2; }
                            ui.separator();
                            ui.label("Window:");
                            egui::ComboBox::from_id_salt("fft_window_multi")
                                .selected_text(self.fft_window.label())
                                .show_ui(ui, |ui| { for w in FftWindow::ALL { ui.selectable_value(&mut self.fft_window, *w, w.label()); } });
                            ui.separator();
                            if ui.button(if self.fft_db { "Linear" } else { "dB" }).on_hover_text("Toggle FFT magnitude scale").clicked() { self.fft_db = !self.fft_db; }
                            ui.separator();
                            if ui.button("Fit into view").on_hover_text("Auto scale FFT axes").clicked() { self.fft_fit_view = true; }
                        });
                    });

                    // Compute all FFTs (throttled)
                    if self.fft_last_compute.elapsed() > Duration::from_millis(100) {
                        for name in self.trace_order.clone().into_iter() {
                            if let Some(tr) = self.traces.get_mut(&name) {
                                tr.last_fft = fft::compute_fft(
                                    &tr.live,
                                    self.paused,
                                    &tr.snap,
                                    self.fft_size,
                                    self.fft_window,
                                );
                            }
                        }
                        self.fft_last_compute = std::time::Instant::now();
                    }

                    // Determine overall bounds for optional fit
                    let mut any_spec = false;
                    let mut min_x = f64::INFINITY;
                    let mut max_x = f64::NEG_INFINITY;
                    let mut min_y = f64::INFINITY;
                    let mut max_y = f64::NEG_INFINITY;
                    for name in self.trace_order.clone().into_iter() {
                        if let Some(tr) = self.traces.get(&name) {
                            if let Some(spec) = &tr.last_fft {
                                any_spec = true;
                                if self.fft_db {
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
                        .y_axis_label(if self.fft_db { "Magnitude (dB)" } else { "Magnitude" })
                        .x_axis_label("Hz");
                    if self.fft_fit_view {
                        if min_x.is_finite() { plot = plot.include_x(min_x).include_x(max_x); }
                        if min_y.is_finite() { plot = plot.include_y(min_y).include_y(max_y); }
                        self.fft_fit_view = false; // consume request
                    }

                    let _ = plot.show(ui, |plot_ui| {
                        for name in self.trace_order.clone().into_iter() {
                            if let Some(tr) = self.traces.get(&name) {
                                if let Some(spec) = &tr.last_fft {
                                    let pts: PlotPoints = if self.fft_db {
                                        spec.iter().map(|p| { let mag = p[1].max(1e-12); let y = 20.0 * mag.log10(); [p[0], y] }).collect()
                                    } else {
                                        spec.iter().map(|p| [p[0], p[1]]).collect()
                                    };
                                    let line = Line::new(&tr.name, pts).color(tr.color);
                                    plot_ui.line(line);
                                }
                            }
                        }
                    });
                    if !any_spec { ui.label("FFT: not enough data yet"); }
                });
        }

        // Prepare selection data for currently selected trace (if any)
        let selected_trace_name = self.selection_trace.clone();
        let sel_data_points: Option<Vec<[f64;2]>> = if let Some(name) = &selected_trace_name {
            self.traces.get(name).map(|tr| {
                let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused {
                    if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                } else { Box::new(tr.live.iter()) };
                iter.cloned().collect()
            })
        } else { None };

        // Plot all traces
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut plot = Plot::new("scope_plot_multi")
                .allow_scroll(false)
                .allow_zoom(true)
                .allow_boxed_zoom(true)
                .x_axis_formatter(|x, _range| {
                    let val = x.value; let secs = val as i64; let nsecs = ((val - secs as f64) * 1e9) as u32;
                    let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                    dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
                });
            if self.reset_view { plot = plot.reset(); self.reset_view = false; }
            // Constrain X axis to the configured rolling time window across all traces
            let mut t_latest_overall = f64::NEG_INFINITY;
            for name in self.trace_order.clone().into_iter() {
                if let Some(tr) = self.traces.get(&name) {
                    let last_t = if self.paused { tr.snap.as_ref().and_then(|s| s.back()).map(|p| p[0]) } else { tr.live.back().map(|p| p[0]) };
                    if let Some(t) = last_t { if t > t_latest_overall { t_latest_overall = t; } }
                }
            }
            if t_latest_overall.is_finite() {
                let t_min = t_latest_overall - self.time_window;
                plot = plot.include_x(t_min).include_x(t_latest_overall);
            }
            if self.traces.len() > 1 { plot = plot.legend(Legend::default()); }
            let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
            let marker_font_size = base_body * 1.5;
            let plot_response = plot.show(ui, |plot_ui| {
                for name in self.trace_order.clone().into_iter() {
                    if let Some(tr) = self.traces.get(&name) {
                        let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                        } else { Box::new(tr.live.iter()) };
                        let pts: PlotPoints = iter.cloned().collect();
                        let mut line = Line::new(&tr.name, pts).color(tr.color);
                        if self.traces.len() > 1 { line = line.name(&tr.name); }
                        plot_ui.line(line);
                    }
                }
                // Draw shared selection markers/overlays (same in all modes)
                if let Some(p) = self.point_selection.selected_p1 {
                    plot_ui.points(Points::new("", vec![p]).radius(5.0).color(Color32::YELLOW));
                    let txt = format!("P1\nx={}\ny={:.4}", self.x_date_format.format_value(p[0]), p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::YELLOW);
                    plot_ui.text(Text::new("p1_lbl", PlotPoint::new(p[0], p[1]), rich));
                }
                if let Some(p) = self.point_selection.selected_p2 {
                    plot_ui.points(Points::new("", vec![p]).radius(5.0).color(Color32::LIGHT_BLUE));
                    let txt = format!("P2\nx={}\ny={:.4}", self.x_date_format.format_value(p[0]), p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::LIGHT_BLUE);
                    plot_ui.text(Text::new("p2_lbl", PlotPoint::new(p[0], p[1]), rich));
                }
                if let (Some(p1), Some(p2)) = (self.point_selection.selected_p1, self.point_selection.selected_p2) {
                    plot_ui.line(Line::new("delta", vec![p1, p2]).color(Color32::LIGHT_GREEN));
                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let slope = if dx.abs() > 1e-12 { dy / dx } else { f64::INFINITY };
                    let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                    let overlay = if slope.is_finite() { format!("Δx={:.4}\nΔy={:.4}\nslope={:.4}", dx, dy, slope) } else { format!("Δx=0\nΔy={:.4}\nslope=∞", dy) };
                    let rich = egui::RichText::new(overlay).size(marker_font_size).color(Color32::LIGHT_GREEN);
                    plot_ui.text(Text::new("delta_lbl", PlotPoint::new(mid[0], mid[1]), rich));
                }
            });
            // Handle click for selection in multi mode
            if plot_response.response.clicked() {
                if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                    let transform = plot_response.transform;
                    let plot_pos = transform.value_from_position(screen_pos);
                    match (&selected_trace_name, &sel_data_points) {
                        (Some(_), Some(data_points)) if !data_points.is_empty() => {
                            let mut best_i = 0usize;
                            let mut best_d2 = f64::INFINITY;
                            for (i, p) in data_points.iter().enumerate() {
                                let dx = p[0] - plot_pos.x;
                                let dy = p[1] - plot_pos.y;
                                let d2 = dx*dx + dy*dy;
                                if d2 < best_d2 { best_d2 = d2; best_i = i; }
                            }
                            let p = data_points[best_i];
                            self.point_selection.handle_click_point(p);
                        },
                        _ => {
                            self.point_selection.handle_click_point([plot_pos.x, plot_pos.y]);
                        }
                    }
                }
            }
        });

        // Repaint
        ctx.request_repaint_after(Duration::from_millis(16));

        // Window controller: publish current window info and record any pending requests.
        if let Some(ctrl) = &self.window_controller {
            let rect = ctx.input(|i| i.screen_rect);
            let ppp = ctx.pixels_per_point();
            let mut inner = ctrl.inner.lock().unwrap();
            let size_pts = rect.size();
            inner.current_size = Some([size_pts.x * ppp, size_pts.y * ppp]);
            let info = WindowInfo { current_size: inner.current_size, requested_size: inner.request_set_size, requested_pos: inner.request_set_pos };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // Screenshot request
        if self.request_window_shot { self.request_window_shot = false; ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default())); }
        if let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        }) {
            self.last_viewport_capture = Some(image_arc.clone());
            let default_name = format!("viewport_{:.0}.png", chrono::Local::now().timestamp_millis());
            if let Some(path) = rfd::FileDialog::new().set_file_name(&default_name).save_file() {
                let egui::ColorImage { size: [w, h], pixels, .. } = &*image_arc;
                let mut out = RgbaImage::new(*w as u32, *h as u32);
                for y in 0..*h { for x in 0..*w {
                    let p = pixels[y * *w + x];
                    out.put_pixel(x as u32, y as u32, Rgba([p.r(), p.g(), p.b(), p.a()]));
                }}
                if let Err(e) = out.save(&path) { eprintln!("Failed to save viewport screenshot: {e}"); } else { eprintln!("Saved viewport screenshot to {:?}", path); }
            }
        }
    }
}

/// Run the multi-trace plotting UI with default window title and size.
pub fn run_multi(rx: Receiver<MultiSample>) -> eframe::Result<()> { run_multi_with_options(rx, "LivePlot (multi)", eframe::NativeOptions::default()) }

/// Run the multi-trace plotting UI with custom window title and options.
pub fn run_multi_with_options(
    rx: Receiver<MultiSample>,
    title: &str,
    mut options: eframe::NativeOptions,
) -> eframe::Result<()> {
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    eframe::run_native(title, options, Box::new(|_cc| Ok(Box::new(ScopeAppMulti::new(rx)))))
}

/// Run multi-trace UI with optional controllers attached.
pub fn run_multi_with_options_and_controllers(
    rx: Receiver<MultiSample>,
    title: &str,
    mut options: eframe::NativeOptions,
    window_controller: Option<WindowController>,
    fft_controller: Option<FftController>,
) -> eframe::Result<()> {
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    eframe::run_native(title, options, Box::new(move |_cc| {
        Ok(Box::new({
            let mut app = ScopeAppMulti::new(rx);
            app.window_controller = window_controller.clone();
            app.fft_controller = fft_controller.clone();
            app
        }))
    }))
}

/// Run the multi-trace plotting UI with a custom configuration (time window and point cap).
pub fn run_multi_with_config(rx: Receiver<MultiSample>, cfg: crate::config::LivePlotConfig) -> eframe::Result<()> {
    let title = "LivePlot (multi)";
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    eframe::run_native(title, options, Box::new(|_cc| {
        Ok(Box::new({
            let mut app = ScopeAppMulti::new(rx);
            app.time_window = cfg.time_window_secs;
            app.max_points = cfg.max_points;
            app.x_date_format = cfg.x_date_format;
            app
        }))
    }))
}
