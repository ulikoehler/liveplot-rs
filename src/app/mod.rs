//! Main application module for LivePlot.
//!
//! This module defines the core types and wiring for the LivePlot GUI.
//! It is split into focused sub-modules so that each concern can be
//! reasoned about independently:
//!
//! | Sub-module                 | Responsibility |
//! | -------------------------- | -------------- |
//! | [`update`]                 | Per-frame data ingestion, panel refresh, and central-panel rendering |
//! | [`panel_helpers`]          | Utilities for locating and toggling specific panel types |
//! | [`controllers_embedded`]   | Processing controller requests when embedded in a parent app |
//! | [`layout`]                 | Responsive layout decisions, menu bar, sidebars, and tab rendering |
//! | [`liveplot_app`]         | Standalone [`LivePlotApp`] (eframe) wrapper and its controller wiring |
//! | [`run`]                    | Top-level [`run_liveplot()`] entry point and icon loading |

// Historically the implementation lived in a single `app.rs`; it was split
// into sub-modules for clarity.  The individual modules still provide the
// relevant types and functions, so we must declare them here.
mod controllers_embedded;
mod layout;
mod liveplot_app;
mod panel_helpers;
mod run;
mod update;

// ── Public re-exports consumed by lib.rs ─────────────────────────────────────
// `LivePlotApp` and `run_liveplot` are defined in sub-modules but are part of the
// public API of `app`, so re-export them at the top level.
pub use liveplot_app::LivePlotApp;
pub use run::run_liveplot;

// ── Crate-internal shared imports ────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui;

use crate::config::ScopeButton;
use crate::controllers::{
    FFTController, LiveplotController, ScopesController, ThresholdController, TracesController,
    UiActionController, WindowController,
};
use crate::data::data::LivePlotRequests;
use crate::data::hotkeys::Hotkeys;
use crate::data::traces::TracesCollection;
use crate::events::EventController;
use crate::panels::liveplot_ui::LiveplotPanel;
use crate::panels::panel_trait::Panel;
use crate::PlotCommand;

#[cfg(feature = "fft")]
use crate::panels::fft_ui::FftPanel;
use crate::panels::{
    export_ui::ExportPanel, hotkeys_ui::HotkeysPanel, math_ui::MathPanel,
    measurment_ui::MeasurementPanel, thresholds_ui::ThresholdsPanel, traces_ui::TracesPanel,
    triggers_ui::TriggersPanel,
};

/// Global monotonic counter that assigns unique IDs to [`LivePlotPanel`] instances.
///
/// Each `LivePlotPanel` gets a unique `panel_id` to namespace its egui widget IDs,
/// which prevents collisions when multiple panels coexist (e.g. in a tiled layout).
static PANEL_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Computed layout describing which buttons appear where for a single frame.
///
/// [`LivePlotPanel::compute_effective_layout`] recalculates this every frame based
/// on the available viewport dimensions and the user's button configuration.
/// It drives responsive behaviour: buttons migrate between the top menu-bar
/// and the sidebar icon-strip depending on the plot-area size.
pub(crate) struct EffectiveLayout {
    /// Buttons to render in the top menu bar (empty ⟹ top bar is not shown).
    pub top_bar_buttons: Vec<ScopeButton>,
    /// Buttons to render in the sidebar icon strip (empty ⟹ no icon strip).
    pub sidebar_buttons: Vec<ScopeButton>,
    /// Whether the top menu bar is visible.
    pub show_top_bar: bool,
    /// Whether sidebar panel content (attached panel widgets) is visible.
    pub show_sidebar_panels: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// LivePlotPanel – the central widget type
// ─────────────────────────────────────────────────────────────────────────────

/// The central widget that owns trace data, panels, and the live-plot scope(s).
///
/// `LivePlotPanel` is the building block of the LivePlot UI.  It can be used:
///
/// * **Standalone** – wrapped inside [`LivePlotApp`] and driven by the eframe event loop.
/// * **Embedded** – placed inside a parent egui application via [`LivePlotPanel::update`] or
///   [`LivePlotPanel::update_embedded`].
///
/// # Fields
///
/// The struct holds:
///
/// * All trace and scope data ([`traces_data`](Self::traces_data),
///   [`liveplot_panel`](Self::liveplot_panel)).
/// * A set of configurable sub-panels (traces list, math, thresholds, …).
/// * Optional *controllers* that allow programmatic interaction from external code
///   (e.g. pause, export, change colours).
/// * Responsive-layout parameters that control when the top-bar or sidebar collapse.
pub struct LivePlotPanel {
    // ── Data ─────────────────────────────────────────────────────────────────
    /// Collection of all traces (time-series data) received through the command channel.
    pub traces_data: TracesCollection,

