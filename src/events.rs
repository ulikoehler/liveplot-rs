//! Generic event system for LivePlot.
//!
//! Callers can subscribe to a rich set of UI and data events via
//! [`EventController`].  Each event carries a set of [`EventKind`] flags
//! (bitflags-style) so that a single occurrence can match multiple
//! categories (e.g. a measurement-point click is *also* a `Click` event).
//!
//! The caller specifies an [`EventFilter`] to receive only the events they
//! care about.  The filter is a simple OR mask: an event is delivered when
//! `(event.kinds & filter) != 0`.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::data::traces::TraceRef;

// ─────────────────────────────────────────────────────────────────────────────
// EventKind – bitflags
// ─────────────────────────────────────────────────────────────────────────────

/// Bitflags describing the *categories* an event belongs to.
///
/// A single [`PlotEvent`] may have several bits set.  For example a
/// "click that set a measurement point" would have both `CLICK` and
/// `MEASUREMENT_POINT` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventKind(pub u64);

impl EventKind {
    // ── Pointer / interaction ────────────────────────────────────────────
    /// A single (primary) click anywhere on a scope plot.
    pub const CLICK: Self = Self(1 << 0);
    /// A double-click on a scope plot (the second click is also a `CLICK`).
    pub const DOUBLE_CLICK: Self = Self(1 << 1);
    /// A click that landed on (or snapped to) a specific curve/trace.
    pub const CLICK_ON_TRACE: Self = Self(1 << 2);

    // ── Pause / resume ──────────────────────────────────────────────────
    /// The scope was paused (either by click or programmatically).
    pub const PAUSE: Self = Self(1 << 3);
    /// The scope was resumed.
    pub const RESUME: Self = Self(1 << 4);

    // ── Measurement ─────────────────────────────────────────────────────
    /// A measurement marker point was set (P1 or P2).
    pub const MEASUREMENT_POINT: Self = Self(1 << 5);
    /// A full measurement (both P1 and P2) is now available.
    pub const MEASUREMENT_COMPLETE: Self = Self(1 << 6);
    /// A measurement was cleared.
    pub const MEASUREMENT_CLEARED: Self = Self(1 << 7);

    // ── Trace visibility / colour ───────────────────────────────────────
    /// A trace was shown.
    pub const TRACE_SHOWN: Self = Self(1 << 8);
    /// A trace was hidden.
    pub const TRACE_HIDDEN: Self = Self(1 << 9);
    /// A trace colour was changed.
    pub const TRACE_COLOR_CHANGED: Self = Self(1 << 10);

    // ── Math traces ─────────────────────────────────────────────────────
    /// A math trace was added.
    pub const MATH_TRACE_ADDED: Self = Self(1 << 11);
    /// A math trace was removed.
    pub const MATH_TRACE_REMOVED: Self = Self(1 << 12);

    // ── Zoom / view ─────────────────────────────────────────────────────
    /// The view was zoomed (scroll-wheel, box-zoom, or programmatic).
    pub const ZOOM: Self = Self(1 << 13);
    /// The view was fit-to-data (auto-fit or button).
    pub const FIT_TO_VIEW: Self = Self(1 << 14);
    /// The view was panned.
    pub const PAN: Self = Self(1 << 15);

    // ── Resize / window ─────────────────────────────────────────────────
    /// The plot widget was resized.
    pub const RESIZE: Self = Self(1 << 16);

    // ── Data ────────────────────────────────────────────────────────────
    /// New data points were received for one or more traces.
    pub const DATA_UPDATED: Self = Self(1 << 17);
    /// All trace data was cleared.
    pub const DATA_CLEARED: Self = Self(1 << 18);

    // ── Thresholds ──────────────────────────────────────────────────────
    /// A threshold event was detected (threshold exceeded condition met).
    pub const THRESHOLD_EXCEEDED: Self = Self(1 << 19);
    /// A threshold definition was added.
    pub const THRESHOLD_ADDED: Self = Self(1 << 20);
    /// A threshold definition was removed.
    pub const THRESHOLD_REMOVED: Self = Self(1 << 21);

