pub mod traces;
pub mod math;
pub mod fft;
pub mod export;

// Shared context passed to panels for non-UI data and functions
#[derive(Default)]
pub struct DataContext {
    pub traces: traces::TracesData,
    pub math: math::MathData,
    pub fft: fft::FftData,
    pub export: export::ExportData,
}
