use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub current_size: Option<[f32; 2]>,
    pub current_pos: Option<[f32; 2]>,
    pub requested_size: Option<[f32; 2]>,
    pub requested_pos: Option<[f32; 2]>,
}

#[derive(Clone)]
pub struct WindowController {
    pub(crate) inner: Arc<Mutex<WindowCtrlInner>>,
}
pub(crate) struct WindowCtrlInner {
    pub(crate) current_size: Option<[f32; 2]>,
    pub(crate) current_pos: Option<[f32; 2]>,
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) request_set_pos: Option<[f32; 2]>,
    pub(crate) listeners: Vec<Sender<WindowInfo>>,
}
impl WindowController {
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
    pub fn get_current_size(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_size
    }
    pub fn get_current_pos(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_pos
    }
    pub fn request_set_size(&self, size_px: [f32; 2]) {
        self.inner.lock().unwrap().request_set_size = Some(size_px);
    }
    pub fn request_set_pos(&self, pos_px: [f32; 2]) {
        self.inner.lock().unwrap().request_set_pos = Some(pos_px);
    }
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<WindowInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.lock().unwrap().listeners.push(tx);
        rx
    }
}

#[derive(Debug, Clone)]
pub struct FftPanelInfo {
    pub shown: bool,
    pub current_size: Option<[f32; 2]>,
    pub requested_size: Option<[f32; 2]>,
}
#[derive(Clone)]
pub struct FftController {
    pub(crate) inner: Arc<Mutex<FftCtrlInner>>,
}
pub(crate) struct FftCtrlInner {
    pub(crate) show: bool,
    pub(crate) current_size: Option<[f32; 2]>,
    pub(crate) request_set_size: Option<[f32; 2]>,
    pub(crate) listeners: Vec<Sender<FftPanelInfo>>,
}
impl FftController {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FftCtrlInner {
                show: false,
                current_size: None,
                request_set_size: None,
                listeners: Vec::new(),
            })),
        }
    }
    pub fn is_shown(&self) -> bool {
        self.inner.lock().unwrap().show
    }
    pub fn set_shown(&self, show: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.show = show;
        let info = FftPanelInfo {
            shown: inner.show,
            current_size: inner.current_size,
            requested_size: inner.request_set_size,
        };
        inner.listeners.retain(|s| s.send(info.clone()).is_ok());
    }
    pub fn get_current_size(&self) -> Option<[f32; 2]> {
        self.inner.lock().unwrap().current_size
    }
    pub fn request_set_size(&self, size_px: [f32; 2]) {
        self.inner.lock().unwrap().request_set_size = Some(size_px);
    }
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<FftPanelInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.lock().unwrap().listeners.push(tx);
        rx
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawExportFormat {
    Csv,
    Parquet,
}
#[derive(Debug, Clone)]
pub enum FftDataRequest {
    CurrentTrace,
    NamedTrace(String),
}
#[derive(Debug, Clone)]
pub struct FftRawData {
    pub trace: String,
    pub data: Vec<[f64; 2]>,
}

#[derive(Clone)]
pub struct UiActionController {
    pub(crate) inner: Arc<Mutex<UiActionInner>>,
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
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(UiActionInner {
                request_pause: None,
                request_screenshot: false,
                request_save_raw: None,
                fft_request: None,
                fft_listeners: Vec::new(),
                request_screenshot_to: None,
                request_save_raw_to: None,
            })),
        }
    }
    pub fn pause(&self) {
        self.inner.lock().unwrap().request_pause = Some(true);
    }
    pub fn resume(&self) {
        self.inner.lock().unwrap().request_pause = Some(false);
    }
    pub fn request_save_png(&self) {
        self.inner.lock().unwrap().request_screenshot = true;
    }
    pub fn request_save_png_to_path<P: Into<std::path::PathBuf>>(&self, path: P) {
        self.inner.lock().unwrap().request_screenshot_to = Some(path.into());
    }
    pub fn request_save_raw(&self, fmt: RawExportFormat) {
        self.inner.lock().unwrap().request_save_raw = Some(fmt);
    }
    pub fn request_save_raw_to_path<P: Into<std::path::PathBuf>>(
        &self,
        fmt: RawExportFormat,
        path: P,
    ) {
        self.inner.lock().unwrap().request_save_raw_to = Some((fmt, path.into()));
    }
    pub fn subscribe_fft_data(&self) -> std::sync::mpsc::Receiver<FftRawData> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.lock().unwrap().fft_listeners.push(tx);
        rx
    }
    pub fn request_fft_data_current(&self) {
        self.inner.lock().unwrap().fft_request = Some(FftDataRequest::CurrentTrace);
    }
    pub fn request_fft_data_for<S: Into<String>>(&self, name: S) {
        self.inner.lock().unwrap().fft_request = Some(FftDataRequest::NamedTrace(name.into()));
    }
}

#[derive(Debug, Clone)]
pub struct TraceInfo {
    pub name: String,
    pub color_rgb: [u8; 3],
    pub visible: bool,
    pub is_math: bool,
    pub offset: f64,
}
#[derive(Debug, Clone)]
pub struct TracesInfo {
    pub traces: Vec<TraceInfo>,
    pub marker_selection: Option<String>,
    pub y_unit: Option<String>,
    pub y_log: bool,
}
#[derive(Clone)]
pub struct TracesController {
    pub(crate) inner: Arc<Mutex<TracesCtrlInner>>,
}
pub(crate) struct TracesCtrlInner {
    pub(crate) color_requests: Vec<(String, [u8; 3])>,
    pub(crate) visible_requests: Vec<(String, bool)>,
    pub(crate) offset_requests: Vec<(String, f64)>,
    pub(crate) y_unit_request: Option<Option<String>>,
    pub(crate) y_log_request: Option<bool>,
    pub(crate) selection_request: Option<Option<String>>,
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
                listeners: Vec::new(),
            })),
        }
    }
    pub fn request_set_color<S: Into<String>>(&self, name: S, rgb: [u8; 3]) {
        self.inner
            .lock()
            .unwrap()
            .color_requests
            .push((name.into(), rgb));
    }
    pub fn request_set_visible<S: Into<String>>(&self, name: S, visible: bool) {
        self.inner
            .lock()
            .unwrap()
            .visible_requests
            .push((name.into(), visible));
    }
    pub fn request_set_offset<S: Into<String>>(&self, name: S, offset: f64) {
        self.inner
            .lock()
            .unwrap()
            .offset_requests
            .push((name.into(), offset));
    }
    pub fn request_set_y_unit<S: Into<String>>(&self, unit: Option<S>) {
        self.inner.lock().unwrap().y_unit_request = Some(unit.map(|s| s.into()));
    }
    pub fn request_set_y_log(&self, enable: bool) {
        self.inner.lock().unwrap().y_log_request = Some(enable);
    }
    pub fn request_select_marker_free(&self) {
        self.inner.lock().unwrap().selection_request = Some(None);
    }
    pub fn request_select_marker_trace<S: Into<String>>(&self, name: S) {
        self.inner.lock().unwrap().selection_request = Some(Some(name.into()));
    }
    pub fn subscribe(&self) -> std::sync::mpsc::Receiver<TracesInfo> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.lock().unwrap().listeners.push(tx);
        rx
    }
}