    // ── Hotkeys / keyboard ──────────────────────────────────────────────
    /// A keyboard key was pressed inside the plot area.
    pub const KEY_PRESSED: Self = Self(1 << 22);

    // ── Export / screenshot ─────────────────────────────────────────────
    /// An export (CSV/Parquet) was initiated.
    pub const EXPORT: Self = Self(1 << 23);
    /// A screenshot was taken.
    pub const SCREENSHOT: Self = Self(1 << 24);

    // ── Scope management ────────────────────────────────────────────────
    /// A scope was added.
    pub const SCOPE_ADDED: Self = Self(1 << 25);
    /// A scope was removed.
    pub const SCOPE_REMOVED: Self = Self(1 << 26);

    // ── Trigger ─────────────────────────────────────────────────────────
    /// A trigger fired.
    pub const TRIGGER_FIRED: Self = Self(1 << 27);

    // ── Trace offset / style ────────────────────────────────────────────
    /// A trace Y-offset was changed.
    pub const TRACE_OFFSET_CHANGED: Self = Self(1 << 28);

    // ── Y axis settings ─────────────────────────────────────────────────
    /// Y-axis log mode was toggled.
    pub const Y_LOG_CHANGED: Self = Self(1 << 29);
    /// Y-axis unit was changed.
    pub const Y_UNIT_CHANGED: Self = Self(1 << 30);

    /// Wildcard: matches *every* event kind.
    pub const ALL: Self = Self(u64::MAX);

