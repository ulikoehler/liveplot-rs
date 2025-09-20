mod point_selection;
mod fft;
mod line_draw;

use std::{collections::{VecDeque, HashMap}, time::Duration};
use eframe::{self, egui};
use egui_plot::{Plot, Line, Legend, PlotPoints, Points, Text, PlotPoint};
use egui::Color32;
use chrono::Local;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub use fft::FftWindow;
use point_selection::PointSelection;

// For PNG export
use image::{RgbaImage, Rgba};
use egui::ViewportCommand;

/// A single input sample with timestamp and value.
#[derive(Debug, Clone)]
pub struct Sample {
    pub index: u64,
    pub value: f64,
    /// Timestamp in microseconds since UNIX epoch
    pub timestamp_micros: i64,
}

/// Convenience sender for feeding `Sample`s into the plotter.
#[derive(Clone)]
pub struct PlotSink {
    tx: Sender<Sample>,
}

impl PlotSink {
    /// Send a complete `Sample` to the plotter. This is a blocking send and will
    /// fail if the receiver has been dropped.
    pub fn send(&self, sample: Sample) -> Result<(), std::sync::mpsc::SendError<Sample>> {
        self.tx.send(sample)
    }

    /// Convenience helper to send using raw fields.
    pub fn send_value(&self, index: u64, value: f64, timestamp_micros: i64) -> Result<(), std::sync::mpsc::SendError<Sample>> {
        let s = Sample { index, value, timestamp_micros };
        self.send(s)
    }
}

/// Create a new channel pair for plotting: `(PlotSink, Receiver<Sample>)`.
///
/// Use the `PlotSink` to send samples from any thread, and pass the `Receiver` to
/// `run()` to start the UI that consumes the samples.
pub fn channel() -> (PlotSink, Receiver<Sample>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (PlotSink { tx }, rx)
}

/// Multi-trace input sample with an associated trace label.
#[derive(Debug, Clone)]
pub struct MultiSample {
    pub index: u64,
    pub value: f64,
    /// Timestamp in microseconds since UNIX epoch
    pub timestamp_micros: i64,
    /// Name of the trace this sample belongs to
    pub trace: String,
}

/// Convenience sender for feeding `MultiSample`s into the multi-trace plotter.
#[derive(Clone)]
pub struct MultiPlotSink {
    tx: Sender<MultiSample>,
}

impl MultiPlotSink {
    pub fn send(&self, sample: MultiSample) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        self.tx.send(sample)
    }
    pub fn send_value<S: Into<String>>(&self, index: u64, value: f64, timestamp_micros: i64, trace: S) -> Result<(), std::sync::mpsc::SendError<MultiSample>> {
        let s = MultiSample { index, value, timestamp_micros, trace: trace.into() };
        self.send(s)
    }
}

/// Create a new channel pair for multi-trace plotting: `(MultiPlotSink, Receiver<MultiSample>)`.
pub fn channel_multi() -> (MultiPlotSink, Receiver<MultiSample>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (MultiPlotSink { tx }, rx)
}

/// Current window information (physical pixels)
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub current_size: Option<[f32; 2]>,
    pub requested_size: Option<[f32; 2]>,
    pub requested_pos: Option<[f32; 2]>,
}

/// Controller to get/set window info and subscribe to updates.
#[derive(Clone)]
pub struct WindowController {
    inner: Arc<Mutex<WindowCtrlInner>>,
}

struct WindowCtrlInner {
    current_size: Option<[f32; 2]>,
    request_set_size: Option<[f32; 2]>,
    request_set_pos: Option<[f32; 2]>,
    listeners: Vec<Sender<WindowInfo>>,
}

