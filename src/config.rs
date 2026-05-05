//! Configuration types shared across the live plot UIs.

use crate::controllers::ThresholdController;
use crate::controllers::TracesController;
use crate::controllers::{FFTController, UiActionController, WindowController};
use crate::data::hotkeys::Hotkeys;
use crate::data::x_formatter::XFormatter;
use crate::events::EventController;

// ─────────────────────────────────────────────────────────────────────────────
// ScopeButton – identifies a UI button slot
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies a specific UI button that can be placed in the top bar or the right sidebar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ScopeButton {
    Scopes,
    Traces,
    Math,
    Hotkeys,
    Thresholds,
    Triggers,
    Measurement,
    Fft,
    Export,
    PauseResume,
    ClearAll,
    /// Any panel with a custom title string.
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
            ScopeButton::Scopes | ScopeButton::PauseResume | ScopeButton::ClearAll => false,
        }
    }

    /// The full default list of buttons (everything in the top bar).
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

// ─────────────────────────────────────────────────────────────────────────────
// Color scheme
// ─────────────────────────────────────────────────────────────────────────────

pub use crate::{ColorScheme, CustomColorScheme};

// ─────────────────────────────────────────────────────────────────────────────
// Feature flags
// ─────────────────────────────────────────────────────────────────────────────