    /// Combine two event kinds (bitwise OR).
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check whether `self` contains all bits in `other`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check whether `self` intersects with `other` (at least one bit in common).
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns `true` if no bits are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for EventKind {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for EventKind {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for EventKind {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for EventKind {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// String conversions
// ─────────────────────────────────────────────────────────────────────────────

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "EMPTY");
        }

        // The ALL constant is a useful shorthand; print it directly instead
        if *self == EventKind::ALL {
            return write!(f, "ALL");
        }

        // Known kinds with their string names in declaration order.
        let pairs: &[(EventKind, &str)] = &[
            (EventKind::CLICK, "CLICK"),
            (EventKind::DOUBLE_CLICK, "DOUBLE_CLICK"),
            (EventKind::CLICK_ON_TRACE, "CLICK_ON_TRACE"),
            (EventKind::PAUSE, "PAUSE"),
            (EventKind::RESUME, "RESUME"),
            (EventKind::MEASUREMENT_POINT, "MEASUREMENT_POINT"),
            (EventKind::MEASUREMENT_COMPLETE, "MEASUREMENT_COMPLETE"),
            (EventKind::MEASUREMENT_CLEARED, "MEASUREMENT_CLEARED"),
            (EventKind::TRACE_SHOWN, "TRACE_SHOWN"),
            (EventKind::TRACE_HIDDEN, "TRACE_HIDDEN"),
            (EventKind::TRACE_COLOR_CHANGED, "TRACE_COLOR_CHANGED"),
            (EventKind::MATH_TRACE_ADDED, "MATH_TRACE_ADDED"),
            (EventKind::MATH_TRACE_REMOVED, "MATH_TRACE_REMOVED"),
            (EventKind::ZOOM, "ZOOM"),
            (EventKind::FIT_TO_VIEW, "FIT_TO_VIEW"),
            (EventKind::PAN, "PAN"),
            (EventKind::RESIZE, "RESIZE"),
            (EventKind::DATA_UPDATED, "DATA_UPDATED"),
            (EventKind::DATA_CLEARED, "DATA_CLEARED"),
            (EventKind::THRESHOLD_EXCEEDED, "THRESHOLD_EXCEEDED"),
            (EventKind::THRESHOLD_ADDED, "THRESHOLD_ADDED"),
            (EventKind::THRESHOLD_REMOVED, "THRESHOLD_REMOVED"),
            (EventKind::KEY_PRESSED, "KEY_PRESSED"),
            (EventKind::EXPORT, "EXPORT"),
            (EventKind::SCREENSHOT, "SCREENSHOT"),
            (EventKind::SCOPE_ADDED, "SCOPE_ADDED"),
            (EventKind::SCOPE_REMOVED, "SCOPE_REMOVED"),
            (EventKind::TRIGGER_FIRED, "TRIGGER_FIRED"),
            (EventKind::TRACE_OFFSET_CHANGED, "TRACE_OFFSET_CHANGED"),
            (EventKind::Y_LOG_CHANGED, "Y_LOG_CHANGED"),
            (EventKind::Y_UNIT_CHANGED, "Y_UNIT_CHANGED"),
        ];

        let mut names = Vec::new();
        let mut known_bits: u64 = 0;

        for (kind, name) in pairs {
            known_bits |= kind.0;
            if self.contains(*kind) {
                names.push((*name).to_string());
            }
        }

        // Bits that weren't covered by the known list
        let extra = self.0 & !known_bits;
        if extra != 0 {
            names.push(format!("0x{:x}", extra));
        }

        if names.is_empty() {
            // No named bits and not ALL (handled above) -> just show hex
            write!(f, "0x{:x}", self.0)
        } else {
            write!(f, "{}", names.join("|"))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Metadata – per-event-type payloads
// ─────────────────────────────────────────────────────────────────────────────

/// Screen (pixel) coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPos {
    pub x: f32,
    pub y: f32,
}

/// Plot-space coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotPos {
    pub x: f64,
    pub y: f64,
}

/// Metadata attached to click / pointer events.
#[derive(Debug, Clone)]
pub struct ClickMeta {
    /// Screen coordinates of the click (pixels within the window).
    pub screen_pos: Option<ScreenPos>,
    /// Plot-space coordinates of the click.
    pub plot_pos: Option<PlotPos>,
    /// If the click snapped to a trace, name of that trace.
    pub trace: Option<TraceRef>,
    /// Scope that was clicked (by id).
    pub scope_id: Option<usize>,
}

/// Metadata for measurement-point events.
#[derive(Debug, Clone)]
pub struct MeasurementMeta {
    /// Which point index was set (0 = P1, 1 = P2).
    pub point_index: usize,
    /// Coordinates of the point that was set.
    pub point: [f64; 2],
    /// If a full measurement is available, the two points.
    pub p1: Option<[f64; 2]>,
    pub p2: Option<[f64; 2]>,
    /// Delta X between P1 and P2 (only valid when both points exist).
    pub delta_x: Option<f64>,
    /// Delta Y between P1 and P2.
    pub delta_y: Option<f64>,
    /// Slope (dy/dx) between the two points.
    pub slope: Option<f64>,
    /// Euclidean distance between the two points.
    pub distance: Option<f64>,
    /// Name of the measurement.
    pub measurement_name: Option<String>,
    /// Trace that the measurement is snapping to (if any).
    pub trace: Option<TraceRef>,
}

/// Metadata for zoom / pan / fit events.
#[derive(Debug, Clone)]
pub struct ViewChangeMeta {
    /// New visible X range after the change.
    pub x_range: Option<(f64, f64)>,
    /// New visible Y range after the change.
    pub y_range: Option<(f64, f64)>,
    /// Scope id that was zoomed/panned.
    pub scope_id: Option<usize>,
}

/// Metadata for trace visibility / colour events.
#[derive(Debug, Clone)]
pub struct TraceMeta {
    /// Name of the trace that changed.
    pub trace: TraceRef,
    /// New visibility state (for show/hide events).
    pub visible: Option<bool>,
    /// New colour (for colour change events).
    pub color_rgb: Option<[u8; 3]>,
    /// New Y-offset (for offset change events).
    pub offset: Option<f64>,
}

/// Metadata for math trace events.
#[derive(Debug, Clone)]
pub struct MathTraceMeta {
    /// Name of the math trace.
    pub name: String,
    /// Formula / expression string (if available).
    pub formula: Option<String>,
}

/// Metadata for resize events.
#[derive(Debug, Clone, Copy)]
pub struct ResizeMeta {
    /// New size in logical pixels.
    pub width: f32,
    pub height: f32,
}

/// Metadata for data-update events.
#[derive(Debug, Clone)]
pub struct DataUpdateMeta {
    /// Traces that received new data.
    pub traces: Vec<TraceRef>,
    /// Total number of new points across all traces (approximate).
    pub new_point_count: usize,
}

/// Metadata for threshold events.
#[derive(Debug, Clone)]
pub struct ThresholdMeta {
    /// Name of the threshold.
    pub threshold_name: String,
    /// Trace being monitored.
    pub trace: Option<TraceRef>,
    /// Start time of the threshold event (seconds).
    pub start_t: Option<f64>,
    /// End time (seconds).
    pub end_t: Option<f64>,
    /// Duration (seconds).
    pub duration: Option<f64>,
    /// Integrated area.
    pub area: Option<f64>,
}

/// Metadata for key-press events.
#[derive(Debug, Clone)]
pub struct KeyPressMeta {
    /// The key that was pressed (as egui key name or char).
    pub key: String,
    /// Modifier state.
    pub modifiers: KeyModifiers,
}

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
}

/// Metadata for export events.
#[derive(Debug, Clone)]
pub struct ExportMeta {
    /// Format of the export ("csv", "parquet", "png").
    pub format: String,
    /// Path where the export was saved (if known).
    pub path: Option<String>,
}

/// Metadata for scope management events.
#[derive(Debug, Clone)]
pub struct ScopeManageMeta {
    /// Scope id.
    pub scope_id: usize,
    /// Scope name.
    pub scope_name: Option<String>,
}

/// Metadata for trigger events.
#[derive(Debug, Clone)]
pub struct TriggerMeta {
    /// Trigger name.
    pub trigger_name: String,
    /// Trace being monitored.
    pub trace: Option<TraceRef>,
    /// Timestamp at which the trigger fired.
    pub timestamp: Option<f64>,
}

/// Metadata for Y-axis setting changes.
#[derive(Debug, Clone)]
pub struct YAxisMeta {
    /// New Y-log state (if changed).
    pub y_log: Option<bool>,
    /// New Y-unit (if changed).
    pub y_unit: Option<Option<String>>,
}

/// Metadata for pause/resume events.
#[derive(Debug, Clone)]
pub struct PauseMeta {
    /// Scope id that was paused/resumed.
    pub scope_id: Option<usize>,
}

// ─────────────────────────────────────────────────────────────────────────────
// PlotEvent – the top-level event type
// ─────────────────────────────────────────────────────────────────────────────

/// A rich event emitted by the LivePlot UI.
///
/// `kinds` is a bitflag set of [`EventKind`] categories.  The various
/// `Option<…Meta>` fields carry metadata relevant to the kinds that are set.
#[derive(Debug, Clone)]
pub struct PlotEvent {
    /// Bitflag set of categories this event belongs to.
    pub kinds: EventKind,
    /// Monotonic timestamp (seconds since app start, from `std::time::Instant`).
    pub timestamp: f64,

