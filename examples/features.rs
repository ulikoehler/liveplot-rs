//! Example: Feature flags demo
//!
//! What it demonstrates
//! - Dynamically toggling the boolean options defined in
//!   [`LivePlotConfig::features`] via UI checkboxes.
//! - Using a `LivePlotApp` instance and feeding it a simple
//!   sine/cosine waveform (basically the same producer used in the
//!   `sine_cosine` example).
//!
//! The checkboxes appear at the top of the window and will modify
//! various parts of the live-plot UI as the user flips them.  Not all
//! flags have a visible effect (some are placeholders in the
//! configuration), but this example shows how to inspect and apply the
//! settings at runtime.
//!
//! How to run
//! ```bash
//! cargo run --example features
//! ```

use eframe::{egui, NativeOptions};
use liveplot::config::ScopeButton;
use liveplot::{channel_plot, FeatureFlags, LivePlotApp, PlotPoint};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Application state for the example.
struct FeaturesApp {
    plot: LivePlotApp,
    features: FeatureFlags,
    // producer handles so we can push data from a background thread
    _sink: liveplot::PlotSink,
    _tr_sine: liveplot::Trace,
    _tr_cos: liveplot::Trace,
}

impl FeaturesApp {
    fn new() -> Self {
        // create shared sink/receiver pair and register traces
        let (sink, rx) = channel_plot();
        let tr_sine = sink.create_trace("sine", None);
        let tr_cos = sink.create_trace("cosine", None);

        // spawn the producer thread (1 kHz sample rate as in sine_cosine.rs)
        let sink_clone = sink.clone();
        let sine_clone = tr_sine.clone();
        let cos_clone = tr_cos.clone();
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
                let _ = sink_clone.send_point(&sine_clone, PlotPoint { x: t_s, y: s_val });
                let _ = sink_clone.send_point(&cos_clone, PlotPoint { x: t_s, y: c_val });
                n = n.wrapping_add(1);
                std::thread::sleep(dt);
            }
        });

        let plot = LivePlotApp::new(rx);
        let features = FeatureFlags::default();

        Self {
            plot,
            features,
            _sink: sink,
            _tr_sine: tr_sine,
            _tr_cos: tr_cos,
        }
    }

    /// Apply the currently selected feature flags to the embedded plot
    /// panel.  This mutates fields on `LivePlotPanel` and some of the
    /// underlying `ScopeData`/`TraceLook` structures so that toggles have
    /// an immediate visible effect.
    fn apply_features(&mut self) {
        let f = &self.features;
        let panel = &mut self.plot.main_panel;

        // Determine button lists based on feature flags.  We start with the
        // full default set and drop any that have been explicitly disabled.
        let mut btns = ScopeButton::all_defaults();
        if !f.pause_resume {
            btns.retain(|b| *b != ScopeButton::PauseResume);
        }
        if !f.clear_all {
            btns.retain(|b| *b != ScopeButton::ClearAll);
        }
        if !f.scopes {
            // the "Scopes" button is purely navigational; hiding it is
            // the only behaviour we can control here.
            btns.retain(|b| *b != ScopeButton::Scopes);
        }

        panel.top_bar_buttons = if f.top_bar {
            // show the filtered default list in the top bar
            Some(btns.clone())
        } else {
            Some(vec![])
        };
        panel.sidebar_buttons = if f.sidebar { Some(btns) } else { Some(vec![]) };

        // legend / grid toggle
        for scope in panel.liveplot_panel.get_data_mut() {
            scope.show_legend = f.legend;
            scope.show_grid = f.grid;
        }

        // tick-label thresholds via the helper method
        panel.liveplot_panel.set_tick_label_thresholds(
            if f.y_tick_labels {
                250.0
            } else {
                f32::INFINITY
            },
            if f.x_tick_labels {
                200.0
            } else {
                f32::INFINITY
            },
        );

        // rebuild right-side panels list based on feature flags, but
        // keep existing panels around so their internal state (visibility,
        // detachment, etc.) isn't wiped each frame.  This mirrors the
        // strategy we use for the FFT bottom panel above.
        {
            let hk = panel.hotkeys.clone();

            // remove any panels whose feature has been disabled
            panel.right_side_panels.retain(|p| {
                if p.downcast_ref::<liveplot::panels::traces_ui::TracesPanel>().is_some() {
                    f.sidebar
                } else if p.downcast_ref::<liveplot::panels::math_ui::MathPanel>().is_some() {
                    f.sidebar && f.math
                } else if p.downcast_ref::<liveplot::panels::hotkeys_ui::HotkeysPanel>().is_some() {
                    f.sidebar && f.hotkeys
                } else if p
                    .downcast_ref::<liveplot::panels::thresholds_ui::ThresholdsPanel>()
                    .is_some()
                {
                    f.sidebar && f.thresholds
                } else if p
                    .downcast_ref::<liveplot::panels::triggers_ui::TriggersPanel>()
                    .is_some()
                {
                    f.sidebar && f.triggers
                } else if p
                    .downcast_ref::<liveplot::panels::measurment_ui::MeasurementPanel>()
                    .is_some()
                {
                    f.sidebar && f.measurement
                } else {
                    // unknown panel type, keep it
                    true
                }
            });

            // add missing panels for which the feature is enabled
            if f.sidebar && !panel
                .right_side_panels
                .iter()
                .any(|p| p.downcast_ref::<liveplot::panels::traces_ui::TracesPanel>().is_some())
            {
                panel
                    .right_side_panels
                    .push(Box::new(liveplot::panels::traces_ui::TracesPanel::default()));
            }
            if f.sidebar && f.math && !panel
                .right_side_panels
                .iter()
                .any(|p| p.downcast_ref::<liveplot::panels::math_ui::MathPanel>().is_some())
            {
                panel
                    .right_side_panels
                    .push(Box::new(liveplot::panels::math_ui::MathPanel::default()));
            }
            if f.sidebar && f.hotkeys && !panel
                .right_side_panels
                .iter()
                .any(|p| p.downcast_ref::<liveplot::panels::hotkeys_ui::HotkeysPanel>().is_some())
            {
                panel
                    .right_side_panels
                    .push(Box::new(liveplot::panels::hotkeys_ui::HotkeysPanel::new(
                        hk.clone(),
                    )));
            }
            if f.sidebar && f.thresholds && !panel
                .right_side_panels
                .iter()
                .any(|p| {
                    p.downcast_ref::<liveplot::panels::thresholds_ui::ThresholdsPanel>()
                        .is_some()
                })
            {
                panel
                    .right_side_panels
                    .push(Box::new(
                        liveplot::panels::thresholds_ui::ThresholdsPanel::default(),
                    ));
            }
            if f.sidebar && f.triggers && !panel
                .right_side_panels
                .iter()
                .any(|p| p.downcast_ref::<liveplot::panels::triggers_ui::TriggersPanel>().is_some())
            {
                panel
                    .right_side_panels
                    .push(Box::new(liveplot::panels::triggers_ui::TriggersPanel::default()));
            }
            if f.sidebar && f.measurement && !panel
                .right_side_panels
                .iter()
                .any(|p| {
                    p.downcast_ref::<liveplot::panels::measurment_ui::MeasurementPanel>()
                        .is_some()
                })
            {
                panel
                    .right_side_panels
                    .push(Box::new(
                        liveplot::panels::measurment_ui::MeasurementPanel::default(),
                    ));
            }
        }

        #[cfg(feature = "fft")]
        {
            // Rather than rebuild the bottom-panels list on every frame (which
            // would reset each panel's `PanelState` and make it impossible to
            // show the FFT panel after clicking the button), we only add or
            // remove the FFT panel when the corresponding feature flag
            // changes.  This keeps the panel object alive across frames so
            // its `visible`/`detached` state is preserved.
            if f.fft {
                let has_fft = panel
                    .bottom_panels
                    .iter()
                    .any(|p| p.downcast_ref::<liveplot::panels::fft_ui::FftPanel>().is_some());
                if !has_fft {
                    panel
                        .bottom_panels
                        .push(Box::new(liveplot::panels::fft_ui::FftPanel::default()));
                }
            } else {
                panel
                    .bottom_panels
                    .retain(|p| p.downcast_ref::<liveplot::panels::fft_ui::FftPanel>().is_none());
            }
        }

        // note: a handful of flags still don't modify the UI:
        // * `markers` – there is no public API to toggle every trace's
        //   `show_points` flag, so this checkbox is only illustrative.
        // * `export` – the export panel button is shown/hidden but we don't
        //   implement any export logic here.
        // Other flags (`scopes`, `pause_resume`, `clear_all`, `grid`, etc.)
        // now drive visible buttons or overlays as expected.
    }
}

