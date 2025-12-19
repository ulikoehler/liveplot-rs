//! Controllers for interacting with the UI from external code.
//!
//! The controllers expose lightweight state and a subscription mechanism so
//! non-UI code can observe window/panel state and push simple requests (like
//! toggling the FFT panel).

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::data::scope::AxisSettings;
use crate::data::scope::ScopeType;
use crate::data::traces::TraceRef;
use egui_plot::LineStyle;

/// Current window information (physical pixels).
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Last observed size of the entire window in physical pixels.
    pub current_size: Option<[f32; 2]>,
    /// Last observed outer position of the window in physical pixels (top-left), if available.
    /// Note: not all backends expose this reliably; may be None.
    pub current_pos: Option<[f32; 2]>,
    /// Requested size (if any) set via controller. Whether it is applied
    /// depends on the backend/platform.
    pub requested_size: Option<[f32; 2]>,
    /// Requested window position (if any) in physical pixels.
    pub requested_pos: Option<[f32; 2]>,
}

/// Controller to get/set window info and subscribe to updates.
#[derive(Clone)]
pub struct WindowController {
    pub(crate) inner: Arc<Mutex<WindowCtrlInner>>, // crate-visible for UI
}

pub(crate) struct WindowCtrlInner {
    pub(crate) current_size: Option<[f32; 2]>,
    pub(crate) current_pos: Option<[f32; 2]>,
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) request_set_pos: Option<[f32; 2]>,
    pub(crate) listeners: Vec<Sender<WindowInfo>>,
}

impl WindowController {
    /// Create a fresh controller.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WindowCtrlInner {
                current_size: None,
                current_pos: None,
                request_set_size: None,
                request_set_pos: None,
                listeners: Vec::new(),
            })),
        }
    }

    /// Get the last observed window size in physical pixels (if known).
    pub fn get_current_size(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_size
    }

    /// Get the last observed window position in physical pixels (if known).
    pub fn get_current_pos(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_pos
    }

    /// Request a window size change (physical pixels). The request is recorded and
    /// will be broadcast to listeners; whether the runtime honors it depends on the backend.
    pub fn request_set_size(&self, size_px: [f32; 2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_size = Some(size_px);
    }

    /// Request a window position change (physical pixels). Recorded and broadcast to listeners.
    pub fn request_set_pos(&self, pos_px: [f32; 2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_pos = Some(pos_px);
    }

    /// Subscribe to window info updates. Returned receiver receives `WindowInfo` whenever the UI publishes it.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<WindowInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        rx
    }
}

/// Information about the FFT bottom panel (shown + size in physical pixels)
#[derive(Debug, Clone)]
pub struct FFTPanelInfo {
    /// Whether the FFT panel is currently shown
    pub shown: bool,
    /// Current panel size in physical pixels (width, height)
    pub current_size: Option<[f32; 2]>,
    /// Requested size (if any) set via controller
    pub requested_size: Option<[f32; 2]>,
}

/// Controller to get/set FFT panel visibility/size and subscribe to updates.
#[derive(Clone)]
pub struct FFTController {
    pub(crate) inner: Arc<Mutex<FFTCtrlInner>>, // crate-visible for UI
}

/// Controller for high-level UI actions like pause/resume and saving a PNG.
///
/// External code can request pausing/resuming the live stream display and trigger
/// a screenshot (equivalent to the UI's "Save PNG" button). The screenshot request
/// behaves like the UI: it will open a save dialog for the user.
#[derive(Clone)]
pub struct UiActionController {
    pub(crate) inner: Arc<Mutex<UiActionInner>>, // crate-visible for UI
}

pub(crate) struct UiActionInner {
    pub(crate) request_pause: Option<bool>,
    pub(crate) request_screenshot: bool,
    pub(crate) request_save_raw: Option<RawExportFormat>,
    pub(crate) fft_request: Option<FFTDataRequest>,
    pub(crate) fft_listeners: Vec<Sender<FFTRawData>>,
    pub(crate) request_screenshot_to: Option<std::path::PathBuf>,
    pub(crate) request_save_raw_to: Option<(RawExportFormat, std::path::PathBuf)>,
}