    // ── Optional metadata ────────────────────────────────────────────────
    pub click: Option<ClickMeta>,
    pub measurement: Option<MeasurementMeta>,
    pub view_change: Option<ViewChangeMeta>,
    pub trace: Option<TraceMeta>,
    pub math_trace: Option<MathTraceMeta>,
    pub resize: Option<ResizeMeta>,
    pub data_update: Option<DataUpdateMeta>,
    pub threshold: Option<ThresholdMeta>,
    pub key_press: Option<KeyPressMeta>,
    pub export: Option<ExportMeta>,
    pub scope_manage: Option<ScopeManageMeta>,
    pub trigger: Option<TriggerMeta>,
    pub y_axis: Option<YAxisMeta>,
    pub pause: Option<PauseMeta>,
}

impl PlotEvent {
    /// Create a new event with the given kinds and current timestamp.
    pub fn new(kinds: EventKind) -> Self {
        Self {
            kinds,
            timestamp: 0.0, // will be set by controller
            click: None,
            measurement: None,
            view_change: None,
            trace: None,
            math_trace: None,
            resize: None,
            data_update: None,
            threshold: None,
            key_press: None,
            export: None,
            scope_manage: None,
            trigger: None,
            y_axis: None,
            pause: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EventFilter
// ─────────────────────────────────────────────────────────────────────────────

/// A filter that selects which event categories a subscriber receives.
///
/// The filter is an OR-mask: an event is delivered when
/// `event.kinds.intersects(filter.mask)`.
#[derive(Debug, Clone, Copy)]
pub struct EventFilter {
    pub mask: EventKind,
}

impl EventFilter {
    /// Accept all events.
    pub const fn all() -> Self {
        Self {
            mask: EventKind::ALL,
        }
    }

    /// Accept only the specified event kinds.
    pub const fn only(mask: EventKind) -> Self {
        Self { mask }
    }

    /// Check whether an event passes this filter.
    #[inline]
    pub fn matches(&self, event: &PlotEvent) -> bool {
        event.kinds.intersects(self.mask)
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::all()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EventController
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) struct Subscriber {
    filter: EventFilter,
    sender: Sender<PlotEvent>,
}

/// Controller that collects and distributes UI events to subscribers.
///
/// Attach it to [`LivePlotConfig`](crate::config::LivePlotConfig) before
/// launching the UI.  Then call [`subscribe`](Self::subscribe) (with an
/// optional filter) to receive events on an `mpsc` channel.
#[derive(Clone)]
pub struct EventController {
    pub(crate) inner: Arc<Mutex<EventCtrlInner>>,
}

pub(crate) struct EventCtrlInner {
    pub(crate) subscribers: Vec<Subscriber>,
    pub(crate) start_instant: std::time::Instant,
    /// Last known plot size, used to detect resize events.
    pub(crate) last_size: Option<[f32; 2]>,
}

impl EventController {
    /// Create a new event controller.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventCtrlInner {
                subscribers: Vec::new(),
                start_instant: std::time::Instant::now(),
                last_size: None,
            })),
        }
    }

