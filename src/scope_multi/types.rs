use std::collections::VecDeque;

use egui::Color32;

/// Internal per-trace state (live buffer, optional snapshot, color, cached FFT).
pub(crate) struct TraceState {
    pub name: String,
    pub color: Color32,
    pub visible: bool,
    /// Additive Y offset applied before plotting and before optional log transform
    pub offset: f64,
    pub live: VecDeque<[f64; 2]>,
    pub snap: Option<VecDeque<[f64; 2]>>,
    /// Cached last computed FFT (frequency, magnitude)
    pub last_fft: Option<Vec<[f64; 2]>>,
    /// Whether this trace is a derived math trace
    pub is_math: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct MathBuilderState {
    pub name: String,
    pub kind_idx: usize,
    pub add_inputs: Vec<(usize, f64)>,
    pub mul_a_idx: usize,
    pub mul_b_idx: usize,
    pub single_idx: usize, // for differentiate/integrate/filter/minmax
    pub integ_y0: f64,
    pub filter_which: usize, // 0 LP,1 HP,2 BP,3 BQLP,4 BQHP,5 BQBP
    pub filter_f1: f64,
    pub filter_f2: f64,
    pub filter_q: f64,
    pub minmax_decay: f64,
}

impl Default for MathBuilderState {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind_idx: 0,
            add_inputs: vec![(0, 1.0), (0, 1.0)],
            mul_a_idx: 0,
            mul_b_idx: 0,
            single_idx: 0,
            integ_y0: 0.0,
            filter_which: 0,
            filter_f1: 1.0,
            filter_f2: 10.0,
            filter_q: 0.707,
            minmax_decay: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ThresholdBuilderState {
    pub name: String,
    pub target_idx: usize,
    pub kind_idx: usize, // 0: >, 1: <, 2: in range
    pub thr1: f64,
    pub thr2: f64,
    pub min_duration_ms: f64,
    pub max_events: usize,
}

impl Default for ThresholdBuilderState {
    fn default() -> Self {
        Self {
            name: String::new(),
            target_idx: 0,
            kind_idx: 0,
            thr1: 0.0,
            thr2: 1.0,
            min_duration_ms: 2.0,
            max_events: 100,
        }
    }
}

// (no-op)
