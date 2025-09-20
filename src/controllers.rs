//! Controllers for interacting with the UI from external code.
//!
//! The controllers expose lightweight state and a subscription mechanism so
//! non-UI code can observe window/panel state and push simple requests (like
//! toggling the FFT panel).

use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

/// Current window information (physical pixels).
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Last observed size of the entire window in physical pixels.
    pub current_size: Option<[f32; 2]>,
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
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) request_set_pos: Option<[f32; 2]>,
    pub(crate) listeners: Vec<Sender<WindowInfo>>,
}

impl WindowController {
    /// Create a fresh controller.
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(WindowCtrlInner { current_size: None, request_set_size: None, request_set_pos: None, listeners: Vec::new() })) }
    }

    /// Get the last observed window size in physical pixels (if known).
    pub fn get_current_size(&self) -> Option<[f32;2]> {
        self.inner.lock().unwrap().current_size
    }

    /// Request a window size change (physical pixels). The request is recorded and
    /// will be broadcast to listeners; whether the runtime honors it depends on the backend.
    pub fn request_set_size(&self, size_px: [f32;2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_size = Some(size_px);
    }

    /// Request a window position change (physical pixels). Recorded and broadcast to listeners.
    pub fn request_set_pos(&self, pos_px: [f32;2]) {
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
pub struct FftPanelInfo {
    /// Whether the FFT panel is currently shown
    pub shown: bool,
    /// Current panel size in physical pixels (width, height)
    pub current_size: Option<[f32; 2]>,
    /// Requested size (if any) set via controller
    pub requested_size: Option<[f32; 2]>,
}

/// Controller to get/set FFT panel visibility/size and subscribe to updates.
#[derive(Clone)]
pub struct FftController {
    pub(crate) inner: Arc<Mutex<FftCtrlInner>>, // crate-visible for UI
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
    pub(crate) fft_request: Option<FftDataRequest>,
    pub(crate) fft_listeners: Vec<Sender<FftRawData>>,
    pub(crate) request_screenshot_to: Option<std::path::PathBuf>,
    pub(crate) request_save_raw_to: Option<(RawExportFormat, std::path::PathBuf)>,
}

impl UiActionController {
    /// Create a fresh UI action controller.
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(UiActionInner {
            request_pause: None,
            request_screenshot: false,
            request_screenshot_to: None,
            request_save_raw: None,
            request_save_raw_to: None,
            fft_request: None,
            fft_listeners: Vec::new(),
        })) }
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
    pub fn request_save_raw_to_path<P: Into<std::path::PathBuf>>(&self, fmt: RawExportFormat, path: P) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_save_raw_to = Some((fmt, path.into()));
    }

    /// Subscribe to receive the current raw FFT input data (time-domain) for a trace.
    pub fn subscribe_fft_data(&self) -> std::sync::mpsc::Receiver<FftRawData> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.fft_listeners.push(tx);
        rx
    }

    /// Request FFT input data for the currently selected trace (if any).
    pub fn request_fft_data_current(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.fft_request = Some(FftDataRequest::CurrentTrace);
    }

    /// Request FFT input data for a specific named trace.
    pub fn request_fft_data_for<S: Into<String>>(&self, name: S) {
        let mut inner = self.inner.lock().unwrap();
        inner.fft_request = Some(FftDataRequest::NamedTrace(name.into()));
    }
}

/// Raw export format for saving captured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawExportFormat { Csv, Parquet }

/// Request for FFT raw input data.
#[derive(Debug, Clone)]
pub enum FftDataRequest { CurrentTrace, NamedTrace(String) }

/// Raw FFT input time-domain data for a single trace.
#[derive(Debug, Clone)]
pub struct FftRawData {
    pub trace: String,
    /// Time-domain points [t_seconds, value]
    pub data: Vec<[f64;2]>,
}

pub(crate) struct FftCtrlInner {
    pub(crate) show: bool,
    pub(crate) current_size: Option<[f32; 2]>,
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) listeners: Vec<Sender<FftPanelInfo>>,
}

impl FftController {
    /// Create a fresh controller.
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(FftCtrlInner { show: false, current_size: None, request_set_size: None, listeners: Vec::new() })) }
    }

    /// Query whether the FFT panel is (last known) shown.
    pub fn is_shown(&self) -> bool { self.inner.lock().unwrap().show }

    /// Request that the FFT panel be shown/hidden. This records the request and
    /// notifies subscribers; whether the runtime honors it depends on the UI.
    pub fn set_shown(&self, show: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.show = show;
        let info = FftPanelInfo { shown: inner.show, current_size: inner.current_size, requested_size: inner.request_set_size };
        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
    }

    /// Get last observed panel size in physical pixels (if known).
    pub fn get_current_size(&self) -> Option<[f32;2]> { self.inner.lock().unwrap().current_size }

    /// Request a panel size change (physical pixels). Recorded and will be
    /// exposed to the UI which may choose to honor it.
    pub fn request_set_size(&self, size_px: [f32;2]) {
        let mut inner = self.inner.lock().unwrap();
        inner.request_set_size = Some(size_px);
    }

    /// Subscribe to FFT panel updates. Returned receiver receives `FftPanelInfo` whenever the UI publishes it.
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<FftPanelInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.listeners.push(tx);
        rx
    }
}