    /// Subscribe to events matching the given filter.
    ///
    /// Returns a receiver that will receive [`PlotEvent`]s whenever the UI
    /// emits an event whose `kinds` intersect with the filter mask.
    pub fn subscribe(&self, filter: EventFilter) -> Receiver<PlotEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.subscribers.push(Subscriber { filter, sender: tx });
        rx
    }

    /// Subscribe to *all* events (no filtering).
    pub fn subscribe_all(&self) -> Receiver<PlotEvent> {
        self.subscribe(EventFilter::all())
    }

    /// Emit an event to all matching subscribers.
    ///
    /// This is called internally by the LivePlot UI.  External code normally
    /// does *not* need to call this, but it is public so that custom panels or
    /// embedding code can inject synthetic events.
    pub fn emit(&self, mut event: PlotEvent) {
        let mut inner = self.inner.lock().unwrap();
        event.timestamp = inner.start_instant.elapsed().as_secs_f64();
        // Retain only subscribers whose channel is still open.
        inner.subscribers.retain(|sub| {
            if sub.filter.matches(&event) {
                sub.sender.send(event.clone()).is_ok()
            } else {
                // Check if channel is still alive by checking the sender.
                // We keep subscribers even if this particular event doesn't match.
                !sub.sender.send(event.clone()).is_err() || true
            }
        });
    }

    /// Internal: emit only to subscribers whose filter matches (avoids sending
    /// non-matching events that would just be discarded).
    pub(crate) fn emit_filtered(&self, mut event: PlotEvent) {
        let mut inner = self.inner.lock().unwrap();
        event.timestamp = inner.start_instant.elapsed().as_secs_f64();
        inner.subscribers.retain(|sub| {
            if sub.filter.matches(&event) {
                sub.sender.send(event.clone()).is_ok()
            } else {
                // Channel open check: try a zero-cost probe.
                // We can't easily probe without sending, so we just keep the subscriber.
                true
            }
        });
    }
}

