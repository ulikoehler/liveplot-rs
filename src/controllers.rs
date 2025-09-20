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
