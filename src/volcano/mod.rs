//! VolcanoPlot (C++/Vulkan) plot backend — feature `volcano`.
//!
//! Wraps the C ABI in `vendor/VolcanoPlot` (`volcano_ffi`): each
//! [`VolcanoFigure`] owns a headless backend + renderer + axes and
//! renders trace data to an RGBA8 buffer that the scope UI uploads as
//! an egui texture. See `docs/volcano-integration.md`.

mod surface;
pub use surface::VolcanoSurface;

use std::ffi::CString;

#[allow(non_camel_case_types)]
enum VpFigure {}

extern "C" {
    fn vp_figure_create(width: i32, height: i32) -> *mut VpFigure;
    fn vp_figure_destroy(fig: *mut VpFigure);
    fn vp_figure_resize(fig: *mut VpFigure, width: i32, height: i32) -> i32;
    fn vp_trace_add(fig: *mut VpFigure) -> u64;
    fn vp_trace_set(
        fig: *mut VpFigure,
        id: u64,
        xy: *const f64,
        n: usize,
        rgba8: u32,
        width: f32,
        style: i32,
    );
    fn vp_trace_slide(
        fig: *mut VpFigure,
        id: u64,
        drop: u32,
        tail: *const f64,
        tail_n: usize,
        rgba8: u32,
        width: f32,
        style: i32,
    );
    fn vp_trace_set_marker(fig: *mut VpFigure, id: u64, marker: i32, size: f32);
    fn vp_trace_remove(fig: *mut VpFigure, id: u64);
    fn vp_set_viewport(fig: *mut VpFigure, xmin: f64, xmax: f64, ymin: f64, ymax: f64);
    fn vp_set_axis_labels(
        fig: *mut VpFigure,
        x_label: *const std::ffi::c_char,
        y_label: *const std::ffi::c_char,
    );
    fn vp_set_grid(fig: *mut VpFigure, on: i32);
    fn vp_set_background(fig: *mut VpFigure, white: i32);
    fn vp_set_margins(fig: *mut VpFigure, l: f32, r: f32, b: f32, t: f32);
    fn vp_set_background_rgba(fig: *mut VpFigure, r: f32, g: f32, b: f32);
    fn vp_render_rgba(fig: *mut VpFigure, out: *mut u8, out_len: usize) -> usize;
    fn vp_render_rgba_view(fig: *mut VpFigure, out_len: *mut usize) -> *const u8;
}

/// Trace line style matching the FFI `style` argument.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VolcanoLineStyle {
    Solid = 0,
    Dashed = 1,
    Dotted = 2,
    PointsOnly = 3,
}

/// A headless VolcanoPlot figure (one per scope).
pub struct VolcanoFigure {
    ptr: *mut VpFigure,
    w: i32,
    h: i32,
    buf: Vec<u8>,
}

// Each figure owns its own Vulkan device state; safe to move between
// threads, not to share concurrently.
unsafe impl Send for VolcanoFigure {}

