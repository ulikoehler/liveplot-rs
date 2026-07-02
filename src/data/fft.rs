use egui::ahash::HashMap;
// FFT logic for time-series data, extracted from main.rs
// Provides windowing and spectrum calculation utilities for plotting
#[cfg(feature = "fft")]
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::collections::VecDeque;

use crate::data::traces::{TraceData, TraceRef};

/// Supported FFT window functions for spectral analysis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FFTWindow {
    /// Rectangular (no windowing)
    Rect,
    /// Hann window
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window
    Blackman,
}

impl Default for FFTWindow {
    fn default() -> Self {
        FFTWindow::Hann
    }
}

impl FFTWindow {
    /// All available window types (for UI selection)
    pub const ALL: &'static [FFTWindow] = &[
        FFTWindow::Rect,
        FFTWindow::Hann,
        FFTWindow::Hamming,
        FFTWindow::Blackman,
    ];

    /// Human-readable label for each window type
    pub fn label(&self) -> &'static str {
        match self {
            FFTWindow::Rect => "Rect",
            FFTWindow::Hann => "Hann",
            FFTWindow::Hamming => "Hamming",
            FFTWindow::Blackman => "Blackman",
        }
    }

    /// Compute the window weight for a given sample index
    pub fn weight(&self, n: usize, len: usize) -> f64 {
        match self {
            FFTWindow::Rect => 1.0,
            FFTWindow::Hann => {
                // Hann window: w[n] = 0.5 - 0.5*cos(2*pi*n/(N-1))
                0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
            FFTWindow::Hamming => {
                // Hamming window: w[n] = 0.54 - 0.46*cos(2*pi*n/(N-1))
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
            FFTWindow::Blackman => {
                // Blackman window: w[n] = 0.42 - 0.5*cos(2*pi*n/(N-1)) + 0.08*cos(4*pi*n/(N-1))
                0.42 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
                    + 0.08 * (4.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
        }
    }
}

#[cfg(feature = "fft")]
struct FftJob {
    trace_ref: TraceRef,
    samples: Vec<f64>,
    fft_size: usize,
    padded_size: usize,
    sample_rate: f64,
    window: FFTWindow,
}

#[cfg(feature = "fft")]
struct FftResult {
    trace_ref: TraceRef,
    spectrum: Vec<[f64; 2]>,
    info: String,
}

#[cfg(feature = "fft")]
struct FftWorker {
    job_sender: std::sync::mpsc::Sender<FftJob>,
    result_receiver: std::sync::mpsc::Receiver<FftResult>,
}

pub struct FftData {
    pub fft_size: usize,
    pub fft_window: FFTWindow,
    /// Zero-padding factor: FFT is computed on fft_size * zero_pad_factor points.
    /// Values > 1 interpolate between frequency bins for a smoother spectrum.
    pub zero_pad_factor: usize,
    /// Minimum interval between FFT recomputes in milliseconds (throttle).
    pub recompute_interval_ms: u64,
    pub fft_traces: HashMap<TraceRef, TraceData>,
    /// Cached FFT plan — avoids recreating FftPlanner every frame.
    #[cfg(feature = "fft")]
    cached_fft_plan: Option<std::sync::Arc<dyn Fft<f64>>>,
    /// Size of the cached FFT plan (to detect when it needs rebuilding).
    #[cfg(feature = "fft")]
    cached_fft_plan_size: usize,
    /// Per-trace cache key: (buffer_len, last_timestamp) used to skip
    /// recomputation when the underlying data hasn't changed.
    #[cfg(feature = "fft")]
    cached_trace_keys: HashMap<TraceRef, (usize, Option<f64>)>,
    /// Last window type used, to invalidate cache when window changes.
    #[cfg(feature = "fft")]
    cached_window: FFTWindow,
    /// Last paused state, to invalidate cache when pause state changes.
    #[cfg(feature = "fft")]
    cached_paused: bool,
    /// Timestamp of the last FFT recompute, for throttling.
    #[cfg(feature = "fft")]
    last_compute_time: Option<std::time::Instant>,
    /// Background FFT worker thread handle.
    #[cfg(feature = "fft")]
    worker: Option<FftWorker>,
}

impl Default for FftData {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            fft_window: FFTWindow::Hann,
            zero_pad_factor: 1,
            recompute_interval_ms: 100,
            fft_traces: HashMap::default(),
            #[cfg(feature = "fft")]
            cached_fft_plan: None,
            #[cfg(feature = "fft")]
            cached_fft_plan_size: 0,
            #[cfg(feature = "fft")]
            cached_trace_keys: HashMap::default(),
            #[cfg(feature = "fft")]
            cached_window: FFTWindow::Hann,
            #[cfg(feature = "fft")]
            cached_paused: false,
            #[cfg(feature = "fft")]
            last_compute_time: None,
            #[cfg(feature = "fft")]
            worker: None,
        }
    }
}

impl FftData {
    /// The effective FFT size after zero-padding.
    pub fn padded_size(&self) -> usize {
        self.fft_size * self.zero_pad_factor.max(1)
    }

    /// Return the cached FFT plan, creating it if needed.
    #[cfg(feature = "fft")]
    fn get_fft_plan(&mut self, fft_size: usize) -> std::sync::Arc<dyn Fft<f64>> {
        if self.cached_fft_plan_size != fft_size || self.cached_fft_plan.is_none() {
            let mut planner = FftPlanner::new();
            self.cached_fft_plan = Some(planner.plan_fft_forward(fft_size));
            self.cached_fft_plan_size = fft_size;
        }
        self.cached_fft_plan
            .as_ref()
            .expect("FFT plan was just initialized")
            .clone()
    }

    /// Compute the FFT of the most recent samples in the buffer, using the selected window.
    ///
    /// Applies zero-padding when `zero_pad_factor > 1` for finer frequency bin spacing.
    ///
    /// - `buf`: The live buffer of [time, value] samples.
    /// - `paused`: Whether the app is paused (if so, use snapshot buffer).
    /// - `buffer_snapshot`: Optional snapshot buffer (used if paused).
    /// - `fft_size`: Number of samples to use for FFT (must be <= buffer length).
    /// - `fft_window`: Window function to apply before FFT.
    ///
    /// Returns: `Some(Vec<[frequency, magnitude]>)` if enough data, else `None`.
    pub fn compute_fft(
        &mut self,
        buf: &VecDeque<[f64; 2]>,
        paused: bool,
        buffer_snapshot: &Option<VecDeque<[f64; 2]>>,
        fft_size: usize,
        fft_window: FFTWindow,
    ) -> Option<Vec<[f64; 2]>> {
        let buf = if paused {
            buffer_snapshot.as_ref()?
        } else {
            buf
        };
        if buf.len() < fft_size {
            return None;
        }
        let len = buf.len();
        let slice: Vec<[f64; 2]> = buf.iter().skip(len - fft_size).cloned().collect();
        if slice.len() != fft_size {
            return None;
        }
        let t0 = slice.first()?[0];
        let t1 = slice.last()?[0];
        if !(t1 > t0) {
            return None;
        }
        let dt_est = (t1 - t0) / (fft_size as f64 - 1.0);
        if dt_est <= 0.0 {
            return None;
        }
        let sample_rate = 1.0 / dt_est;

        let padded_size = self.padded_size();
        let fft = self.get_fft_plan(padded_size);

        // Window the real input, then zero-pad to padded_size
        let mut data: Vec<Complex<f64>> = slice
            .iter()
            .enumerate()
            .map(|(i, arr)| {
                let w = fft_window.weight(i, fft_size);
                Complex {
                    re: arr[1] * w,
                    im: 0.0,
                }
            })
            .collect();
        data.resize(padded_size, Complex { re: 0.0, im: 0.0 });

        fft.process(&mut data);

        // Compute one-sided magnitude spectrum (up to Nyquist of padded size)
        let half = padded_size / 2;
        let scale = 2.0 / fft_size as f64; // normalize by actual data length, not padded
        let mut out: Vec<[f64; 2]> = Vec::with_capacity(half);
        for (k, c) in data.iter().take(half).enumerate() {
            let freq = k as f64 * sample_rate / padded_size as f64;
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            out.push([freq, mag]);
        }
        Some(out)
    }

    /// Check whether the FFT for a given trace needs to be recomputed.
    /// Returns `true` if the data has changed (or this is the first call).
    ///
    /// Applies a time-based throttle when not paused: even if data has changed,
    /// recomputation is skipped if the last compute was less than
       /// `recompute_interval_ms` ago.
    #[cfg(feature = "fft")]
    pub fn needs_recompute(
        &mut self,
        name: &TraceRef,
        buf_len: usize,
        last_ts: Option<f64>,
        paused: bool,
    ) -> bool {
        let window_changed = self.cached_window != self.fft_window;
        let paused_changed = self.cached_paused != paused;
        if window_changed {
            self.cached_window = self.fft_window;
        }
        if paused_changed {
            self.cached_paused = paused;
        }

        // Always recompute immediately on window/pause change
        if window_changed || paused_changed {
            let key = (buf_len, last_ts);
            self.cached_trace_keys.insert(name.clone(), key);
            self.last_compute_time = Some(std::time::Instant::now());
            return true;
        }

        // Check if data changed
        let key = (buf_len, last_ts);
        let data_changed = self.cached_trace_keys.get(name) != Some(&key);
        if !data_changed {
            return false;
        }

        // Throttle: when not paused, limit recompute rate
        if !paused {
            if let Some(last) = self.last_compute_time {
                let elapsed = last.elapsed();
                let throttle = std::time::Duration::from_millis(self.recompute_interval_ms);
                if elapsed < throttle {
                    // Don't update cache key — we'll recompute when throttle expires
                    return false;
                }
            }
        }

        self.cached_trace_keys.insert(name.clone(), key);
        self.last_compute_time = Some(std::time::Instant::now());
        true
    }

    /// Ensure the background FFT worker thread is running.
    #[cfg(feature = "fft")]
    fn ensure_worker(&mut self) {
        if self.worker.is_none() {
            let (job_tx, job_rx) = std::sync::mpsc::channel();
            let (result_tx, result_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                fft_worker_loop(job_rx, result_tx);
            });
            self.worker = Some(FftWorker {
                job_sender: job_tx,
                result_receiver: result_rx,
            });
        }
    }

    /// Dispatch an FFT computation to the background worker (non-blocking).
    /// Returns `true` if the job was successfully sent.
    #[cfg(feature = "fft")]
    pub fn dispatch_fft(
        &mut self,
        trace_ref: &TraceRef,
        buf: &VecDeque<[f64; 2]>,
        paused: bool,
        buffer_snapshot: &Option<VecDeque<[f64; 2]>>,
    ) -> bool {
        self.ensure_worker();
        let worker = match self.worker.as_ref() {
            Some(w) => w,
            None => return false,
        };

        let src_buf = if paused {
            match buffer_snapshot.as_ref() {
                Some(s) => s,
                None => return false,
            }
        } else {
            buf
        };

        let fft_size = self.fft_size;
        if src_buf.len() < fft_size {
            return false;
        }

        let len = src_buf.len();
        let slice: Vec<[f64; 2]> = src_buf.iter().skip(len - fft_size).cloned().collect();
        if slice.len() != fft_size {
            return false;
        }

        let t0 = slice.first().map(|p| p[0]);
        let t1 = slice.last().map(|p| p[0]);
        match (t0, t1) {
            (Some(t0), Some(t1)) if t1 > t0 => {
                let dt_est = (t1 - t0) / (fft_size as f64 - 1.0);
                if dt_est <= 0.0 {
                    return false;
                }
                let sample_rate = 1.0 / dt_est;
                let samples: Vec<f64> = slice.iter().map(|p| p[1]).collect();
                let padded_size = self.padded_size();

                worker
                    .job_sender
                    .send(FftJob {
                        trace_ref: trace_ref.clone(),
                        samples,
                        fft_size,
                        padded_size,
                        sample_rate,
                        window: self.fft_window,
                    })
                    .is_ok()
            }
            _ => false,
        }
    }

    /// Poll for completed FFT results from the background worker (non-blocking).
    /// Returns `(trace_ref, spectrum, info)` tuples for all completed jobs.
    #[cfg(feature = "fft")]
    pub fn poll_fft_results(&mut self) -> Vec<(TraceRef, Vec<[f64; 2]>, String)> {
        let worker = match self.worker.as_ref() {
            Some(w) => w,
            None => return Vec::new(),
        };

        let mut results = Vec::new();
        while let Ok(result) = worker.result_receiver.try_recv() {
            results.push((result.trace_ref, result.spectrum, result.info));
        }
        results
    }

    #[cfg(not(feature = "fft"))]
    pub fn compute_fft(
        &mut self,
        _buf: &VecDeque<[f64; 2]>,
        _paused: bool,
        _buffer_snapshot: &Option<VecDeque<[f64; 2]>>,
        _fft_size: usize,
        _fft_window: FFTWindow,
    ) -> Option<Vec<[f64; 2]>> {
        None
    }

    #[cfg(not(feature = "fft"))]
    pub fn needs_recompute(
        &mut self,
        _name: &TraceRef,
        _buf_len: usize,
        _last_ts: Option<f64>,
        _paused: bool,
    ) -> bool {
        true
    }

    #[cfg(not(feature = "fft"))]
    pub fn dispatch_fft(
        &mut self,
        _trace_ref: &TraceRef,
        _buf: &VecDeque<[f64; 2]>,
        _paused: bool,
        _buffer_snapshot: &Option<VecDeque<[f64; 2]>>,
    ) -> bool {
        false
    }

    #[cfg(not(feature = "fft"))]
    pub fn poll_fft_results(&mut self) -> Vec<(TraceRef, Vec<[f64; 2]>, String)> {
        Vec::new()
    }
}