impl eframe::App for FeaturesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // data is produced on a background thread; nothing to do here

        // draw checkboxes at the top
        egui::TopBottomPanel::top("features_top").show(ctx, |ui| {
            ui.label("Toggle features:");
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.features.top_bar, "top_bar");
                ui.checkbox(&mut self.features.sidebar, "sidebar");
                ui.checkbox(&mut self.features.markers, "markers");
                ui.checkbox(&mut self.features.thresholds, "thresholds");
                ui.checkbox(&mut self.features.triggers, "triggers");
                ui.checkbox(&mut self.features.measurement, "measurement");
                ui.checkbox(&mut self.features.export, "export");
                ui.checkbox(&mut self.features.math, "math");
                ui.checkbox(&mut self.features.hotkeys, "hotkeys");
                ui.checkbox(&mut self.features.fft, "fft");
                ui.checkbox(&mut self.features.x_tick_labels, "x_tick_labels");
                ui.checkbox(&mut self.features.y_tick_labels, "y_tick_labels");
                ui.checkbox(&mut self.features.grid, "grid");
                ui.checkbox(&mut self.features.legend, "legend");
                ui.checkbox(&mut self.features.scopes, "scopes");
                ui.checkbox(&mut self.features.pause_resume, "pause_resume");
                ui.checkbox(&mut self.features.clear_all, "clear_all");
            });
        });

        // Apply flags every frame (cost negligible)
        self.apply_features();

        // render the plot panel
        egui::CentralPanel::default().show(ctx, |ui| {
            self.plot.main_panel.update_embedded(ui);
        });

        // keep redrawing at roughly 60Hz
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    let app = FeaturesApp::new();
    eframe::run_native(
        "Feature Flags Example",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