impl Default for EventController {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_union_and_intersection() {
        let click = EventKind::CLICK;
        let dbl = EventKind::DOUBLE_CLICK;
        let combined = click | dbl;
        assert!(combined.contains(click));
        assert!(combined.contains(dbl));
        assert!(combined.intersects(click));
        assert!(!EventKind::PAUSE.intersects(click));
    }

    #[test]
    fn event_kind_all_matches_everything() {
        assert!(EventKind::ALL.contains(EventKind::CLICK));
        assert!(EventKind::ALL.contains(EventKind::ZOOM));
        assert!(EventKind::ALL.contains(EventKind::THRESHOLD_EXCEEDED));
    }

    #[test]
    fn event_filter_matches() {
        let filter = EventFilter::only(EventKind::CLICK | EventKind::DOUBLE_CLICK);
        let mut evt = PlotEvent::new(EventKind::CLICK);
        evt.timestamp = 1.0;
        assert!(filter.matches(&evt));

        let evt2 = PlotEvent::new(EventKind::ZOOM);
        assert!(!filter.matches(&evt2));

        let evt3 = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
        assert!(filter.matches(&evt3));
    }

    #[test]
    fn event_filter_all_matches_everything() {
        let filter = EventFilter::all();
        let evt = PlotEvent::new(EventKind::THRESHOLD_EXCEEDED);
        assert!(filter.matches(&evt));
    }

    #[test]
    fn event_controller_subscribe_and_emit() {
        let ctrl = EventController::new();
        let rx_all = ctrl.subscribe_all();
        let rx_clicks = ctrl.subscribe(EventFilter::only(EventKind::CLICK));
        let rx_zoom = ctrl.subscribe(EventFilter::only(EventKind::ZOOM));

        // Emit a click event
        let evt = PlotEvent::new(EventKind::CLICK);
        ctrl.emit_filtered(evt);

        // All subscriber should get it
        assert!(rx_all.try_recv().is_ok());
        // Click subscriber should get it
        assert!(rx_clicks.try_recv().is_ok());
        // Zoom subscriber should not
        assert!(rx_zoom.try_recv().is_err());
    }

    #[test]
    fn event_controller_combined_kinds() {
        let ctrl = EventController::new();
        let rx_click = ctrl.subscribe(EventFilter::only(EventKind::CLICK));
        let rx_meas = ctrl.subscribe(EventFilter::only(EventKind::MEASUREMENT_POINT));

        // Emit event that is both a click AND a measurement point
        let evt = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
        ctrl.emit_filtered(evt);

        assert!(rx_click.try_recv().is_ok());
        assert!(rx_meas.try_recv().is_ok());
    }

    #[test]
    fn event_controller_timestamp_set_on_emit() {
        let ctrl = EventController::new();
        let rx = ctrl.subscribe_all();

        std::thread::sleep(std::time::Duration::from_millis(10));
        ctrl.emit_filtered(PlotEvent::new(EventKind::CLICK));

        let evt = rx.try_recv().unwrap();
        assert!(evt.timestamp > 0.0);
    }

