//! Controllers for interacting with the UI from external code.
//!
//! The controllers expose lightweight state and a subscription mechanism so
//! non-UI code can observe window/panel state and push simple requests (like
//! toggling the FFT panel).

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

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
        rx
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
    pub(crate) listeners: Vec<Sender<TracesInfo>>,
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
                listeners: Vec::new(),
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

    /// Request setting the global Y unit label (None for no unit).
    pub fn request_set_y_unit<S: Into<String>>(&self, unit: Option<S>) {
        let mut inner = self.inner.lock().unwrap();
        inner.y_unit_request = Some(unit.map(|s| s.into()));
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
        rx
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