impl UiActionController {
    /// Create a fresh UI action controller.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(UiActionInner {
                request_pause: None,
                request_screenshot: false,
                request_screenshot_to: None,
                request_save_raw: None,
                request_save_raw_to: None,
                fft_request: None,
                fft_listeners: Vec::new(),
            })),
        }
    }

    /// Request the UI to pause (freeze) the time-domain display.
    pub fn pause(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_pause = Some(true);
    }

    /// Request the UI to resume live updates.
    pub fn resume(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_pause = Some(false);
    }

    /// Request the UI to take a screenshot and prompt to save as PNG.
    pub fn request_save_png(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_screenshot = true;
    }

    /// Request the UI to save a PNG screenshot to the exact provided path (non-interactive).
    pub fn request_save_png_to_path<P: Into<std::path::PathBuf>>(&self, path: P) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_screenshot_to = Some(path.into());
    }

    /// Request saving raw time-domain data; the UI will prompt for a filename.
    pub fn request_save_raw(&self, fmt: RawExportFormat) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_save_raw = Some(fmt);
    }

    /// Request saving raw data directly to the given path (non-interactive).
    pub fn request_save_raw_to_path<P: Into<std::path::PathBuf>>(
        &self,
        fmt: RawExportFormat,
        path: P,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_save_raw_to = Some((fmt, path.into()));
    }

    /// Subscribe to receive the current raw FFT input data (time-domain) for a trace.
    pub fn subscribe_fft_data(&self) -> std::sync::mpsc::Receiver<FFTRawData> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.fft_listeners.push(tx);
        rx
    }

    /// Request FFT input data for the currently selected trace (if any).
    pub fn request_fft_data_current(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.fft_request = Some(FFTDataRequest::CurrentTrace);
    }

    /// Request FFT input data for a specific named trace.
    pub fn request_fft_data_for<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.fft_request = Some(FFTDataRequest::NamedTrace(name.into()));
    }
}

/// Raw export format for saving captured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawExportFormat {
    Csv,
    Parquet,
}

/// Request for FFT raw input data.
#[derive(Debug, Clone)]
pub enum FFTDataRequest {
    CurrentTrace,
    NamedTrace(String),
}

/// Raw FFT input time-domain data for a single trace.
#[derive(Debug, Clone)]
pub struct FFTRawData {
    pub trace: String,
    /// Time-domain points [t_seconds, value]
    pub data: Vec<[f64; 2]>,
}

pub(crate) struct FFTCtrlInner {
    pub(crate) show: bool,
    pub(crate) current_size: Option<[f32; 2]>,
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) last_info: Option<FFTPanelInfo>,
    pub(crate) listeners: Vec<Sender<FFTPanelInfo>>,
}

impl FFTController {
    /// Create a fresh controller.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FFTCtrlInner {
                show: false,
                current_size: None,
                request_set_size: None,
                last_info: None,
                listeners: Vec::new(),
            })),
        }
    }

    /// Query whether the FFT panel is (last known) shown.
    pub fn is_shown(&self) -> bool {
        self.inner.lock().unwrap().show
    }

    /// Request that the FFT panel be shown/hidden. This records the request and
    /// notifies subscribers; whether the runtime honors it depends on the UI.
    pub fn set_shown(&self, show: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.show = show;
        let info = FFTPanelInfo {
            shown: inner.show,
            current_size: inner.current_size,
            requested_size: inner.request_set_size,
        };
        inner.last_info = Some(info.clone());
        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
    }

    /// Get last observed panel size in physical pixels (if known).
    pub fn get_current_size(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_size
    }

    /// Request a panel size change (physical pixels). Recorded and will be
    /// exposed to the UI which may choose to honor it.
    pub fn request_set_size(&self, size_px: [f32; 2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_size = Some(size_px);
    }

    /// Subscribe to FFT panel updates. Returned receiver receives `FFTPanelInfo` whenever the UI publishes it.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<FFTPanelInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        if let Some(info) = inner.last_info.clone() {
            let _ = inner.listeners.last().unwrap().send(info);
        }
        rx
    }

    /// Get the last published FFT panel info, if any.
    pub fn get_last_info(&self) -> Option<FFTPanelInfo> {
        self.inner.lock().unwrap().last_info.clone()
    }
}

/// Information about a single trace for external observation.
#[derive(Debug, Clone)]
pub struct TraceInfo {
    pub name: String,
    pub color_rgb: [u8; 3],
    pub visible: bool,
    pub is_math: bool,
    /// Additive offset applied to Y before plotting or log-transform
    pub offset: f64,
}

/// Snapshot of all traces and current marker selection.
#[derive(Debug, Clone)]
pub struct TracesInfo {
    pub traces: Vec<TraceInfo>,
    pub y_unit: Option<String>,
    pub y_log: bool,
}

/// Rich state snapshot for the traces panel, used by the new controller API.
#[derive(Clone, Debug)]
pub struct TracesPanelState {
    pub max_points: usize,
    pub points_bounds: (usize, usize),
    pub hover_trace: Option<TraceRef>,
    pub traces: Vec<TraceControlState>,
    pub show: bool,
    pub detached: bool,
}