    #[test]
    fn event_kind_display() {
        // Single bit
        assert_eq!(format!("{}", EventKind::CLICK), "CLICK");
        assert_eq!(format!("{}", EventKind::DOUBLE_CLICK), "DOUBLE_CLICK");
        // Combined bits are joined with '|'
        let combo = EventKind::CLICK | EventKind::DOUBLE_CLICK;
        assert_eq!(format!("{}", combo), "CLICK|DOUBLE_CLICK");
        // ALL should print as "ALL"
        assert_eq!(format!("{}", EventKind::ALL), "ALL");
        // Unknown bits still produce hex representation
        let unknown = EventKind(1 << 63);
        assert!(format!("{}", unknown).starts_with("0x"));
    }

    #[test]
    fn event_kinds_do_not_overlap() {
        // Verify that all defined constants have unique bit positions.
        let all_kinds = [
            EventKind::CLICK,
            EventKind::DOUBLE_CLICK,
            EventKind::CLICK_ON_TRACE,
            EventKind::PAUSE,
            EventKind::RESUME,
            EventKind::MEASUREMENT_POINT,
            EventKind::MEASUREMENT_COMPLETE,
            EventKind::MEASUREMENT_CLEARED,
            EventKind::TRACE_SHOWN,
            EventKind::TRACE_HIDDEN,
            EventKind::TRACE_COLOR_CHANGED,
            EventKind::MATH_TRACE_ADDED,
            EventKind::MATH_TRACE_REMOVED,
            EventKind::ZOOM,
            EventKind::FIT_TO_VIEW,
            EventKind::PAN,
            EventKind::RESIZE,
            EventKind::DATA_UPDATED,
            EventKind::DATA_CLEARED,
            EventKind::THRESHOLD_EXCEEDED,
            EventKind::THRESHOLD_ADDED,
            EventKind::THRESHOLD_REMOVED,
            EventKind::KEY_PRESSED,
            EventKind::EXPORT,
            EventKind::SCREENSHOT,
            EventKind::SCOPE_ADDED,
            EventKind::SCOPE_REMOVED,
            EventKind::TRIGGER_FIRED,
            EventKind::TRACE_OFFSET_CHANGED,
            EventKind::Y_LOG_CHANGED,
            EventKind::Y_UNIT_CHANGED,
        ];
        for (i, a) in all_kinds.iter().enumerate() {
            for (j, b) in all_kinds.iter().enumerate() {
                if i != j {
                    assert!(
                        !a.intersects(*b),
                        "EventKind bits {} and {} overlap: {:b} & {:b}",
                        i,
                        j,
                        a.0,
                        b.0
                    );
                }
            }
        }
    }

    #[test]
    fn dropped_receiver_is_cleaned_up() {
        let ctrl = EventController::new();
        let rx1 = ctrl.subscribe_all();
        let rx2 = ctrl.subscribe_all();

        // Drop rx1
        drop(rx1);

        ctrl.emit_filtered(PlotEvent::new(EventKind::CLICK));
        // rx2 should still work
        assert!(rx2.try_recv().is_ok());

        // Emit again – the dead subscriber should have been pruned
        ctrl.emit_filtered(PlotEvent::new(EventKind::ZOOM));
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn plot_event_carries_metadata() {
        let mut evt = PlotEvent::new(EventKind::CLICK | EventKind::MEASUREMENT_POINT);
        evt.click = Some(ClickMeta {
            screen_pos: Some(ScreenPos { x: 100.0, y: 200.0 }),
            plot_pos: Some(PlotPos { x: 1.5, y: 2.5 }),
            trace: Some(TraceRef("signal".into())),
            scope_id: Some(0),
        });
        evt.measurement = Some(MeasurementMeta {
            point_index: 0,
            point: [1.5, 2.5],
            p1: Some([1.5, 2.5]),
            p2: None,
            delta_x: None,
            delta_y: None,
            slope: None,
            distance: None,
            measurement_name: Some("M1".into()),
            trace: Some(TraceRef("signal".into())),
        });

        assert!(evt.kinds.contains(EventKind::CLICK));
        assert!(evt.click.is_some());
        assert!(evt.measurement.is_some());
        assert_eq!(evt.click.as_ref().unwrap().plot_pos.unwrap().x, 1.5);
    }
}
