//! Data export utilities - REMOVED AS DUPLICATES
//!
//! NOTE: The following functions were removed as duplicates of main crate src/export.rs:
//! - AlignedRow type alias -> see src/export.rs line ~11
//! - align_series() -> see src/export.rs line ~20
//! - write_aligned_rows_csv() -> see src/export.rs line ~62
//! - write_csv_aligned_path() -> see src/export.rs line ~88
//! - write_parquet_aligned_path() -> see src/export.rs line ~100
//!
//! The only difference was the use of TraceRef vs String types.
//! During merge, consider whether to use TraceRef or String as the key type.