    /// Shared hotkey bindings used by all panels and menu buttons.
    pub hotkeys: Rc<RefCell<Hotkeys>>,

    // ── Panels ───────────────────────────────────────────────────────────────
    /// The primary live-plot panel that renders scope(s) with traces.
    pub liveplot_panel: LiveplotPanel,

    /// Panels docked to the right side of the plot area.
    pub right_side_panels: Vec<Box<dyn Panel>>,

    /// Panels docked to the left side of the plot area.
    pub left_side_panels: Vec<Box<dyn Panel>>,

    /// Panels docked to the bottom of the plot area (e.g. FFT).
    pub bottom_panels: Vec<Box<dyn Panel>>,

    /// Panels shown in detached (floating) windows.
    pub detached_panels: Vec<Box<dyn Panel>>,

    /// Panels that exist but are not rendered in any dock position (e.g. export dialog).
    pub empty_panels: Vec<Box<dyn Panel>>,

    // ── Controllers (for embedded / programmatic use) ────────────────────────
    /// Controls the host window (size, position).
    pub(crate) window_ctrl: Option<WindowController>,

    /// Programmatic UI actions (pause, screenshot, export).
    pub(crate) ui_ctrl: Option<UiActionController>,

    /// Programmatic trace manipulation (colour, visibility, offset, etc.).
    pub(crate) traces_ctrl: Option<TracesController>,

    /// Programmatic scope management (add/remove/configure scopes).
    pub(crate) scopes_ctrl: Option<ScopesController>,

    /// High-level liveplot control (pause all, clear all, save/load state).
    pub(crate) liveplot_ctrl: Option<LiveplotController>,

    /// FFT panel control (show/hide, resize).
    pub(crate) fft_ctrl: Option<FFTController>,

    /// Threshold management (add/remove thresholds, listen for threshold events).
    pub(crate) threshold_ctrl: Option<ThresholdController>,

    /// Event controller for dispatching UI/data events to subscribers.
    pub(crate) event_ctrl: Option<EventController>,

    /// Per-threshold event cursor: tracks how many events we have already forwarded
    /// to controller listeners so that only *new* events are published.
    pub(crate) threshold_event_cursors: HashMap<String, usize>,

    /// Pending requests (save/load state, add/remove scope) accumulated during one frame.
    pub pending_requests: LivePlotRequests,

    // ── Responsive button-layout configuration ───────────────────────────────
    /// Buttons placed in the top menu bar.  `None` = the full default set.
    pub top_bar_buttons: Option<Vec<ScopeButton>>,

    /// Buttons placed in the right sidebar icon strip.  `None` = empty (standard behaviour).
    pub sidebar_buttons: Option<Vec<ScopeButton>>,

    /// Minimum plot-area height (px) before the top bar is hidden and its buttons move to sidebar.
    pub min_height_for_top_bar: f32,

    /// Minimum plot-area width (px) before the sidebar is hidden and its buttons move to top bar.
    pub min_width_for_sidebar: f32,