/// Toggle individual UI features on or off.
///
/// All features default to `true` (enabled). Disable features to create a
/// minimal, focused UI for embedded dashboards.
#[derive(Clone, Debug)]
pub struct FeatureFlags {
    /// Show the top menu bar.
    pub top_bar: bool,
    /// Show the sidebar icon strip.
    pub sidebar: bool,
    /// Show trace point markers.
    pub markers: bool,
    /// Enable the thresholds panel.
    pub thresholds: bool,
    /// Enable the triggers panel.
    pub triggers: bool,
    /// Enable the measurement panel.
    pub measurement: bool,
    /// Enable the export panel.
    pub export: bool,
    /// Enable the math panel.
    pub math: bool,
    /// Enable the hotkeys panel.
    pub hotkeys: bool,
    /// Enable the FFT panel.
    pub fft: bool,
    /// Show X-axis tick labels.
    pub x_tick_labels: bool,
    /// Show Y-axis tick labels.
    pub y_tick_labels: bool,
    /// Show the plot grid.
    pub grid: bool,
    /// Show the plot legend.
    pub legend: bool,
    /// Show the scopes dropdown.
    pub scopes: bool,
    /// Show pause/resume button.
    pub pause_resume: bool,
    /// Show the clear-all button.
    pub clear_all: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            top_bar: true,
            sidebar: true,
            markers: true,
            thresholds: true,
            triggers: true,
            measurement: true,
            export: true,
            math: true,
            hotkeys: true,
            fft: true,
            x_tick_labels: true,
            y_tick_labels: true,
            grid: true,
            legend: true,
            scopes: true,
            pause_resume: true,
            clear_all: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Responsive layout thresholds
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum-size thresholds that control responsive hide/show of UI elements.
#[derive(Clone, Debug)]
pub struct ResponsiveLayout {
    /// Buttons to display in the top menu bar. `None` = all defaults.
    pub top_bar_buttons: Option<Vec<ScopeButton>>,
    /// Buttons to display as a persistent icon strip in the right sidebar.
    pub sidebar_buttons: Option<Vec<ScopeButton>>,
    /// Minimum plot width (px) required to show Y-axis tick labels. Default: `250.0`.
    pub min_width_for_y_ticklabels: f32,
    /// Minimum plot height (px) required to show X-axis tick labels. Default: `200.0`.
    pub min_height_for_x_ticklabels: f32,
    /// Minimum plot height (px) before the top bar hides. Default: `200.0`.
    pub min_height_for_top_bar: f32,
    /// Minimum plot width (px) before the sidebar hides. Default: `150.0`.
    pub min_width_for_sidebar: f32,
    /// Minimum plot height (px) before the sidebar hides. Default: `200.0`.
    pub min_height_for_sidebar: f32,
    /// Minimum plot width (px) required to show the legend.
    pub min_width_for_legend: f32,
    /// Minimum plot height (px) required to show the legend.
    pub min_height_for_legend: f32,
}

impl Default for ResponsiveLayout {
    fn default() -> Self {
        Self {
            top_bar_buttons: None,
            sidebar_buttons: None,
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

// ─────────────────────────────────────────────────────────────────────────────
// Auto-fit configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for automatic axis fitting behaviour.
#[derive(Clone, Debug)]
pub struct AutoFitConfig {
    /// When `true`, axes are automatically fitted to data each frame.
    /// Manual pan/zoom/drag disables auto-fit; clicking "Fit to View"
    /// re-enables it (if this field is `true`). Default: `true`.
    pub auto_fit_to_view: bool,
    /// When `true`, auto-fit only expands the view — it never shrinks.
    /// Keeps historical peaks visible. Default: `false`.
    pub keep_max_fit: bool,
}

impl Default for AutoFitConfig {
    fn default() -> Self {
        Self {
            auto_fit_to_view: true,
            keep_max_fit: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Controllers sub-config
// ─────────────────────────────────────────────────────────────────────────────

/// Optional programmatic controllers attached to the plot.
#[derive(Clone, Default)]
pub struct Controllers {
    pub window: Option<WindowController>,
    pub fft: Option<FFTController>,
    pub ui_action: Option<UiActionController>,
    pub threshold: Option<ThresholdController>,
    pub traces: Option<TracesController>,
    pub event: Option<EventController>,
}

// ─────────────────────────────────────────────────────────────────────────────
// LivePlotConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level configuration for the live plot.
///
/// Organised into sub-configs for clarity:
///
/// | Field            | Purpose |
/// |------------------|---------|
/// | `features`       | Toggle individual UI features on/off |
/// | `layout`         | Responsive hide/show thresholds |
/// | `color_scheme`   | Predefined visual theme |
/// | `auto_fit`       | Automatic axis fitting behaviour |
/// | `controllers`    | Programmatic interaction handles |
pub struct LivePlotConfig {
    // ── Scope / data ─────────────────────────────────────────────────────────
    /// Rolling time window in seconds.
    pub time_window_secs: f64,
    /// Maximum number of points retained per trace.
    pub max_points: usize,
    /// Optional unit label for the Y axis (e.g. "V", "°C").
    pub y_unit: Option<String>,
    /// Show Y axis in log10 scale.
    pub y_log: bool,

    // ── Window / chrome ──────────────────────────────────────────────────────
    /// Native window title.
    pub title: String,
    /// Optional headline rendered inside the UI.
    pub headline: Option<String>,
    /// Optional subheadline below the headline.
    pub subheadline: Option<String>,
    /// Optional eframe native-window options.
    pub native_options: Option<eframe::NativeOptions>,

    // ── Feature flags ────────────────────────────────────────────────────────
    /// Toggle individual UI features on/off.
    pub features: FeatureFlags,

    // ── Responsive layout ────────────────────────────────────────────────────
    /// Responsive hide/show thresholds for UI elements.
    pub layout: ResponsiveLayout,

    // ── Appearance ───────────────────────────────────────────────────────────
    /// Color scheme / visual theme.
    pub color_scheme: ColorScheme,
    /// Optional per-plot overlay callback.  The closure is invoked inside the
    /// plot rendering callback and can draw custom graphics using the
    /// [`egui_plot::PlotUi`] API.  Useful for example code that wants to add
    /// extra decorations (rainbow grid lines, annotations, etc.).
    pub overlays: Option<
        Box<
            dyn for<'a> FnMut(
                    &mut egui_plot::PlotUi,
                    &crate::data::scope::ScopeData,
                    &crate::data::traces::TracesCollection,
                ) + 'static,
        >,
    >,

    // ── Auto-fit ─────────────────────────────────────────────────────────────
    /// Automatic axis fitting configuration.
    pub auto_fit: AutoFitConfig,

    // ── Formatting ───────────────────────────────────────────────────────────
    /// X-axis formatter.
    pub x_formatter: XFormatter,

    // ── Hotkeys ──────────────────────────────────────────────────────────────
    /// Optional hotkeys configuration.
    pub hotkeys: Option<Hotkeys>,

    // ── Programmatic controllers ─────────────────────────────────────────────
    /// External controllers for programmatic interaction.
    pub controllers: Controllers,
}

impl Clone for LivePlotConfig {
    fn clone(&self) -> Self {
        Self {
            time_window_secs: self.time_window_secs,
            max_points: self.max_points,
            y_unit: self.y_unit.clone(),
            y_log: self.y_log,
            title: self.title.clone(),
            headline: self.headline.clone(),
            subheadline: self.subheadline.clone(),
            native_options: self.native_options.clone(),
            features: self.features.clone(),
            layout: self.layout.clone(),
            color_scheme: self.color_scheme.clone(),
            overlays: None, // cannot clone closure
            auto_fit: self.auto_fit.clone(),
            x_formatter: self.x_formatter.clone(),
            hotkeys: self.hotkeys.clone(),
            controllers: self.controllers.clone(),
        }
    }
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
            native_options: None,

            features: FeatureFlags::default(),
            layout: ResponsiveLayout::default(),
            color_scheme: ColorScheme::default(),
            overlays: None,
            auto_fit: AutoFitConfig::default(),

            x_formatter: XFormatter::Auto,
            hotkeys: None,
            controllers: Controllers::default(),
        }
    }
}