/// Background FFT worker loop — receives jobs, computes spectra, sends results.
#[cfg(feature = "fft")]
fn fft_worker_loop(
    job_receiver: std::sync::mpsc::Receiver<FftJob>,
    result_sender: std::sync::mpsc::Sender<FftResult>,
) {
    let mut planner = FftPlanner::new();
    let mut cached_plan: Option<std::sync::Arc<dyn Fft<f64>>> = None;
    let mut cached_plan_size: usize = 0;

    while let Ok(job) = job_receiver.recv() {
        let plan = if cached_plan_size != job.padded_size || cached_plan.is_none() {
            let plan = planner.plan_fft_forward(job.padded_size);
            cached_plan = Some(plan.clone());
            cached_plan_size = job.padded_size;
            plan
        } else {
            cached_plan.as_ref().unwrap().clone()
        };

        // Apply window function and zero-pad
        let mut data: Vec<Complex<f64>> = job
            .samples
            .iter()
            .enumerate()
            .map(|(i, &v)| Complex {
                re: v * job.window.weight(i, job.fft_size),
                im: 0.0,
            })
            .collect();
        data.resize(job.padded_size, Complex { re: 0.0, im: 0.0 });

        plan.process(&mut data);

        let half = job.padded_size / 2;
        let scale = 2.0 / job.fft_size as f64;
        let mut spectrum: Vec<[f64; 2]> = Vec::with_capacity(half);
        for (k, c) in data.iter().take(half).enumerate() {
            let freq = k as f64 * job.sample_rate / job.padded_size as f64;
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            spectrum.push([freq, mag]);
        }

        let pad_factor = job.padded_size / job.fft_size;
        let info = if pad_factor > 1 {
            format!(
                "FFT N={} ×{} {}",
                job.fft_size,
                pad_factor,
                job.window.label()
            )
        } else {
            format!("FFT N={} {}", job.fft_size, job.window.label())
        };

        let _ = result_sender.send(FftResult {
            trace_ref: job.trace_ref,
            spectrum,
            info,
        });
    }
}
