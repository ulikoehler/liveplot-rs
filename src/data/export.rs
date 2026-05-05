//! Data export utilities: align multi-trace time series by timestamp tolerance and write CSV.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
#[cfg(feature = "parquet")]
use std::sync::Arc;

use crate::data::traces::TraceRef;

/// A single aligned row: timestamp in seconds and one value per trace (None if missing).
pub type AlignedRow = (f64, Vec<Option<f64>>);

/// Align multiple traces by timestamp with a given tolerance (in seconds).
///
/// Contract:
/// - Input `series` is a map from trace name to sorted `[timestamp_sec, value]` pairs.
/// - `trace_order` defines the column order in the resulting rows.
/// - Rows are sorted by the smallest timestamp among the grouped samples.
/// - Samples whose timestamps differ by at most `tol` are merged into the same row.
pub fn align_series(
    trace_order: &[TraceRef],
    series: &HashMap<TraceRef, Vec<[f64; 2]>>,
    tol: f64,
) -> Vec<AlignedRow> {
    // Build per-trace indices and local refs for faster access.
    let mut idx: Vec<usize> = vec![0; trace_order.len()];
    let refs: Vec<&[[f64; 2]]> = trace_order
        .iter()
        .map(|name| series.get(name).map(|v| v.as_slice()).unwrap_or(&[][..]))
        .collect();

    let mut out: Vec<AlignedRow> = Vec::new();
    loop {
        // Find the minimum next timestamp across traces.
        let mut t_min: Option<f64> = None;
        for (i, data) in refs.iter().enumerate() {
            if idx[i] < data.len() {
                let t = data[idx[i]][0];
                if t_min.map_or(true, |m| t < m) {
                    t_min = Some(t);
                }
            }
        }
        let Some(t_ref) = t_min else {
            break;
        };

        // For each trace, if the next sample is within tolerance, consume it into the row.
        let mut row_vals: Vec<Option<f64>> = Vec::with_capacity(trace_order.len());
        for (i, data) in refs.iter().enumerate() {
            let mut val: Option<f64> = None;
            if idx[i] < data.len() {
                let t = data[idx[i]][0];
                if (t - t_ref).abs() <= tol {
                    val = Some(data[idx[i]][1]);
                    idx[i] += 1;
                }
            }
            row_vals.push(val);
        }
        out.push((t_ref, row_vals));
    }
    out
}

/// Write aligned rows to CSV with the header: `timestamp_seconds,<trace1>,<trace2>,...`.
pub fn write_aligned_rows_csv<W: Write>(
    mut w: W,
    trace_order: &[TraceRef],
    rows: &[AlignedRow],
) -> io::Result<()> {
    // Header
    write!(w, "timestamp_seconds")?;
    for name in trace_order {
        write!(w, ",{}", name.0)?;
    }
    writeln!(w)?;

    // Rows
    for (t, vals) in rows.iter() {
        // 9 decimal places as in previous CSV
        write!(w, "{:.9}", *t)?;
        for v in vals.iter() {
            if let Some(y) = v {
                write!(w, ",{}", y)?;
            } else {
                write!(w, ",")?;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}

/// Convenience: align series by tolerance and write to a CSV file at `path`.
pub fn write_csv_aligned_path(
    path: &Path,
    trace_order: &[TraceRef],
    series: &HashMap<TraceRef, Vec<[f64; 2]>>,
    tol: f64,
) -> io::Result<()> {
    let rows = align_series(trace_order, series, tol);
    let mut f = std::fs::File::create(path)?;
    write_aligned_rows_csv(&mut f, trace_order, &rows)
}

/// Convenience: align series by tolerance and write to a Parquet file at `path` (feature-gated).
///
/// Schema: `timestamp_seconds: Float64` + one nullable `Float64` column per trace in `trace_order`.
#[cfg(feature = "parquet")]
pub fn write_parquet_aligned_path(
    path: &Path,
    trace_order: &[TraceRef],
    series: &HashMap<TraceRef, Vec<[f64; 2]>>,
    tol: f64,
) -> io::Result<()> {
    use arrow_array::builder::Float64Builder;
    use arrow_array::{ArrayRef, Float64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::arrow_writer::ArrowWriter;
    use parquet::file::properties::WriterProperties;

    let rows = align_series(trace_order, series, tol);

    // Build Arrow schema
    let mut fields: Vec<Field> = Vec::with_capacity(1 + trace_order.len());
    fields.push(Field::new("timestamp_seconds", DataType::Float64, false));
    for name in trace_order.iter() {
        // TraceRef implements AsRef<str>; Arrow Field::new accepts Into<String>
        fields.push(Field::new(name.as_ref(), DataType::Float64, true));
    }
    let schema = Arc::new(Schema::new(fields));

    // Build column arrays
    let mut ts_builder = Float64Builder::with_capacity(rows.len());
    let mut value_builders: Vec<Float64Builder> = trace_order
        .iter()
        .map(|_| Float64Builder::with_capacity(rows.len()))
        .collect();

    for (t, vals) in rows.iter() {
        ts_builder.append_value(*t);
        for (i, v) in vals.iter().enumerate() {
            if let Some(y) = v {
                value_builders[i].append_value(*y);
            } else {
                value_builders[i].append_null();
            }
        }
    }

    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(1 + value_builders.len());
    arrays.push(Arc::new(ts_builder.finish()));
    for mut b in value_builders.into_iter() {
        let arr: Float64Array = b.finish();
        arrays.push(Arc::new(arr));
    }

    let batch = RecordBatch::try_new(schema.clone(), arrays)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    // Write Parquet
    let file = std::fs::File::create(path)?;
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    writer
        .write(&batch)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    writer
        .close()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    Ok(())
}

/// Stub if the `parquet` feature is disabled.
#[cfg(not(feature = "parquet"))]
pub fn write_parquet_aligned_path(
    _path: &Path,
    _trace_order: &[TraceRef],
    _series: &HashMap<TraceRef, Vec<[f64; 2]>>,
    _tol: f64,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Parquet export not available: build with feature `parquet`",
    ))
}

// tests moved to `tests/export.rs`