#[derive(Clone, Debug)]
pub struct TraceControlState {
    pub name: TraceRef,
    pub color_rgb: [u8; 3],
    pub width: f32,
    pub style: LineStyle,
    pub visible: bool,
    pub offset: f64,
    pub is_math: bool,
}

/// Controller to observe and modify traces UI state (color/visibility/marker selection).
#[derive(Clone)]
pub struct TracesController {
    pub(crate) inner: Arc<Mutex<TracesCtrlInner>>, // crate-visible for UI
}

pub(crate) struct TracesCtrlInner {
    pub(crate) color_requests: Vec<(String, [u8; 3])>,
    pub(crate) visible_requests: Vec<(String, bool)>,
    pub(crate) offset_requests: Vec<(String, f64)>,
    pub(crate) y_unit_request: Option<Option<String>>,
    pub(crate) y_log_request: Option<bool>,
    pub(crate) selection_request: Option<Option<String>>, // Some(None)=Free, Some(Some(name))=select, None=no-op
    pub(crate) hover_request: Option<Option<String>>, // Some(None)=clear, Some(Some(name))=highlight
    pub(crate) max_points_request: Option<usize>,
    pub(crate) points_bounds_request: Option<(usize, usize)>,
    pub(crate) hover_trace_request: Option<Option<TraceRef>>,
    pub(crate) show_request: Option<bool>,
    pub(crate) detached_request: Option<bool>,
    pub(crate) width_requests: Vec<(String, f32)>,
    pub(crate) style_requests: Vec<(String, LineStyle)>,
    pub(crate) listeners: Vec<Sender<TracesInfo>>,
    pub(crate) panel_listeners: Vec<Sender<TracesPanelState>>,
    pub(crate) last_snapshot: Option<TracesInfo>,
    pub(crate) last_panel_state: Option<TracesPanelState>,
}

impl TracesController {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TracesCtrlInner {
                color_requests: Vec::new(),
                visible_requests: Vec::new(),
                offset_requests: Vec::new(),
                y_unit_request: None,
                y_log_request: None,
                selection_request: None,
                hover_request: None,
                max_points_request: None,
                points_bounds_request: None,
                hover_trace_request: None,
                show_request: None,
                detached_request: None,
                width_requests: Vec::new(),
                style_requests: Vec::new(),
                listeners: Vec::new(),
                panel_listeners: Vec::new(),
                last_snapshot: None,
                last_panel_state: None,
            })),
        }
    }

    /// Request setting the RGB color of a trace by name.
    pub fn request_set_color<S: Into<String>>(&self, name: S, rgb: [u8; 3]) {
        let mut inner = self.inner.lock().unwrap();
        inner.color_requests.push((name.into(), rgb));
    }

    /// Request setting the visibility of a trace by name.
    pub fn request_set_visible<S: Into<String>>(&self, name: S, visible: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.visible_requests.push((name.into(), visible));
    }

    /// Request setting the Y offset of a trace by name.
    pub fn request_set_offset<S: Into<String>>(&self, name: S, offset: f64) {
        let mut inner = self.inner.lock().unwrap();
        inner.offset_requests.push((name.into(), offset));
    }

    pub fn request_set_max_points(&self, v: usize) {
        self.inner.lock().unwrap().max_points_request = Some(v);
    }

    pub fn request_set_points_bounds(&self, bounds: (usize, usize)) {
        self.inner.lock().unwrap().points_bounds_request = Some(bounds);
    }

    pub fn request_set_hover_trace(&self, trace: Option<TraceRef>) {
        self.inner.lock().unwrap().hover_trace_request = Some(trace);
    }

    /// Request setting the Y axis unit (value axes only). Pass `None` to clear.
    pub fn request_set_y_unit<S: Into<Option<String>>>(&self, unit: S) {
        self.inner.lock().unwrap().y_unit_request = Some(unit.into());
    }
    pub fn request_set_show(&self, show: bool) {
        self.inner.lock().unwrap().show_request = Some(show);
    }

    pub fn request_set_detached(&self, detached: bool) {
        self.inner.lock().unwrap().detached_request = Some(detached);
    }

    pub fn request_set_width<S: Into<String>>(&self, name: S, width: f32) {
        self.inner
            .lock()
            .unwrap()
            .width_requests
            .push((name.into(), width));
    }

    pub fn request_set_style<S: Into<String>>(&self, name: S, style: LineStyle) {
        self.inner
            .lock()
            .unwrap()
            .style_requests
            .push((name.into(), style));
    }

    /// Request toggling Y log scale.
    pub fn request_set_y_log(&self, enable: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.y_log_request = Some(enable);
    }

    /// Request selecting the marker to be "Free" (no snapping).
    pub fn request_select_marker_free(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.selection_request = Some(None);
    }

    /// Request selecting a specific trace for markers.
    pub fn request_select_marker_trace<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.selection_request = Some(Some(name.into()));
    }

    /// Request highlighting a trace (similar to hovering it in the UI).
    pub fn request_highlight_trace<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.hover_request = Some(Some(name.into()));
    }

    /// Clear any externally requested highlight.
    pub fn clear_highlight(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.hover_request = Some(None);
    }

    /// Subscribe to receive updates about traces and current selection.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<TracesInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        if let Some(last) = inner.last_snapshot.clone() {
            let _ = inner.listeners.last().unwrap().send(last);
        }
        rx
    }

    /// Subscribe to the richer panel state (max points, bounds, show/detached, styles).
    pub fn subscribe_panel_state(&self) -> std::sync::mpsc::Receiver<TracesPanelState> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.panel_listeners.push(tx);
        if let Some(last) = inner.last_panel_state.clone() {
            let _ = inner.panel_listeners.last().unwrap().send(last);
        }
        rx
    }

    /// Get the last published traces snapshot, if any.
    pub fn get_last_snapshot(&self) -> Option<TracesInfo> {
        self.inner.lock().unwrap().last_snapshot.clone()
    }

    /// Get the last published traces panel state, if any.
    pub fn get_last_panel_state(&self) -> Option<TracesPanelState> {
        self.inner.lock().unwrap().last_panel_state.clone()
    }
}

