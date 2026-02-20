//! Top-level entry point for running LivePlot as a native window.
//!
//! The [`run_liveplot`] function is the primary public API for launching the
//! plot application.  It accepts a command channel receiver and a configuration
//! object, wires up controllers, and enters the eframe event loop.

use eframe::egui;

use crate::PlotCommand;

use super::main_app::MainApp;

/// Launch the LivePlot application in a native window.
///
/// This is the main entry point for standalone use.  It:
///
/// 1. Constructs a [`MainApp`] with the given command channel and any
///    controllers extracted from `cfg`.
/// 2. Applies the configuration (axis units, hotkeys, responsive thresholds, …).
/// 3. Opens a native window and enters the eframe event loop.
///
/// The call blocks until the window is closed.
pub fn run_liveplot(
    rx: std::sync::mpsc::Receiver<PlotCommand>,
    mut cfg: crate::config::LivePlotConfig,
) -> eframe::Result<()> {
    let window_ctrl = cfg.window_controller.take();
    let ui_ctrl = cfg.ui_action_controller.take();
    let traces_ctrl = cfg.traces_controller.take();
    let scopes_ctrl = None;
    let liveplot_ctrl = None;
    let fft_ctrl = cfg.fft_controller.take();
    let threshold_ctrl = cfg.threshold_controller.take();
    let mut app = MainApp::with_controllers(
        rx,
        window_ctrl,
        ui_ctrl,
        traces_ctrl,
        scopes_ctrl,
        liveplot_ctrl,
        fft_ctrl,
        threshold_ctrl,
    );
    app.apply_config(&cfg);

    let title = cfg.title.clone();
    let mut opts = cfg
        .native_options
        .take()
        .unwrap_or_else(eframe::NativeOptions::default);

    // Try to set application icon from icon.svg if available.
    if opts.viewport.icon.is_none() {
        if let Some(icon) = load_app_icon_svg() {
            opts.viewport = egui::ViewportBuilder::default().with_icon(icon);
        }
    }

    // Set a bigger default window size if one is not provided by config.
    if opts.viewport.inner_size.is_none() {
        opts.viewport = opts
            .viewport
            .clone()
            .with_inner_size(egui::vec2(1400.0, 900.0));
    }

    eframe::run_native(
        &title,
        opts,
        Box::new(|cc| {
            // Install Phosphor icon font before creating the app.
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app))
        }),
    )
}

/// Attempt to load the project's `icon.svg` as an [`egui::IconData`].
///
/// Returns `None` if the file does not exist or cannot be parsed/rendered.
fn load_app_icon_svg() -> Option<egui::IconData> {
    let svg_path = concat!(env!("CARGO_MANIFEST_DIR"), "/icon.svg");
    let data = std::fs::read(svg_path).ok()?;

    // Parse and render SVG to RGBA using usvg + resvg.
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).ok()?;
    let size = tree.size().to_int_size();
    if size.width() == 0 || size.height() == 0 {
        return None;
    }
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    let mut canvas = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut canvas);
    let rgba = pixmap.take();
    Some(egui::IconData {
        rgba,
        width: size.width(),
        height: size.height(),
    })
}
