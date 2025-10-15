use std::collections::VecDeque;

use egui::Color32;

#[derive(Debug, Clone)]
pub(crate) struct TraceLook {
    pub visible: bool,
    
    // Line style
    pub color: Color32,
    pub width: f32,
    pub style: egui_plot::LineStyle,

    // Point style
    pub show_points: bool,
    pub point_size: f32,
    pub marker: egui_plot::MarkerShape,
}

impl Default for TraceLook {
    fn default() -> Self {
        Self {
            visible: true,
            color: Color32::WHITE,
            width: 1.0,
            style: egui_plot::LineStyle::Solid,
            show_points: false,
            point_size: 2.0,
            marker: egui_plot::MarkerShape::Circle,
        }
    }
}

/// Internal per-trace state (live buffer, optional snapshot, color, cached FFT).
pub(crate) struct TraceState {
    pub name: String,
    pub look: TraceLook,
    /// Additive Y offset applied before plotting and before optional log transform
    pub offset: f64,
    pub live: VecDeque<[f64; 2]>,
    pub snap: Option<VecDeque<[f64; 2]>>,
    /// Cached last computed FFT (frequency, magnitude)
    #[cfg_attr(not(feature = "fft"), allow(dead_code))]
    pub last_fft: Option<Vec<[f64; 2]>>,
    /// Whether this trace is a derived math trace
    pub is_math: bool,
    pub info: String,
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
    pub look: TraceLook,
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
            look: TraceLook::default(),
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
    pub look: TraceLook,
    pub look_start_events: TraceLook,
    pub look_stop_events: TraceLook,
}

impl Default for ThresholdBuilderState {
    fn default() -> Self {
        let mut look = TraceLook::default();
        look.style = egui_plot::LineStyle::Dashed { length: 6.0 };
        let mut look_start = TraceLook::default();
        look_start.show_points = true;
        look_start.point_size = 6.0;
        look_start.marker = egui_plot::MarkerShape::Diamond;
        // Hide line by default for start/stop looks; rely on points
        look_start.visible = true; // keep visible, but the renderer will use points setting
        let mut look_stop = TraceLook::default();
        look_stop.show_points = true;
        look_stop.point_size = 6.0;
        look_stop.marker = egui_plot::MarkerShape::Square;
        Self {
            name: String::new(),
            target_idx: 0,
            kind_idx: 0,
            thr1: 0.0,
            thr2: 1.0,
            min_duration_ms: 2.0,
            max_events: 100,
            look,
            look_start_events: look_start,
            look_stop_events: look_stop,
        }
    }
}

// (no-op)