/// Controller to manage threshold definitions and subscribe to threshold events.
#[derive(Clone)]
pub struct ThresholdController {
    pub(crate) inner: Arc<Mutex<ThresholdCtrlInner>>, // crate-visible for UI
}

pub(crate) struct ThresholdCtrlInner {
    pub(crate) add_requests: Vec<crate::data::thresholds::ThresholdDef>,
    pub(crate) remove_requests: Vec<String>,
    pub(crate) listeners: Vec<Sender<crate::data::thresholds::ThresholdEvent>>, // name + events
}

/// Per-scope control/state snapshot.
#[derive(Clone, Debug)]
pub struct ScopeControlState {
    pub id: usize,
    pub name: String,
    pub y_axis: AxisSettings,
    pub x_axis: AxisSettings,
    pub time_window: f64,
    pub paused: bool,
    pub show_legend: bool,
    pub show_info_in_legend: bool,
    pub trace_order: Vec<TraceRef>,
    pub scope_type: ScopeType,
}

#[derive(Clone, Debug)]
pub struct ScopesState {
    pub scopes: Vec<ScopeControlState>,
    pub show: bool,
    pub detached: bool,
}

#[derive(Default)]
pub struct ScopeRequests {
    pub set_show: Option<bool>,
    pub set_detached: Option<bool>,
    pub set_scopes: Vec<ScopeControlState>,
    pub add_scope: bool,
    pub remove_scope: Option<usize>,
    pub reorder: Option<Vec<usize>>, // new order by scope id
    pub save_screenshot: bool,
}

#[derive(Clone)]
pub struct ScopesController {
    pub(crate) inner: Arc<Mutex<ScopeCtrlInner>>, // crate-visible for UI
}

pub(crate) struct ScopeCtrlInner {
    pub(crate) requests: ScopeRequests,
    pub(crate) last_state: Option<ScopesState>,
    pub(crate) listeners: Vec<Sender<ScopesState>>,
}

impl ScopesController {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScopeCtrlInner {
                requests: ScopeRequests::default(),
                last_state: None,
                listeners: Vec::new(),
            })),
        }
    }

    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<ScopesState> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        if let Some(last) = inner.last_state.clone() {
            let _ = inner.listeners.last().unwrap().send(last);
        }
        rx
    }

    /// Get the last published scopes state, if any.
    pub fn get_last_state(&self) -> Option<ScopesState> {
        self.inner.lock().unwrap().last_state.clone()
    }

    pub fn request_set_show(&self, show: bool) {
        self.inner.lock().unwrap().requests.set_show = Some(show);
    }

    pub fn request_set_detached(&self, detached: bool) {
        self.inner.lock().unwrap().requests.set_detached = Some(detached);
    }

    pub fn request_add_scope(&self) {
        self.inner.lock().unwrap().requests.add_scope = true;
    }

    pub fn request_remove_scope(&self, id: usize) {
        self.inner.lock().unwrap().requests.remove_scope = Some(id);
    }

    pub fn request_reorder(&self, order_by_id: Vec<usize>) {
        self.inner.lock().unwrap().requests.reorder = Some(order_by_id);
    }

    pub fn request_save_screenshot(&self) {
        self.inner.lock().unwrap().requests.save_screenshot = true;
    }

    pub fn request_replace_scopes(&self, scopes: Vec<ScopeControlState>) {
        self.inner.lock().unwrap().requests.set_scopes = scopes;
    }
}

