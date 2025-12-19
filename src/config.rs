//! Configuration types shared across the live plot UIs.

use crate::controllers::ThresholdController;
use crate::controllers::TracesController;
use crate::controllers::{FFTController, UiActionController, WindowController};
use crate::data::hotkeys::Hotkeys;

/// Configuration options for the live plot runtime (single- and multi-trace).
#[derive(Clone)]
pub struct LivePlotConfig {
    /// Rolling time window in seconds that is kept in memory and shown on X axis.
    pub time_window_secs: f64,
    /// Maximum number of points retained per trace (cap to limit memory/CPU).
    pub max_points: usize,
    /// Optional unit label for the Y axis (e.g., "V", "A", "°C"). If set, it is appended
    /// to Y tick labels and to point readouts/overlays.
    pub y_unit: Option<String>,
    /// Show the Y axis using a base-10 logarithmic scale. The transform is applied
    /// to the plotted values; axis tick labels show the corresponding linear values.
    pub y_log: bool,
    /// Optional window title (default: None)
    /// Window title shown on the native window chrome. This is always present and
    /// defaults to "LivePlot".
    pub title: String,
    /// Optional headline rendered inside the UI (e.g. large heading). If None,
    /// no headline is shown.
    pub headline: Option<String>,
    /// Optional subheadline rendered underneath the main headline (smaller).
    /// If None, no subheadline is shown.
    pub subheadline: Option<String>,
    /// Whether to show the legend within the UI. Defaults to true.
    pub show_legend: bool,
    /// Optional eframe/native window options. If not provided, sensible defaults are used.
    pub native_options: Option<eframe::NativeOptions>,
    /// Optional controllers to attach.
    pub window_controller: Option<WindowController>,
    pub fft_controller: Option<FFTController>,
    pub ui_action_controller: Option<UiActionController>,
    pub threshold_controller: Option<ThresholdController>,
    pub traces_controller: Option<TracesController>,
    /// Optional hotkeys configuration (if present overrides defaults)
    pub hotkeys: Option<Hotkeys>,
}

impl Default for LivePlotConfig {
    fn default() -> Self {
        Self {
            time_window_secs: 10.0,
            max_points: 10_000,

            y_unit: None,
            y_log: false,
            title: "LivePlot".to_string(),
            headline: None,
            subheadline: None,
            show_legend: true,
            native_options: None,
            window_controller: None,
            fft_controller: None,
            ui_action_controller: None,
            threshold_controller: None,
            traces_controller: None,
            hotkeys: None,
        }
    }
}
