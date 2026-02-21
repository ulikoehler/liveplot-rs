//! Example: Color scheme picker
//!
//! What it demonstrates
//! - Using the [`ColorScheme`] enum to set a predefined visual theme.
//! - Switching color scheme at startup via command-line argument.
//!
//! How to run
//! ```bash
//! cargo run --example color_scheme                   # default (Dark)
//! cargo run --example color_scheme -- solarized-dark # Solarized Dark
//! cargo run --example color_scheme -- nord           # Nord
//! cargo run --example color_scheme -- dracula        # Dracula
//! ```
//!
//! Available schemes: dark, light, solarized-dark, solarized-light, ggplot,
//! nord, monokai, dracula, gruvbox-dark, high-contrast.

use liveplot::{channel_plot, ColorScheme, LivePlotApp, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn parse_scheme(arg: &str) -> ColorScheme {
    match arg.to_ascii_lowercase().as_str() {
        "dark" => ColorScheme::Dark,
        "light" => ColorScheme::Light,
        "solarized-dark" | "solarizeddark" => ColorScheme::SolarizedDark,
        "solarized-light" | "solarizedlight" => ColorScheme::SolarizedLight,
        "ggplot" | "ggplot2" => ColorScheme::GgPlot,
        "nord" => ColorScheme::Nord,
        "monokai" => ColorScheme::Monokai,
        "dracula" => ColorScheme::Dracula,
        "gruvbox-dark" | "gruvboxdark" | "gruvbox" => ColorScheme::GruvboxDark,
        "high-contrast" | "highcontrast" | "hc" => ColorScheme::HighContrast,
        other => {
            eprintln!(
                "Unknown color scheme '{}', falling back to Dark.\n\
                 Available: dark, light, solarized-dark, solarized-light, ggplot, \
                 nord, monokai, dracula, gruvbox-dark, high-contrast",
                other
            );
            ColorScheme::Dark
        }
    }
}

use eframe::egui::{self, ComboBox};

struct ColorSchemePickerApp {
    scheme: ColorScheme,
    plot: LivePlotApp,
}

impl eframe::App for ColorSchemePickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply color scheme
        self.scheme.apply(ctx);

        egui::TopBottomPanel::top("color_scheme_picker_top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Color scheme:");
                ComboBox::from_id_salt("color_scheme_picker")
                    .selected_text(self.scheme.label())
                    .show_ui(ui, |ui| {
                        for scheme in ColorScheme::all() {
                            let label = scheme.label();
                            if ui.selectable_label(self.scheme == *scheme, label).clicked() {
                                self.scheme = scheme.clone();
                            }
                        }
                    });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.plot.main_panel.update_embedded(ui);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let scheme = std::env::args()
        .nth(1)
        .map(|a| parse_scheme(&a))
        .unwrap_or(ColorScheme::Dark);

    let (sink, rx) = channel_plot();
    let tr_sine = sink.create_trace("sine", None);
    let tr_cos = sink.create_trace("cosine", None);

    // Producer: 1 kHz, 3 Hz sine + cosine
    std::thread::spawn(move || {
        const FS_HZ: f64 = 1000.0;
        const F_HZ: f64 = 3.0;
        let dt = Duration::from_millis(1);
        let mut n: u64 = 0;
        loop {
            let t = n as f64 / FS_HZ;
            let s_val = (2.0 * std::f64::consts::PI * F_HZ * t).sin();
            let c_val = (2.0 * std::f64::consts::PI * F_HZ * t).cos();
            let t_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let _ = sink.send_point(&tr_sine, PlotPoint { x: t_s, y: s_val });
            let _ = sink.send_point(&tr_cos, PlotPoint { x: t_s, y: c_val });
            n = n.wrapping_add(1);
            std::thread::sleep(dt);
        }
    });

    let plot = LivePlotApp::new(rx);
    let app = ColorSchemePickerApp { scheme, plot };

    eframe::run_native(
        "Color Scheme Demo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