/// Global liveplot controller (window/frame + high-level actions).
#[derive(Clone, Debug)]
pub struct LiveplotState {
    pub paused: bool,
    pub show: bool,
    pub detached: bool,
    pub window_size: Option<[f32; 2]>,
    pub window_pos: Option<[f32; 2]>,
    pub fft_size: Option<usize>,
}

#[derive(Default)]
pub struct LiveplotRequests {
    pub pause_all: Option<bool>,
    pub clear_all: bool,
    pub save_state: Option<PathBuf>,
    pub load_state: Option<PathBuf>,
    pub set_window_size: Option<[f32; 2]>,
    pub set_window_pos: Option<[f32; 2]>,
    pub request_focus: bool,
    pub set_fft_size: Option<usize>,
    pub add_scope: bool,
    pub remove_scope: Option<usize>,
    pub reorder_scopes: Option<Vec<usize>>, // ids in order
}

#[derive(Clone)]
pub struct LiveplotController {
    pub(crate) inner: Arc<Mutex<LiveplotCtrlInner>>, // crate-visible for UI
}

pub(crate) struct LiveplotCtrlInner {
    pub(crate) requests: LiveplotRequests,
    pub(crate) last_state: Option<LiveplotState>,
    pub(crate) listeners: Vec<Sender<LiveplotState>>,
}

impl LiveplotController {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LiveplotCtrlInner {
                requests: LiveplotRequests::default(),
                last_state: None,
                listeners: Vec::new(),
            })),
        }
    }

    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<LiveplotState> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        if let Some(last) = inner.last_state.clone() {
            let _ = inner.listeners.last().unwrap().send(last);
        }
        rx
    }

    /// Get the last published liveplot state, if any.
    pub fn get_last_state(&self) -> Option<LiveplotState> {
        self.inner.lock().unwrap().last_state.clone()
    }

    pub fn request_pause_all(&self, pause: bool) {
        self.inner.lock().unwrap().requests.pause_all = Some(pause);
    }

    pub fn request_clear_all(&self) {
        self.inner.lock().unwrap().requests.clear_all = true;
    }

    pub fn request_save_state<P: Into<PathBuf>>(&self, path: P) {
        self.inner.lock().unwrap().requests.save_state = Some(path.into());
    }

    pub fn request_load_state<P: Into<PathBuf>>(&self, path: P) {
        self.inner.lock().unwrap().requests.load_state = Some(path.into());
    }

    pub fn request_set_window_size(&self, size: [f32; 2]) {
        self.inner.lock().unwrap().requests.set_window_size = Some(size);
    }

    pub fn request_set_window_pos(&self, pos: [f32; 2]) {
        self.inner.lock().unwrap().requests.set_window_pos = Some(pos);
    }

    pub fn request_focus(&self) {
        self.inner.lock().unwrap().requests.request_focus = true;
    }

    pub fn request_set_fft_size(&self, size: usize) {
        self.inner.lock().unwrap().requests.set_fft_size = Some(size);
    }

    pub fn request_add_scope(&self) {
        self.inner.lock().unwrap().requests.add_scope = true;
    }

    pub fn request_remove_scope(&self, id: usize) {
        self.inner.lock().unwrap().requests.remove_scope = Some(id);
    }

    pub fn request_reorder_scopes(&self, order: Vec<usize>) {
        self.inner.lock().unwrap().requests.reorder_scopes = Some(order);
    }
}

impl ThresholdController {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ThresholdCtrlInner {
                add_requests: Vec::new(),
                remove_requests: Vec::new(),
                listeners: Vec::new(),
            })),
        }
    }

    /// Request adding a threshold definition to the UI.
    pub fn request_add_threshold(&self, def: crate::data::thresholds::ThresholdDef) {
        let mut inner = self.inner.lock().unwrap();
        inner.add_requests.push(def);
    }

    /// Request removing a threshold by name.
    pub fn request_remove_threshold<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.remove_requests.push(name.into());
    }

    /// Subscribe to threshold events fired by the UI.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<crate::data::thresholds::ThresholdEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        rx
    }
}