impl WindowController {
    /// Create a fresh controller.
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(WindowCtrlInner { current_size: None, request_set_size: None, request_set_pos: None, listeners: Vec::new() })) }
    }

    /// Get the last observed window size in physical pixels (if known).
    pub fn get_current_size(&self) -> Option<[f32;2]> {
        self.inner.lock().unwrap().current_size
    }

    /// Request a window size change (physical pixels). The request is recorded and
    /// will be broadcast to listeners; whether the runtime honors it depends on the backend.
    pub fn request_set_size(&self, size_px: [f32;2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_size = Some(size_px);
    }

    /// Request a window position change (physical pixels). Recorded and broadcast to listeners.
    pub fn request_set_pos(&self, pos_px: [f32;2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_pos = Some(pos_px);
    }

    /// Subscribe to window info updates. Returned receiver receives `WindowInfo` whenever the UI publishes it.
    pub fn subscribe(&self) -> Receiver<WindowInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        rx
    }
}

fn compute_fft_if_possible(app: &ScopeApp) -> Option<Vec<[f64;2]>> {
    fft::compute_fft(
        &app.buffer_live,
        app.paused,
        &app.buffer_snapshot,
        app.fft_size,
        app.fft_window,
    )
}

/// Egui application implementing the plotting UI for a stream of `Sample`s.
pub struct ScopeApp {
    pub rx: Receiver<Sample>,
    /// Optional controller to let external code get/set/listen to window info.
    pub window_controller: Option<WindowController>,
    /// Live rolling buffer continuously filled with incoming samples.
    pub buffer_live: VecDeque<[f64; 2]>,
    /// Snapshot of `buffer_live` taken at the moment the user pauses. Displayed while paused.
    pub buffer_snapshot: Option<VecDeque<[f64; 2]>>,
    pub max_points: usize,
    pub time_window: f64,
    pub last_prune: std::time::Instant,
    pub reset_view: bool,
    pub paused: bool,
    // FFT related
    pub show_fft: bool,
    pub fft_size: usize,
    pub fft_window: FftWindow,
    pub fft_overlap: f32, // fraction 0.. <1
    pub last_fft: Option<Vec<[f64;2]>>, // frequency,magnitude
    pub fft_last_compute: std::time::Instant,
    pub fft_db: bool, // display magnitude in dB if true
    pub fft_fit_view: bool, // request to fit FFT plot bounds
    // Point selection (time-domain)
    pub point_selection: PointSelection,
    pub request_window_shot: bool,
    pub last_viewport_capture: Option<Arc<egui::ColorImage>>, // retained screenshot
}

impl ScopeApp {
    /// Construct the app around a sample receiver.
    pub fn new(rx: Receiver<Sample>) -> Self {
        Self {
            rx,
            window_controller: None,
            buffer_live: VecDeque::new(),
            buffer_snapshot: None,
            max_points: 10_000,
            time_window: 10.0,
            last_prune: std::time::Instant::now(),
            reset_view: false,
            paused: false,
            show_fft: false,
            fft_size: 1024,
            fft_window: FftWindow::Hann,
            fft_overlap: 0.5,
            last_fft: None,
            fft_last_compute: std::time::Instant::now(),
            fft_db: false,
            fft_fit_view: false,
            point_selection: PointSelection::default(),
            request_window_shot: false,
            last_viewport_capture: None,
        }
    }
}

