pub mod traces;
pub mod math;
pub mod fft;
pub mod export;
pub mod trace_look;
pub mod thresholds;
pub mod scope;

// Shared context passed to panels for non-UI data and functions
#[derive(Default)]
pub struct DataContext {
    pub traces: traces::TracesData,
    pub math: math::MathData,
    pub fft: fft::FftData,
    pub export: export::ExportData,
    pub thresholds: thresholds::ThresholdsData,
}

impl DataContext {
    pub fn new_with_rx(rx: std::sync::mpsc::Receiver<crate::sink::MultiSample>) -> Self {
        let mut s = Self::default();
        s.traces.set_rx(rx);
        s
    }

    pub fn calculate(&mut self) {
        self.traces.drain_and_update();
        self.traces.prune_by_time_window();
        self.math.calculate(&mut self.traces);
        self.fft.calculate();
        self.thresholds.calculate(&self.traces);
        self.export.calculate();
    }
}
