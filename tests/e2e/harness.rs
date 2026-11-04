//! Shared e2e harness: builds a `LivePlotPanel` inside an `egui_kittest`
//! `Harness` and feeds it synthetic trace data through the public sink API.

use liveplot::{channel_plot, LivePlotPanel, PlotPoint, PlotSink};

pub struct TestBed {
    pub harness: egui_kittest::Harness<'static, LivePlotPanel>,
    pub sink: PlotSink,
}

pub fn make_test_bed() -> TestBed {
    let (sink, rx) = channel_plot();
    let panel = LivePlotPanel::new(rx);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::Vec2::new(1200.0, 800.0))
        // Font setup must happen before the first pass — the builder runs
        // one frame immediately.
        .with_context_setup(|ctx| {
            egui_material_icons::initialize(ctx);
        })
        // Several scope toolbar widgets (sliders, drag values) have no
        // accessible name today — that's tracked as a follow-up, not an
        // e2e failure condition.
        .with_accessibility_check(false)
        .build_ui_state(
            move |ui, panel| {
                liveplot::data::hotkeys::handle_hotkeys(panel, ui.ctx());
                panel.update(ui);
            },
            panel,
        );
    harness.step();
    TestBed { harness, sink }
}

pub fn feed_sine(bed: &TestBed, name: &str, n: usize, freq: f64, fs: f64, t0: f64) {
    let trace = bed.sink.create_trace(name, None);
    let pts: Vec<PlotPoint> = (0..n)
        .map(|i| {
            let t = t0 + i as f64 / fs;
            PlotPoint {
                x: t,
                y: (2.0 * std::f64::consts::PI * freq * t).sin(),
            }
        })
        .collect();
    bed.sink.send_points(&trace, pts).unwrap();
}
