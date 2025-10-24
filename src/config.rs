//! Configuration types shared across the live plot UIs.

use crate::controllers::TracesController;
use crate::controllers::{FFTController, UiActionController, WindowController};
use crate::hotkeys::Hotkeys;
use crate::thresholds::ThresholdController;
use chrono::Local;

/// Formatting options for the x-value (time) shown in point labels.
#[derive(Debug, Clone, Copy)]
pub enum XDateFormat {
    /// Local time with date, ISO8601-like: YYYY-MM-DD HH:MM:SS
    Iso8601WithDate,
    /// Local time, time-of-day only: HH:MM:SS
    Iso8601Time,
}

impl Default for XDateFormat {
    fn default() -> Self {
        XDateFormat::Iso8601Time
    }
}

impl XDateFormat {
    /// Format an `x` value (seconds since UNIX epoch as f64) according to the selected format.
    pub fn format_value(&self, x_seconds: f64) -> String {
        let secs = x_seconds as i64;
        let nsecs = ((x_seconds - secs as f64) * 1e9) as u32;
        let dt_utc = chrono::DateTime::from_timestamp(secs, nsecs)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        match self {
            XDateFormat::Iso8601WithDate => dt_utc
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            XDateFormat::Iso8601Time => dt_utc.with_timezone(&Local).format("%H:%M:%S").to_string(),
        }
    }
}

/// Configuration options for the live plot runtime (single- and multi-trace).
#[derive(Clone)]
pub struct LivePlotConfig {
    /// Rolling time window in seconds that is kept in memory and shown on X axis.
    pub time_window_secs: f64,
    /// Maximum number of points retained per trace (cap to limit memory/CPU).
    pub max_points: usize,
    /// Format used for x-values in point labels.
    pub x_date_format: XDateFormat,
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
            x_date_format: XDateFormat::default(),
            y_unit: None,
            y_log: false,
            title: "LivePlot".to_string(),
            headline: None,
            subheadline: None,
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