impl VolcanoFigure {
    /// Create a headless figure. Returns `None` when Vulkan init fails.
    pub fn new(w: u32, h: u32) -> Option<Self> {
        let ptr = unsafe { vp_figure_create(w as i32, h as i32) };
        if ptr.is_null() {
            return None;
        }
        Some(Self {
            ptr,
            w: w as i32,
            h: h as i32,
            buf: vec![0; (w * h * 4) as usize],
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.w as u32, self.h as u32)
    }

    /// Resize the offscreen surface (recreates the backend). Traces are
    /// kept — they're figure-layer objects.
    pub fn resize(&mut self, w: u32, h: u32) -> bool {
        if unsafe { vp_figure_resize(self.ptr, w as i32, h as i32) } != 0 {
            self.w = w as i32;
            self.h = h as i32;
            self.buf.resize((w * h * 4) as usize, 0);
            true
        } else {
            false
        }
    }

    /// Register a trace slot; returns its id.
    pub fn add_trace(&mut self) -> u64 {
        unsafe { vp_trace_add(self.ptr) }
    }

    /// Replace a trace's points and style.
    pub fn set_trace(
        &mut self,
        id: u64,
        xy: &[[f64; 2]],
        rgba8: u32,
        width: f32,
        style: VolcanoLineStyle,
    ) {
        unsafe {
            vp_trace_set(
                self.ptr,
                id,
                xy.as_ptr() as *const f64,
                xy.len(),
                rgba8,
                width,
                style as i32,
            )
        }
    }

    /// Sliding-window update: `drop` points evicted at the front,
    /// `tail` appended at the back — uploads only the tail.
    pub fn set_trace_slide(
        &mut self,
        id: u64,
        drop: u32,
        tail: &[[f64; 2]],
        rgba8: u32,
        width: f32,
        style: VolcanoLineStyle,
    ) {
        unsafe {
            vp_trace_slide(
                self.ptr,
                id,
                drop,
                tail.as_ptr() as *const f64,
                tail.len(),
                rgba8,
                width,
                style as i32,
            )
        }
    }

    /// Marker glyph at each point — independent of the line style.
    /// `marker` is a `plot::MarkerStyle` ordinal; `MARKER_NONE` = 40.
    pub fn set_marker(&mut self, id: u64, marker: i32, size: f32) {
        unsafe { vp_trace_set_marker(self.ptr, id, marker, size) }
    }

    pub fn remove_trace(&mut self, id: u64) {
        unsafe { vp_trace_remove(self.ptr, id) }
    }

    /// Grid lines on/off (egui `show_grid` equivalent).
    pub fn set_grid(&mut self, on: bool) {
        unsafe { vp_set_grid(self.ptr, on as i32) }
    }

    /// Set viewport bounds; pass `f64::NAN` pairs to keep autoscale.
    pub fn set_viewport(&mut self, xmin: f64, xmax: f64, ymin: f64, ymax: f64) {
        unsafe { vp_set_viewport(self.ptr, xmin, xmax, ymin, ymax) }
    }

    /// Canvas+axes background to an exact color — pass egui's
    /// panel_fill so the figure margins blend into the egui chrome.
    pub fn set_background_rgba(&mut self, r: f32, g: f32, b: f32) {
        unsafe { vp_set_background_rgba(self.ptr, r, g, b) }
    }

    /// Grid margins as canvas fractions — must match the overlay
    /// axes-rect the caller paints overlays into.
    pub fn set_margins(&mut self, l: f32, r: f32, b: f32, t: f32) {
        unsafe { vp_set_margins(self.ptr, l, r, b, t) }
    }

    pub fn set_axis_labels(&mut self, x: Option<&str>, y: Option<&str>) {
        let x_c = x.map(|s| CString::new(s).unwrap_or_default());
        let y_c = y.map(|s| CString::new(s).unwrap_or_default());
        // Borrow — the CStrings must outlive the FFI call. (.map owned
        // them and freed the buffer before the call — a dangling ptr.)
        let x_p = x_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
        let y_p = y_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
        unsafe { vp_set_axis_labels(self.ptr, x_p, y_p) }
    }

    /// Light vs dark figure styling.
    pub fn set_background(&mut self, white: bool) {
        unsafe { vp_set_background(self.ptr, white as i32) }
    }

    /// Render one frame; returns the RGBA8 framebuffer (w*h*4 bytes).
    pub fn render_rgba(&mut self) -> Option<&[u8]> {
        let n = unsafe { vp_render_rgba(self.ptr, self.buf.as_mut_ptr(), self.buf.len()) };
        (n == self.buf.len()).then_some(&self.buf[..])
    }

    /// Render one frame straight into `out` (resized to w*h*4) —
    /// skips the internal `buf` so the result can travel without an
    /// extra copy.
    pub fn render_rgba_into(&mut self, out: &mut Vec<u8>) -> Option<(u32, u32)> {
        out.resize((self.w * self.h * 4) as usize, 0);
        let n = unsafe { vp_render_rgba(self.ptr, out.as_mut_ptr(), out.len()) };
        (n == out.len()).then_some((self.w as u32, self.h as u32))
    }

    /// Render one frame and borrow the backend's staging buffer —
    /// skips the staging→caller memcpy entirely. The slice is valid
    /// until the next render call on this figure. Returns None when
    /// the backend has no view support (fall back to render_rgba_into).
    pub fn render_rgba_view(&mut self) -> Option<(u32, u32, &[u8])> {
        let mut len = 0usize;
        let p = unsafe { vp_render_rgba_view(self.ptr, &mut len) };
        if p.is_null() || len == 0 {
            return None;
        }
        // SAFETY: `p` points into the figure's host-mapped staging
        // buffer, valid for `len` bytes until the next render call.
        Some((self.w as u32, self.h as u32, unsafe {
            std::slice::from_raw_parts(p, len)
        }))
    }
}

impl Drop for VolcanoFigure {
    fn drop(&mut self) {
        unsafe { vp_figure_destroy(self.ptr) }
    }
}