impl eframe::App for ScopeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Always ingest new samples into the live buffer (even while paused) so data isn't lost.
        while let Ok(sample) = self.rx.try_recv() {
            let t = sample.timestamp_micros as f64 * 1e-6;
            let y = sample.value;
            self.buffer_live.push_back([t, y]);
            if self.buffer_live.len() > self.max_points {
                self.buffer_live.pop_front();
                // Adjust selections for single element removal from front (live only)
                if !self.paused { self.point_selection.adjust_for_front_removal(1); }
            }
        }

        // Periodically prune to keep only the most recent time window
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            if let Some((&[t_latest, _], _)) = self.buffer_live.back().map(|x| (x, ())) {
                let cutoff = t_latest - self.time_window;
                let mut removed = 0usize;
                while let Some(&[t, _]) = self.buffer_live.front() {
                    if t < cutoff {
                        self.buffer_live.pop_front();
                        removed += 1;
                    } else {
                        break;
                    }
                }
                if removed > 0 && !self.paused {
                    self.point_selection.adjust_for_front_removal(removed);
                }
            }
            self.last_prune = std::time::Instant::now();
            // If we're paused we do NOT mutate the snapshot; it's a static view.
        }

        // Decide which buffer to display: live (normal) or snapshot (paused)
        let display_iter: Box<dyn Iterator<Item = &[f64; 2]> + '_> = if self.paused {
            if let Some(snapshot) = &self.buffer_snapshot { Box::new(snapshot.iter()) } else { Box::new(self.buffer_live.iter()) }
        } else { Box::new(self.buffer_live.iter()) };
        let data_points: Vec<[f64;2]> = display_iter.map(|&[t,y]| [t,y]).collect();
        // Invalidate selections if indices out of range after pruning/live update
        self.point_selection.invalidate_out_of_range(data_points.len());
        let plot_points: PlotPoints = data_points.clone().into();
        let line = Line::new("sine", plot_points);

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.heading("LivePlot");
            ui.label("Left mouse: pan/select  |  Right mouse drag: zoom box");
            ui.horizontal(|ui| {
                ui.label("Time window (s):");
                ui.add(egui::Slider::new(&mut self.time_window, 1.0..=60.0));
                ui.label("Points cap:");
                ui.add(egui::Slider::new(&mut self.max_points, 5_000..=200_000));
                if ui.button(if self.show_fft { "Hide FFT" } else { "Show FFT" }).clicked() { self.show_fft = !self.show_fft; }
                if ui.button(if self.paused { "Resume" } else { "Pause" }).clicked() {
                    if self.paused {
                        // Transition to running: discard snapshot
                        self.paused = false;
                        self.buffer_snapshot = None;
                    } else {
                        // Transition to paused: capture snapshot
                        self.buffer_snapshot = Some(self.buffer_live.clone());
                        self.paused = true;
                    }
                }
                if ui.button("Reset View").clicked() {
                    self.reset_view = true;
                }
                if ui.button("Clear").clicked() {
                    self.buffer_live.clear();
                    if let Some(snapshot) = &mut self.buffer_snapshot {
                        snapshot.clear();
                    }
                }
                if ui.button("Clear Selection").clicked() {
                    self.point_selection.clear();
                }
                if ui.button("Save PNG").on_hover_text("Take an egui viewport screenshot").clicked() { self.request_window_shot = true; }
            });
        });

        // Layout depends on whether FFT is shown. If shown, place it in a resizable bottom panel.
        if self.show_fft {
            egui::TopBottomPanel::bottom("fft_panel")
                .resizable(true)
                .min_height(120.0)
                .default_height(300.0)
                .show(ctx, |ui| {
                    // FFT controls + plot
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
                            egui::ComboBox::from_id_salt("fft_window")
                                .selected_text(self.fft_window.label())
                                .show_ui(ui, |ui| {
                                    for w in FftWindow::ALL { ui.selectable_value(&mut self.fft_window, *w, w.label()); }
                                });
                            ui.separator();
                            ui.label("Overlap:");
                            ui.add(egui::Slider::new(&mut self.fft_overlap, 0.0..=0.9).step_by(0.05));
                            ui.separator();
                            if ui.button(if self.fft_db { "Linear" } else { "dB" }).on_hover_text("Toggle FFT magnitude scale").clicked() { self.fft_db = !self.fft_db; }
                            ui.separator();
                            if ui.button("Fit into view").on_hover_text("Auto scale FFT axes").clicked() { self.fft_fit_view = true; }
                        });
                    });

                    // Compute / update FFT (throttle)
                    if self.fft_last_compute.elapsed() > Duration::from_millis(100) {
                        if let Some(points) = compute_fft_if_possible(self) { self.last_fft = Some(points); }
                        self.fft_last_compute = std::time::Instant::now();
                    }
                    if let Some(spec) = &self.last_fft {
                        let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
                        let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
                        let fft_points: PlotPoints = if self.fft_db {
                            spec.iter().map(|p| { let mag = p[1].max(1e-12); let y = 20.0 * mag.log10(); if p[0]<min_x{min_x=p[0];} if p[0]>max_x{max_x=p[0];} if y<min_y{min_y=y;} if y>max_y{max_y=y;} [p[0], y] }).collect()
                        } else {
                            spec.iter().map(|p| { if p[0]<min_x{min_x=p[0];} if p[0]>max_x{max_x=p[0];} if p[1]<min_y{min_y=p[1];} if p[1]>max_y{max_y=p[1];} [p[0], p[1]] }).collect()
                        };
                        let mut fft_plot = Plot::new("fft_plot")
                            .legend(Legend::default())
                            .allow_zoom(true)
                            .allow_scroll(false)
                            .allow_boxed_zoom(true)
                            .y_axis_label(if self.fft_db { "Magnitude (dB)" } else { "Magnitude" })
                            .x_axis_label("Hz");
                        if self.fft_fit_view {
                            if min_x.is_finite() { fft_plot = fft_plot.include_x(min_x).include_x(max_x); }
                            if min_y.is_finite() { fft_plot = fft_plot.include_y(min_y).include_y(max_y); }
                            self.fft_fit_view = false; // consume request
                        }
                        let fft_line = Line::new(if self.fft_db { "FFT (dB)" } else { "FFT" }, fft_points);
                        let _ = fft_plot.show(ui, |plot_ui| { plot_ui.line(fft_line); });
                    } else { ui.label("FFT: not enough data yet"); }
                });
        }

        // Time-domain central panel (fills remaining space; entire window if FFT hidden)
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut plot = Plot::new("scope_plot")
                .legend(Legend::default())
                .allow_scroll(false)
                .allow_zoom(true)
                .allow_boxed_zoom(true)
                .x_axis_formatter(|x, _range| {
                    let val = x.value;
                    let secs = val as i64;
                    let nsecs = ((val - secs as f64) * 1e9) as u32;
                    // Use new chrono DateTime::from_timestamp API (UTC) then convert to Local
                    let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                    dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string()
                });
            if self.reset_view { plot = plot.reset(); self.reset_view = false; }
            let data_points_ref = &data_points; // alias for closure capture
            let selected1 = self.point_selection.selected_idx1;
            let selected2 = self.point_selection.selected_idx2;
            // Determine base font size and compute marker font size (50% larger)
            let base_body = ctx.style().text_styles[&egui::TextStyle::Body].size;
            let marker_font_size = base_body * 1.5;
            let plot_response = plot.show(ui, |plot_ui| {
                // Draw the base line
                plot_ui.line(line);

                // Draw selected points if any
                if let Some(i) = selected1 { if let Some(p) = data_points_ref.get(i) {
                    plot_ui.points(Points::new("", vec![*p]).radius(5.0).color(Color32::YELLOW));
                    let txt = format!("P1\nx={:.4}\ny={:.4}", p[0], p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::YELLOW);
                    plot_ui.text(Text::new("p1_lbl", PlotPoint::new(p[0], p[1]), rich));
                }}
                if let Some(i) = selected2 { if let Some(p) = data_points_ref.get(i) {
                    plot_ui.points(Points::new("", vec![*p]).radius(5.0).color(Color32::LIGHT_BLUE));
                    let txt = format!("P2\nx={:.4}\ny={:.4}", p[0], p[1]);
                    let rich = egui::RichText::new(txt).size(marker_font_size).color(Color32::LIGHT_BLUE);
                    plot_ui.text(Text::new("p2_lbl", PlotPoint::new(p[0], p[1]), rich));
                }}
                // If both selected, draw line and overlay with deltas & slope
                if let (Some(i1), Some(i2)) = (selected1, selected2) {
                    if let (Some(p1), Some(p2)) = (data_points_ref.get(i1), data_points_ref.get(i2)) {
                        plot_ui.line(Line::new("delta", vec![*p1, *p2]).color(Color32::LIGHT_GREEN));
                        let dx = p2[0] - p1[0];
                        let dy = p2[1] - p1[1];
                        let slope = if dx.abs() > 1e-12 { dy / dx } else { f64::INFINITY };
                        let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                        let overlay = if slope.is_finite() {
                            format!("Δx={:.4}\nΔy={:.4}\nslope={:.4}", dx, dy, slope)
                        } else { format!("Δx=0\nΔy={:.4}\nslope=∞", dy) };
                        let rich = egui::RichText::new(overlay).size(marker_font_size).color(Color32::LIGHT_GREEN);
                        plot_ui.text(Text::new("delta_lbl", PlotPoint::new(mid[0], mid[1]), rich));
                    }
                }
            });
            // Handle click for (de)selection after drawing so transformed points are available
            if plot_response.response.clicked() {
                if let Some(screen_pos) = plot_response.response.interact_pointer_pos() {
                    let transform = plot_response.transform; // direct
                    let plot_pos = transform.value_from_position(screen_pos);
                    if !data_points.is_empty() {
                        let mut best_i = 0usize;
                        let mut best_d2 = f64::INFINITY;
                        for (i, p) in data_points.iter().enumerate() {
                            let dx = p[0] - plot_pos.x;
                            let dy = p[1] - plot_pos.y;
                            let d2 = dx*dx + dy*dy;
                            if d2 < best_d2 { best_d2 = d2; best_i = i; }
                        }
                        self.point_selection.handle_click(best_i);
                    }
                }
            }
        });

        // Continuously repaint so it feels real-time
        ctx.request_repaint_after(Duration::from_millis(16));

        // Window controller: publish current window info and record any pending requests.
        if let Some(ctrl) = &self.window_controller {
            // Obtain screen rect and pixels per point from egui context
            let rect = ctx.input(|i| i.screen_rect);
            let ppp = ctx.pixels_per_point();
            let mut inner = ctrl.inner.lock().unwrap();
            // Update current size (in physical pixels)
            let size_pts = rect.size();
            inner.current_size = Some([size_pts.x * ppp, size_pts.y * ppp]);

            // We do NOT attempt to change the native window here because that's
            // backend/platform dependent and eframe does not expose a portable API.
            // Instead, requested set size/pos are recorded in `request_set_*` and
            // exposed to subscribers via the broadcast below.

            // Broadcast current info to listeners (non-blocking)
            let info = WindowInfo {
                current_size: inner.current_size,
                requested_size: inner.request_set_size,
                requested_pos: inner.request_set_pos,
            };
            inner.listeners.retain(|s| s.send(info.clone()).is_ok());
        }

        // Perform deferred window screenshot (after UI drawn)
        if self.request_window_shot {
            self.request_window_shot = false;
            // Ask egui to perform a viewport screenshot; result arrives as Event::Screenshot.
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }

        // Collect latest screenshot event and optionally prompt save
        if let Some(image_arc) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        }) {
            self.last_viewport_capture = Some(image_arc.clone());
            // Offer immediate save dialog (non-blocking to logic) on receipt
            let default_name = format!("viewport_{:.0}.png", chrono::Local::now().timestamp_millis());
            if let Some(path) = rfd::FileDialog::new().set_file_name(&default_name).save_file() {
                // Convert egui::ColorImage to PNG via image crate
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

/// Run the plotting UI with default window title and size.
pub fn run(rx: Receiver<Sample>) -> eframe::Result<()> {
    run_with_options(rx, "LivePlot", eframe::NativeOptions::default())
}

/// Run the plotting UI with custom window title and options.
pub fn run_with_options(
    rx: Receiver<Sample>,
    title: &str,
    mut options: eframe::NativeOptions,
) -> eframe::Result<()> {
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    eframe::run_native(title, options, Box::new(|_cc| Ok(Box::new(ScopeApp::new(rx)))))
}

// ============================== Multi-trace app ===============================

struct TraceState {
    name: String,
    color: Color32,
    live: VecDeque<[f64;2]>,
    snap: Option<VecDeque<[f64;2]>>,
}

pub struct ScopeAppMulti {
    pub rx: Receiver<MultiSample>,
    traces: HashMap<String, TraceState>,
    pub trace_order: Vec<String>,
    pub max_points: usize,
    pub time_window: f64,
    pub last_prune: std::time::Instant,
    pub reset_view: bool,
    pub paused: bool,
    pub request_window_shot: bool,
    pub last_viewport_capture: Option<Arc<egui::ColorImage>>,
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
            request_window_shot: false,
            last_viewport_capture: None,
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
            let entry = self.traces.entry(s.trace.clone()).or_insert_with(|| {
                let idx = self.trace_order.len();
                self.trace_order.push(s.trace.clone());
                TraceState { name: s.trace.clone(), color: Self::alloc_color(idx), live: VecDeque::new(), snap: None }
            });
            let t = s.timestamp_micros as f64 * 1e-6;
            entry.live.push_back([t, s.value]);
            if entry.live.len() > self.max_points { entry.live.pop_front(); }
        }

        // Prune per-trace based on rolling time window
        if self.last_prune.elapsed() > Duration::from_millis(200) {
            for (_k, tr) in self.traces.iter_mut() {
                if let Some((&[t_latest, _], _)) = tr.live.back().map(|x| (x, ())) {
                    let cutoff = t_latest - self.time_window;
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
                if ui.button("Clear").clicked() {
                    for tr in self.traces.values_mut() { tr.live.clear(); if let Some(s) = &mut tr.snap { s.clear(); } }
                }
                if ui.button("Save PNG").on_hover_text("Take an egui viewport screenshot").clicked() { self.request_window_shot = true; }
            });
        });

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
            if self.traces.len() > 1 { plot = plot.legend(Legend::default()); }
            let _resp = plot.show(ui, |plot_ui| {
                for name in self.trace_order.clone().into_iter() {
                    if let Some(tr) = self.traces.get(&name) {
                        let iter: Box<dyn Iterator<Item=&[f64;2]> + '_> = if self.paused {
                            if let Some(snap) = &tr.snap { Box::new(snap.iter()) } else { Box::new(tr.live.iter()) }
                        } else { Box::new(tr.live.iter()) };
                        let pts: PlotPoints = iter.cloned().collect();
                        let mut line = Line::new(&tr.name, pts).color(tr.color);
                        // Ensure legend only shows when multiple traces
                        if self.traces.len() > 1 { line = line.name(&tr.name); }
                        plot_ui.line(line);
                    }
                }
            });
        });

        // Repaint
        ctx.request_repaint_after(Duration::from_millis(16));

        // Screenshot request
        if self.request_window_shot {
            self.request_window_shot = false;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
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
pub fn run_multi(rx: Receiver<MultiSample>) -> eframe::Result<()> {
    run_multi_with_options(rx, "LivePlot (multi)", eframe::NativeOptions::default())
}

/// Run the multi-trace plotting UI with custom window title and options.
pub fn run_multi_with_options(
    rx: Receiver<MultiSample>,
    title: &str,
    mut options: eframe::NativeOptions,
) -> eframe::Result<()> {
    options.viewport = egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]);
    eframe::run_native(title, options, Box::new(|_cc| Ok(Box::new(ScopeAppMulti::new(rx)))))
}
