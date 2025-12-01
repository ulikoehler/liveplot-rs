//! Data module - ALL ITEMS MERGED INTO MAIN CRATE
//!
//! ==============================================================================
//! MERGE STATUS: COMPLETE - All files merged to main crate (2024-12-01)
//! ==============================================================================
//!
//! Merged to main crate:
//! - data.rs (LivePlotData) -> src/data/data.rs
//! - traces.rs (TraceRef, TracesCollection, TraceData) -> src/data/traces.rs
//! - scope.rs (AxisSettings, ScopeType, ScopeData) -> src/data/scope.rs
//! - trace_look.rs (TraceLook) -> src/data/trace_look.rs
//! - triggers.rs (Trigger, TriggerSlope) -> src/data/triggers.rs
//! - thresholds.rs (ThresholdDef, ThresholdEvent) -> src/data/thresholds.rs
//! - math.rs (MathTrace, MathKind, FilterKind) -> src/data/math.rs
//! - export.rs -> src/data/export.rs
//! - measurement.rs (Measurement) -> src/data/measurement.rs

pub mod data;
pub mod export;
pub mod math;
pub mod measurement;
pub mod scope;
pub mod thresholds;
pub mod trace_look;
pub mod traces;
pub mod triggers;

#[cfg(feature = "fft")]
pub mod fft;