    /// Minimum plot-area height (px) before the sidebar is hidden and its buttons move to top bar.
    pub min_height_for_sidebar: f32,

    /// Central-panel size captured at the end of the previous frame (for responsive decisions).
    pub(crate) last_plot_size: egui::Vec2,

    /// Unique ID for this panel instance, used to namespace egui panel IDs.
    pub(crate) panel_id: u64,

    /// When `true`, the inner CentralPanel is rendered with no frame/margin so the plot
    /// fills every pixel of the allocated space.  Useful for dense embedded grid layouts.
    pub compact: bool,
}

impl LivePlotPanel {
    /// Create a new `LivePlotPanel` that will receive [`PlotCommand`]s from the given channel.
    ///
    /// The panel is pre-populated with the default set of sub-panels:
    ///
    /// * **Right:** Traces, Math, Hotkeys, Thresholds, Triggers, Measurement
    /// * **Bottom:** FFT (when the `fft` feature is enabled)
    /// * **Hidden:** Export
    pub fn new(rx: std::sync::mpsc::Receiver<PlotCommand>) -> Self {
        let hotkeys = Rc::new(RefCell::new(Hotkeys::default()));
        Self {
            traces_data: TracesCollection::new(rx),
            hotkeys: hotkeys.clone(),
            liveplot_panel: LiveplotPanel::default(),
            right_side_panels: vec![
                Box::new(TracesPanel::default()),
                Box::new(MathPanel::default()),
                Box::new(HotkeysPanel::new(hotkeys.clone())),
                Box::new(ThresholdsPanel::default()),
                Box::new(TriggersPanel::default()),
                Box::new(MeasurementPanel::default()),
            ],
            left_side_panels: vec![],
            #[cfg(feature = "fft")]
            bottom_panels: vec![Box::new(FftPanel::default())],
            #[cfg(not(feature = "fft"))]
            bottom_panels: vec![],
            detached_panels: vec![],
            empty_panels: vec![Box::new(ExportPanel::default())],
            window_ctrl: None,
            ui_ctrl: None,
            traces_ctrl: None,
            scopes_ctrl: None,
            liveplot_ctrl: None,
            fft_ctrl: None,
            threshold_ctrl: None,
            event_ctrl: None,
            threshold_event_cursors: HashMap::new(),
            pending_requests: LivePlotRequests::default(),
            top_bar_buttons: None,
            sidebar_buttons: None,
            min_height_for_top_bar: 200.0,
            min_width_for_sidebar: 150.0,
            min_height_for_sidebar: 200.0,
            // Initialise to a large number so that no suppression happens on the first frame.
            last_plot_size: egui::Vec2::new(10_000.0, 10_000.0),
            panel_id: PANEL_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            compact: false,
        }
    }

    /// Attach controllers for embedded usage.
    ///
    /// These mirror the controllers used by [`LivePlotApp`]; call this once after
    /// construction to enable programmatic interaction from external code.
    pub fn set_controllers(
        &mut self,
        window_ctrl: Option<WindowController>,
        ui_ctrl: Option<UiActionController>,
        traces_ctrl: Option<TracesController>,
        scopes_ctrl: Option<ScopesController>,
        liveplot_ctrl: Option<LiveplotController>,
        fft_ctrl: Option<FFTController>,
        threshold_ctrl: Option<ThresholdController>,
    ) {
        self.window_ctrl = window_ctrl;
        self.ui_ctrl = ui_ctrl;
        self.traces_ctrl = traces_ctrl;
        self.scopes_ctrl = scopes_ctrl;
        self.liveplot_ctrl = liveplot_ctrl;
        self.fft_ctrl = fft_ctrl;
        self.threshold_ctrl = threshold_ctrl;
    }

    /// Attach an event controller for event dispatch.
    pub fn set_event_controller(&mut self, event_ctrl: Option<EventController>) {
        self.event_ctrl = event_ctrl;
    }
}
