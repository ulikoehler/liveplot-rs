//! Configuration types shared across the live plot UIs.

use crate::controllers::ThresholdController;
use crate::controllers::TracesController;
use crate::controllers::{FFTController, UiActionController, WindowController};
use crate::data::hotkeys::Hotkeys;
use crate::data::x_formatter::XFormatter;
use crate::events::EventController;

/// Identifies a specific UI button that can be placed in the top bar or the right sidebar.
///
/// Use these variants to build [`LivePlotConfig::top_bar_buttons`] and
/// [`LivePlotConfig::sidebar_buttons`] lists.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScopeButton {
    /// The Scopes dropdown (add / remove / rename scopes).
    Scopes,
    /// The Traces panel toggle.
    Traces,
    /// The Math panel toggle.
    Math,
    /// The Hotkeys panel toggle.
    Hotkeys,
    /// The Thresholds panel toggle.
    Thresholds,
    /// The Triggers panel toggle.
    Triggers,
    /// The Measurement panel toggle.
    Measurement,
    /// The FFT panel toggle (only has effect when the `fft` feature is enabled).
    Fft,
    /// The Export panel toggle.
    Export,
    /// The Pause / Resume button.
    PauseResume,
    /// The Clear All button.
    ClearAll,
    /// Any panel with a custom title string (for user-defined panels added to the sidebar).
    Custom(String),
}

impl ScopeButton {
    /// Returns `true` if this button should render the toggle for a panel whose `title()` matches.
    pub fn matches_panel_title(&self, title: &str) -> bool {
        match self {
            ScopeButton::Traces => title == "Traces",
            ScopeButton::Math => title == "Math",
            ScopeButton::Hotkeys => title == "Hotkeys",
            ScopeButton::Thresholds => title == "Thresholds",
            ScopeButton::Triggers => title == "Triggers",
            ScopeButton::Measurement => title == "Measurement",
            ScopeButton::Fft => title == "FFT",
            ScopeButton::Export => title == "Export",
            ScopeButton::Custom(t) => t.as_str() == title,
            // Non-panel buttons never match a panel title
            ScopeButton::Scopes | ScopeButton::PauseResume | ScopeButton::ClearAll => false,
        }
    }

    /// The full default list of buttons (current behaviour: everything in the top bar).
    pub fn all_defaults() -> Vec<ScopeButton> {
        vec![
            ScopeButton::Scopes,
            ScopeButton::Traces,
            ScopeButton::Math,
            ScopeButton::Hotkeys,
            ScopeButton::Thresholds,
            ScopeButton::Triggers,
            ScopeButton::Measurement,
            ScopeButton::Fft,
            ScopeButton::Export,
            ScopeButton::PauseResume,
            ScopeButton::ClearAll,
        ]
    }
}

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
    /// Optional event controller for subscribing to UI/data events.
    pub event_controller: Option<EventController>,
    /// Optional hotkeys configuration (if present overrides defaults)
    pub hotkeys: Option<Hotkeys>,
    /// Formatter used for the X axis. Defaults to [`XFormatter::Auto`], which
    /// automatically selects a decimal formatter for X/Y mode and the smart
    /// [`TimeFormatter`](crate::data::x_formatter::TimeFormatter) for time axes.
    pub x_formatter: XFormatter,

    // ── Responsive layout thresholds ──────────────────────────────────────────
    /// Buttons to display in the top menu bar.
    /// Pass `None` to use the default set (all buttons).
    /// Pass `Some(vec![])` to put no buttons in the top bar.
    pub top_bar_buttons: Option<Vec<ScopeButton>>,

    /// Buttons to display as a persistent icon strip in the right sidebar.
    /// Defaults to `None` (empty – no extra icon strip beyond the standard collapsed state).
    pub sidebar_buttons: Option<Vec<ScopeButton>>,

    /// Minimum plot widget **width** (logical pixels) required to show Y-axis tick labels.
    /// When the plot is narrower than this, all Y tick labels are hidden.
    /// Default: `250.0`.
    pub min_width_for_y_ticklabels: f32,

    /// Minimum plot widget **height** (logical pixels) required to show X-axis tick labels.
    /// When the plot is shorter than this, all X tick labels are hidden.
    /// Default: `200.0`.
    pub min_height_for_x_ticklabels: f32,

    /// Minimum plot widget **height** (logical pixels) before the top bar is hidden and its
    /// buttons are moved to the sidebar.  Default: `200.0`.
    pub min_height_for_top_bar: f32,

    /// Minimum plot widget **width** (logical pixels) before the sidebar is hidden and its
    /// icon-strip buttons are moved to the top bar.  Default: `150.0`.
    pub min_width_for_sidebar: f32,

    /// Minimum plot widget **height** (logical pixels) before the sidebar is hidden and its
    /// icon-strip buttons are moved to the top bar.  Default: `200.0`
    /// (same as [`min_height_for_x_ticklabels`][Self::min_height_for_x_ticklabels]).
    pub min_height_for_sidebar: f32,

    /// Minimum plot widget **width** (logical pixels) required to show the legend.
    /// When the widget is narrower than this, the legend is hidden.  Default: `0.0` (disabled).
    pub min_width_for_legend: f32,

    /// Minimum plot widget **height** (logical pixels) required to show the legend.
    /// When the widget is shorter than this, the legend is hidden.  Default: `0.0` (disabled).
    pub min_height_for_legend: f32,
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
            event_controller: None,
            hotkeys: None,
            x_formatter: XFormatter::Auto,

            top_bar_buttons: None, // None = all buttons (default behaviour)
            sidebar_buttons: None, // None = no extra icon strip

            min_width_for_y_ticklabels: 250.0,
            min_height_for_x_ticklabels: 200.0,
            min_height_for_top_bar: 200.0,
            min_width_for_sidebar: 150.0,
            min_height_for_sidebar: 200.0,
            min_width_for_legend: 0.0,
            min_height_for_legend: 0.0,
        }
    }
}
