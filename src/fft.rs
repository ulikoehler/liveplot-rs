
// FFT logic for time-series data, extracted from main.rs
// Provides windowing and spectrum calculation utilities for plotting
use rustfft::{FftPlanner, num_complex::Complex};
use std::collections::VecDeque;


/// Supported FFT window functions for spectral analysis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FftWindow {
    /// Rectangular (no windowing)
    Rect,
    /// Hann window
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window
    Blackman,
}

impl FftWindow {
    /// All available window types (for UI selection)
    pub const ALL: &'static [FftWindow] = &[
        FftWindow::Rect,
        FftWindow::Hann,
        FftWindow::Hamming,
        FftWindow::Blackman,
    ];

    /// Human-readable label for each window type
    pub fn label(&self) -> &'static str {
        match self {
            FftWindow::Rect => "Rect",
            FftWindow::Hann => "Hann",
            FftWindow::Hamming => "Hamming",
            FftWindow::Blackman => "Blackman",
        }
    }

    /// Compute the window weight for a given sample index
    pub fn weight(&self, n: usize, len: usize) -> f64 {
        match self {
            FftWindow::Rect => 1.0,
            FftWindow::Hann => {
                // Hann window: w[n] = 0.5 - 0.5*cos(2*pi*n/(N-1))
                0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
            FftWindow::Hamming => {
                // Hamming window: w[n] = 0.54 - 0.46*cos(2*pi*n/(N-1))
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
            FftWindow::Blackman => {
                // Blackman window: w[n] = 0.42 - 0.5*cos(2*pi*n/(N-1)) + 0.08*cos(4*pi*n/(N-1))
                0.42 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
                    + 0.08 * (4.0 * std::f64::consts::PI * n as f64 / (len as f64)).cos()
            }
        }
    }
}


/// Compute the FFT of the most recent samples in the buffer, using the selected window.
///
/// - `buf`: The live buffer of [time, value] samples.
/// - `paused`: Whether the app is paused (if so, use snapshot buffer).
/// - `buffer_snapshot`: Optional snapshot buffer (used if paused).
/// - `fft_size`: Number of samples to use for FFT (must be <= buffer length).
/// - `fft_window`: Window function to apply before FFT.
///
/// Returns: `Some(Vec<[frequency, magnitude]>)` if enough data, else `None`.
pub fn compute_fft(
    buf: &VecDeque<[f64; 2]>,
    paused: bool,
    buffer_snapshot: &Option<VecDeque<[f64; 2]>>,
    fft_size: usize,
    fft_window: FftWindow,
) -> Option<Vec<[f64;2]>> {
    // Use snapshot buffer if paused, else live buffer
    let buf = if paused { buffer_snapshot.as_ref()? } else { buf };
    if buf.len() < fft_size { return None; }
    let len = buf.len();
    // Take the last fft_size samples for analysis
    let slice: Vec<[f64;2]> = buf.iter().skip(len - fft_size).cloned().collect();
    if slice.len() != fft_size { return None; }
    let t0 = slice.first()?[0];
    let t1 = slice.last()?[0];
    if !(t1 > t0) { return None; }
    // Estimate sample rate from time axis
    let dt_est = (t1 - t0) / (fft_size as f64 - 1.0);
    if dt_est <= 0.0 { return None; }
    let sample_rate = 1.0 / dt_est;

    // Prepare windowed real input for FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut data: Vec<Complex<f64>> = slice.iter().enumerate().map(|(i, arr)| {
        let w = fft_window.weight(i, fft_size);
        Complex { re: arr[1] * w, im: 0.0 }
    }).collect();
    fft.process(&mut data);

    // Compute one-sided magnitude spectrum (up to Nyquist)
    let half = fft_size / 2;
    let scale = 2.0 / fft_size as f64; // amplitude normalization
    let mut out: Vec<[f64;2]> = Vec::with_capacity(half);
    for (k, c) in data.iter().take(half).enumerate() {
        let freq = k as f64 * sample_rate / fft_size as f64;
        let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
        out.push([freq, mag]);
    }
    Some(out)
}
